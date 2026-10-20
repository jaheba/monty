//! OXC syntax lowered to the shared compiler IR, without generating Python source.

use std::{collections::HashSet, mem};

use num_traits::ToPrimitive;
use oxc_allocator::Allocator;
use oxc_ast::ast::{
    Argument, ArrayExpressionElement, ArrowFunctionBody, AssignmentTarget, BindingPattern, CallExpression,
    ChainElement, Expression, ForStatementInit, ForStatementLeft, FormalParameters, Function, FunctionBody,
    ObjectPropertyKind, PropertyKey, PropertyKind, SimpleAssignmentTarget, Statement, VariableDeclaration,
    VariableDeclarationKind,
};
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType, Span};

use monty_types::MontyException;

use crate::{
    args::{ArgExprs, CallArg},
    builtins::Builtins,
    exception_private::ExcType,
    expressions::{
        Callable, CmpOperator, DictItem, Expr, ExprLoc, Identifier, Literal, Node, Operator, SequenceItem, UnpackTarget,
    },
    intern::{CompileInterns, StringId},
    parse::{
        CodeRange, ExceptHandler, ParseNode, ParsedParam, ParsedSignature, RawFunctionDef, Try, parse_with_interner,
    },
    types::Type,
    value::EitherStr,
};

pub(crate) const STDLIB: &str = include_str!("javascript/stdlib.py");
pub(crate) const STDLIB_FILENAME: &str = "<javascript-builtins>";

// Helper names are reserved in JS; Python can deliberately replace shared globals.
const PREFIX: &str = "_js_";

type LowerResult<T> = Result<T, MontyException>;

fn unsupported(feature: &str) -> MontyException {
    MontyException::new(
        ExcType::SyntaxError,
        Some(format!("experimental JavaScript frontend: {feature}")),
    )
}

pub(crate) fn check_source(source: &str) -> LowerResult<()> {
    let too_complex = || unsupported("source exceeds the proof-of-concept complexity limit");
    if source.len() > 65_536 {
        return Err(too_complex());
    }

    let bytes = source.as_bytes();
    let mut i = 0;
    let mut delimiters = Vec::new();
    let mut work = 0;
    while i < bytes.len() {
        let byte = bytes[i];
        if byte == b'\'' || byte == b'"' {
            i += 1;
            while i < bytes.len() && bytes[i] != byte {
                i += if bytes[i] == b'\\' { 2 } else { 1 };
            }
            i += 1;
            continue;
        }
        let mut fallback = matches!(byte, b'/' | b'`' | b'\\');
        if byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$') || byte >= 128 {
            let start = i;
            while i < bytes.len()
                && (bytes[i].is_ascii_alphanumeric() || matches!(bytes[i], b'_' | b'$') || bytes[i] >= 128)
            {
                i += 1;
            }
            fallback = &bytes[start..i] == b"else";
        } else {
            i += 1;
        }
        if fallback {
            // Regex, templates and else-if chains need grammar-aware depth tracking.
            // Retain the original whole-feed bound for these forms, including comments.
            return if source.chars().filter(|c| "([{!?=+-*/%~^&|<>.".contains(*c)).count() <= 128
                && source.split_whitespace().count() <= 512
            {
                Ok(())
            } else {
                Err(too_complex())
            };
        }
        match byte {
            b'(' | b'[' | b'{' => {
                delimiters.push(byte);
                if delimiters.len() > 24 {
                    return Err(too_complex());
                }
            }
            b')' | b']' | b'}' => {
                let open = delimiters.pop();
                if !matches!(
                    (open, byte),
                    (Some(b'('), b')') | (Some(b'['), b']') | (Some(b'{'), b'}')
                ) {
                    return Err(unsupported("unbalanced delimiters"));
                }
            }
            b';' if !delimiters.iter().any(|b| matches!(b, b'(' | b'[')) => work = 0,
            _ => {}
        }
        if !byte.is_ascii_whitespace() {
            work += 1;
        }
        if work > 128 {
            return Err(too_complex());
        }
    }
    if !delimiters.is_empty() {
        return Err(unsupported("unbalanced delimiters"));
    }
    Ok(())
}

pub(crate) fn parse(
    source: &str,
    filename: &str,
    interns: &mut CompileInterns<'_>,
    install_stdlib: bool,
) -> LowerResult<Vec<ParseNode>> {
    check_source(source)?;

    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::mjs()).parse();
    if let Some(error) = parsed.diagnostics.errors().next() {
        return Err(unsupported(&error.to_string()));
    }
    let filename = interns.intern(filename);
    let mut lower = Lower {
        interns,
        filename,
        temporary: 0,
        loop_updates: Vec::new(),
        scopes: vec![Scope::default()],
        finally_depth: 0,
    };
    let mut nodes = if install_stdlib {
        parse_with_interner(STDLIB, STDLIB_FILENAME, lower.interns)
            .map_err(|e| e.into_python_exc(STDLIB_FILENAME, STDLIB))?
    } else {
        Vec::new()
    };
    if install_stdlib {
        let name = lower.interns.intern("_js_eq");
        let def = nodes
            .iter_mut()
            .find_map(|node| match node {
                Node::FunctionDef { def, .. } if def.name.name_id == name => Some(def),
                _ => None,
            })
            .expect("stdlib equality intrinsic");
        let position = def.name.position;
        let a = Identifier::new(def.signature.args[0].name, position);
        let b = Identifier::new(def.signature.args[1].name, position);
        def.body = vec![Node::Return(Some(ExprLoc::new(
            position,
            Expr::Op {
                left: Box::new(Lower::name(a)),
                op: Operator::JsStrictEq,
                right: Box::new(Lower::name(b)),
            },
        )))];
    }
    if install_stdlib {
        nodes.push(Node::Assign {
            target: lower.identifier("_js_ready", Span::default()),
            object: ExprLoc::new(lower.range(Span::default()), Expr::Literal(Literal::Bool(true))),
        });
    }
    for directive in &parsed.program.directives {
        let value = directive
            .expression
            .value
            .as_str()
            .ok_or_else(|| unsupported("lone UTF-16 surrogates"))?;
        nodes.push(Node::Expr(lower.string(value, directive.span)));
    }
    for statement in &parsed.program.body {
        declared_names(statement, lower.interns, &mut lower.scopes[0].declared);
    }
    for statement in &parsed.program.body {
        lower.statement(statement, &mut nodes)?;
    }
    Ok(nodes)
}

#[derive(Default)]
struct Scope {
    declared: HashSet<StringId>,
    written: Vec<StringId>,
}

fn pattern_names(pattern: &BindingPattern<'_>, interns: &mut CompileInterns<'_>, names: &mut HashSet<StringId>) {
    match pattern {
        BindingPattern::BindingIdentifier(name) => {
            names.insert(interns.intern(name.name.as_str()));
        }
        BindingPattern::AssignmentPattern(pattern) => pattern_names(&pattern.left, interns, names),
        BindingPattern::ArrayPattern(pattern) => {
            for pattern in pattern.elements.iter().flatten() {
                pattern_names(pattern, interns, names);
            }
            if let Some(rest) = &pattern.rest {
                pattern_names(&rest.argument, interns, names);
            }
        }
        BindingPattern::ObjectPattern(pattern) => {
            for property in &pattern.properties {
                pattern_names(&property.value, interns, names);
            }
            if let Some(rest) = &pattern.rest {
                pattern_names(&rest.argument, interns, names);
            }
        }
    }
}

fn declared_names(statement: &Statement<'_>, interns: &mut CompileInterns<'_>, names: &mut HashSet<StringId>) {
    match statement {
        Statement::VariableDeclaration(declaration) => {
            for binding in &declaration.declarations {
                pattern_names(&binding.id, interns, names);
            }
        }
        Statement::FunctionDeclaration(function) => {
            if let Some(name) = &function.id {
                names.insert(interns.intern(name.name.as_str()));
            }
        }
        Statement::BlockStatement(block) => {
            for statement in &block.body {
                declared_names(statement, interns, names);
            }
        }
        Statement::IfStatement(statement) => {
            declared_names(&statement.consequent, interns, names);
            if let Some(alternate) = &statement.alternate {
                declared_names(alternate, interns, names);
            }
        }
        Statement::WhileStatement(statement) => declared_names(&statement.body, interns, names),
        Statement::ForStatement(statement) => {
            if let Some(ForStatementInit::VariableDeclaration(declaration)) = &statement.init {
                for binding in &declaration.declarations {
                    pattern_names(&binding.id, interns, names);
                }
            }
            declared_names(&statement.body, interns, names);
        }
        Statement::ForOfStatement(statement) => {
            if let ForStatementLeft::VariableDeclaration(declaration) = &statement.left {
                for binding in &declaration.declarations {
                    pattern_names(&binding.id, interns, names);
                }
            }
            declared_names(&statement.body, interns, names);
        }
        Statement::TryStatement(statement) => {
            for stmt in &statement.block.body {
                declared_names(stmt, interns, names);
            }
            if let Some(handler) = &statement.handler {
                if let Some(param) = &handler.param {
                    pattern_names(&param.pattern, interns, names);
                }
                for stmt in &handler.body.body {
                    declared_names(stmt, interns, names);
                }
            }
            if let Some(finalizer) = &statement.finalizer {
                for stmt in &finalizer.body {
                    declared_names(stmt, interns, names);
                }
            }
        }
        _ => {}
    }
}

struct Lower<'a, 'i> {
    interns: &'a mut CompileInterns<'i>,
    filename: StringId,
    temporary: usize,
    loop_updates: Vec<Option<ExprLoc>>,
    scopes: Vec<Scope>,
    finally_depth: usize,
}

impl Lower<'_, '_> {
    fn call(&mut self, call: &CallExpression<'_>) -> LowerResult<ExprLoc> {
        let args = call
            .arguments
            .iter()
            .map(|arg| match arg {
                Argument::SpreadElement(spread) => self.expression(&spread.argument).map(CallArg::Unpack),
                arg => self.expression(arg.to_expression()).map(CallArg::Value),
            })
            .collect::<LowerResult<Vec<_>>>()?;
        if matches!(&call.callee, Expression::Identifier(name) if name.name == "Array") {
            let callable = Callable::Name(self.identifier("_js_array", call.span));
            return Ok(ExprLoc::new(
                self.range(call.span),
                Expr::Call {
                    callable,
                    args: Box::new(ArgExprs::new_generalized(args, Vec::new())),
                },
            ));
        }
        if call.optional {
            let target = self.expression(&call.callee)?;
            let temp = self.fresh(call.span);
            let test = ExprLoc::new(
                self.range(call.span),
                Expr::Named {
                    target: temp,
                    value: Box::new(target),
                },
            );
            let call_expr = ExprLoc::new(
                self.range(call.span),
                Expr::IndirectCall {
                    callable: Box::new(Self::name(temp)),
                    args: Box::new(ArgExprs::new_generalized(args, Vec::new())),
                },
            );
            return Ok(ExprLoc::new(
                self.range(call.span),
                Expr::IfElse {
                    test: Box::new(Self::null_test(test)),
                    body: Box::new(self.null(call.span)),
                    orelse: Box::new(call_expr),
                },
            ));
        }
        match &call.callee {
            Expression::StaticMemberExpression(member) => {
                let object = self.expression(&member.object)?;
                let key = self.string(member.property.name.as_str(), member.property.span);
                Ok(self.method(object, key, args, member.optional, call.span))
            }
            Expression::ComputedMemberExpression(member) => {
                let object = self.expression(&member.object)?;
                let key = self.expression(&member.expression)?;
                Ok(self.method(object, key, args, member.optional, call.span))
            }
            Expression::Identifier(name) => {
                let callable = Callable::Name(self.user_identifier(name.name.as_str(), name.span)?);
                Ok(ExprLoc::new(
                    self.range(call.span),
                    Expr::Call {
                        callable,
                        args: Box::new(ArgExprs::new_generalized(args, Vec::new())),
                    },
                ))
            }
            _ => Ok(ExprLoc::new(
                self.range(call.span),
                Expr::IndirectCall {
                    callable: Box::new(self.expression(&call.callee)?),
                    args: Box::new(ArgExprs::new_generalized(args, Vec::new())),
                },
            )),
        }
    }

    fn null(&self, span: Span) -> ExprLoc {
        ExprLoc::new(self.range(span), Expr::Literal(Literal::None))
    }

    fn name(name: Identifier) -> ExprLoc {
        ExprLoc::new(name.position, Expr::Name(name))
    }

    fn user_identifier(&mut self, name: &str, span: Span) -> LowerResult<Identifier> {
        if name.starts_with(PREFIX) {
            return Err(unsupported("reserved helper name"));
        }
        Ok(self.identifier(name, span))
    }

    fn fresh(&mut self, span: Span) -> Identifier {
        self.temporary += 1;
        self.identifier(&format!("<js:temp:{}>", self.temporary), span)
    }

    fn helper(&mut self, name: &str, args: Vec<ExprLoc>, span: Span) -> ExprLoc {
        let callable = Callable::Name(self.identifier(&format!("{PREFIX}{name}"), span));
        ExprLoc::new(
            self.range(span),
            Expr::Call {
                callable,
                args: Box::new(ArgExprs::Args(args)),
            },
        )
    }

    fn null_test(value: ExprLoc) -> ExprLoc {
        let position = value.position;
        ExprLoc::new(
            position,
            Expr::CmpOp {
                left: Box::new(value),
                op: CmpOperator::Is,
                right: Box::new(ExprLoc::new(position, Expr::Literal(Literal::None))),
            },
        )
    }

    fn test(&mut self, value: &Expression<'_>) -> LowerResult<ExprLoc> {
        let expr = self.expression(value)?;
        Ok(self.helper("truth", vec![expr], value.span()))
    }

    fn get(&mut self, object: ExprLoc, key: ExprLoc, optional: bool, span: Span) -> ExprLoc {
        if optional {
            let temp = self.fresh(span);
            let test = ExprLoc::new(
                self.range(span),
                Expr::Named {
                    target: temp,
                    value: Box::new(object),
                },
            );
            let read = self.helper("get", vec![Self::name(temp), key], span);
            ExprLoc::new(
                self.range(span),
                Expr::IfElse {
                    test: Box::new(Self::null_test(test)),
                    body: Box::new(self.null(span)),
                    orelse: Box::new(read),
                },
            )
        } else {
            self.helper("get", vec![object, key], span)
        }
    }

    fn method(&mut self, object: ExprLoc, key: ExprLoc, args: Vec<CallArg>, optional: bool, span: Span) -> ExprLoc {
        let (object, guard) = if optional {
            let temp = self.fresh(span);
            let test = ExprLoc::new(
                self.range(span),
                Expr::Named {
                    target: temp,
                    value: Box::new(object),
                },
            );
            (Self::name(temp), Some(Self::null_test(test)))
        } else {
            (object, None)
        };
        let mut all = vec![CallArg::Value(object), CallArg::Value(key)];
        all.extend(args);
        let call = ExprLoc::new(
            self.range(span),
            Expr::Call {
                callable: Callable::Name(self.identifier("_js_method", span)),
                args: Box::new(ArgExprs::new_generalized(all, Vec::new())),
            },
        );
        if let Some(test) = guard {
            ExprLoc::new(
                self.range(span),
                Expr::IfElse {
                    test: Box::new(test),
                    body: Box::new(self.null(span)),
                    orelse: Box::new(call),
                },
            )
        } else {
            call
        }
    }

    fn property_key(&mut self, key: &PropertyKey<'_>, computed: bool) -> LowerResult<ExprLoc> {
        if let PropertyKey::StaticIdentifier(key) = key {
            return Ok(self.string(key.name.as_str(), key.span));
        }
        if let PropertyKey::PrivateIdentifier(_) = key {
            return Err(unsupported("private properties"));
        }
        let value = self.expression(key.to_expression())?;
        if computed || !matches!(key, PropertyKey::StringLiteral(_)) {
            Ok(self.helper("string", vec![value], key.span()))
        } else {
            Ok(value)
        }
    }

    fn declaration(&mut self, declaration: &VariableDeclaration<'_>, nodes: &mut Vec<ParseNode>) -> LowerResult<()> {
        if declaration.kind == VariableDeclarationKind::Var {
            return Err(unsupported("var; use let or const"));
        }
        for binding in &declaration.declarations {
            let value = binding
                .init
                .as_ref()
                .map(|e| self.expression(e))
                .transpose()?
                .unwrap_or_else(|| self.null(binding.span));
            self.bind_pattern(&binding.id, value, nodes)?;
        }
        Ok(())
    }

    fn bind_pattern(
        &mut self,
        pattern: &BindingPattern<'_>,
        value: ExprLoc,
        nodes: &mut Vec<ParseNode>,
    ) -> LowerResult<()> {
        let span = pattern.span();
        match pattern {
            BindingPattern::BindingIdentifier(_) => nodes.push(Node::Assign {
                target: self.binding(pattern)?,
                object: value,
            }),
            BindingPattern::AssignmentPattern(pattern) => {
                let temp = self.fresh(span);
                nodes.push(Node::Assign {
                    target: temp,
                    object: value,
                });
                let fallback = self.expression(&pattern.right)?;
                let value = ExprLoc::new(
                    self.range(span),
                    Expr::IfElse {
                        test: Box::new(Self::null_test(Self::name(temp))),
                        body: Box::new(fallback),
                        orelse: Box::new(Self::name(temp)),
                    },
                );
                self.bind_pattern(&pattern.left, value, nodes)?;
            }
            BindingPattern::ObjectPattern(pattern) => {
                let temp = self.fresh(span);
                nodes.push(Node::Assign {
                    target: temp,
                    object: value,
                });
                let mut keys = Vec::new();
                for property in &pattern.properties {
                    let key = self.property_key(&property.key, property.computed)?;
                    let saved = self.fresh(property.span);
                    nodes.push(Node::Assign {
                        target: saved,
                        object: key,
                    });
                    keys.push(SequenceItem::Value(Self::name(saved)));
                    let value = self.get(Self::name(temp), Self::name(saved), false, property.span);
                    self.bind_pattern(&property.value, value, nodes)?;
                }
                if let Some(rest) = &pattern.rest {
                    let keys = ExprLoc::new(self.range(span), Expr::List(keys));
                    let value = self.helper("rest", vec![Self::name(temp), keys], span);
                    self.bind_pattern(&rest.argument, value, nodes)?;
                }
            }
            BindingPattern::ArrayPattern(pattern) => {
                let temp = self.fresh(span);
                nodes.push(Node::Assign {
                    target: temp,
                    object: value,
                });
                for (index, pattern) in pattern.elements.iter().enumerate() {
                    if let Some(pattern) = pattern {
                        let index = ExprLoc::new(
                            self.range(span),
                            Expr::Literal(Literal::Int(i64::try_from(index).expect("source bound"))),
                        );
                        let value = self.get(Self::name(temp), index, false, span);
                        self.bind_pattern(pattern, value, nodes)?;
                    }
                }
                if let Some(rest) = &pattern.rest {
                    let start = ExprLoc::new(
                        self.range(span),
                        Expr::Literal(Literal::Int(
                            i64::try_from(pattern.elements.len()).expect("source bound"),
                        )),
                    );
                    let key = self.string("slice", span);
                    let value = self.method(Self::name(temp), key, vec![CallArg::Value(start)], false, span);
                    self.bind_pattern(&rest.argument, value, nodes)?;
                }
            }
        }
        Ok(())
    }

    fn parameter_name(&mut self, pattern: &BindingPattern<'_>, span: Span) -> LowerResult<Identifier> {
        if matches!(pattern, BindingPattern::BindingIdentifier(_)) {
            self.binding(pattern)
        } else {
            Ok(self.identifier(&format!("<js:param:{}>", span.start), span))
        }
    }

    fn rest_prologue(&mut self, params: &FormalParameters<'_>) -> LowerResult<Vec<ParseNode>> {
        let mut nodes = Vec::new();
        for param in &params.items {
            let target = self.parameter_name(&param.pattern, param.span)?;
            if let Some(default) = &param.initializer {
                nodes.push(Node::If {
                    test: Self::null_test(Self::name(target)),
                    body: vec![Node::Assign {
                        target,
                        object: self.expression(default)?,
                    }],
                    or_else: Vec::new(),
                });
            }
            if !matches!(param.pattern, BindingPattern::BindingIdentifier(_)) {
                self.bind_pattern(&param.pattern, Self::name(target), &mut nodes)?;
            }
        }
        if let Some(rest) = &params.rest {
            let target = self.binding(&rest.rest.argument)?;
            let object = ExprLoc::new(
                self.range(rest.span),
                Expr::Call {
                    callable: Callable::Builtin(Builtins::Type(Type::List)),
                    args: Box::new(ArgExprs::One(Self::name(target))),
                },
            );
            nodes.push(Node::Assign { target, object });
        }
        Ok(nodes)
    }

    fn enter_function(&mut self, params: &FormalParameters<'_>, body: Option<&FunctionBody<'_>>) {
        let mut scope = Scope::default();
        for param in &params.items {
            pattern_names(&param.pattern, self.interns, &mut scope.declared);
        }
        if let Some(rest) = &params.rest {
            pattern_names(&rest.rest.argument, self.interns, &mut scope.declared);
        }
        if let Some(body) = body {
            for statement in &body.statements {
                declared_names(statement, self.interns, &mut scope.declared);
            }
        }
        self.scopes.push(scope);
    }

    fn write(&mut self, target: Identifier) {
        let scope = self.scopes.last_mut().expect("module scope");
        if !scope.written.contains(&target.name_id) {
            scope.written.push(target.name_id);
        }
    }

    fn leave_function(&mut self, nodes: &mut Vec<ParseNode>, position: CodeRange) {
        let scope = self.scopes.pop().expect("function scope");
        for name in scope.written {
            if scope.declared.contains(&name) {
                continue;
            }
            // Block scopes are flattened, but writes to enclosing function bindings use shared cells.
            let captured = self
                .scopes
                .iter()
                .skip(1)
                .rev()
                .any(|scope| scope.declared.contains(&name));
            let node = if captured {
                Node::Nonlocal {
                    position,
                    names: vec![name],
                }
            } else {
                Node::Global {
                    position,
                    names: vec![name],
                }
            };
            nodes.insert(0, node);
        }
    }

    fn function_body(&mut self, body: &FunctionBody<'_>, params: &FormalParameters<'_>) -> LowerResult<Vec<ParseNode>> {
        let loops = mem::take(&mut self.loop_updates);
        let finally_depth = mem::take(&mut self.finally_depth);
        self.enter_function(params, Some(body));
        let mut nodes = self.rest_prologue(params)?;
        for statement in &body.statements {
            self.statement(statement, &mut nodes)?;
        }
        self.loop_updates = loops;
        self.finally_depth = finally_depth;
        self.leave_function(&mut nodes, self.range(body.span));
        Ok(nodes)
    }

    fn function_expression(&mut self, function: &Function<'_>) -> LowerResult<ExprLoc> {
        if function.generator || function.id.is_some() {
            return Err(unsupported("named function expressions and generators"));
        }
        let signature = self.parameters(&function.params)?;
        let body = function
            .body
            .as_ref()
            .ok_or_else(|| unsupported("function without body"))?;
        let body = self.function_body(body, &function.params)?;
        Ok(ExprLoc::new(
            self.range(function.span),
            Expr::FunctionRaw {
                name_id: self.interns.intern("<function>"),
                signature,
                body,
                is_async: function.r#async,
            },
        ))
    }

    fn assignment(
        &mut self,
        target: &AssignmentTarget<'_>,
        operator: &str,
        right: &Expression<'_>,
        nodes: &mut Vec<ParseNode>,
    ) -> LowerResult<()> {
        if let AssignmentTarget::AssignmentTargetIdentifier(name) = target {
            let target = self.user_identifier(name.name.as_str(), name.span)?;
            self.write(target);
            let value = self.expression(right)?;
            let value = self.assignment_value(Self::name(target), operator, value, name.span)?;
            nodes.push(Node::Assign { target, object: value });
            return Ok(());
        }
        let (object, key, span) = match target {
            AssignmentTarget::StaticMemberExpression(member) if !member.optional => {
                let object = self.expression(&member.object)?;
                (
                    object,
                    self.string(member.property.name.as_str(), member.property.span),
                    member.span,
                )
            }
            AssignmentTarget::ComputedMemberExpression(member) if !member.optional => {
                let object = self.expression(&member.object)?;
                (object, self.expression(&member.expression)?, member.span)
            }
            _ => return Err(unsupported("assignment target")),
        };
        // Preserve JS's receiver/key evaluation before the RHS, including augmented assignment.
        let obj = self.fresh(span);
        let prop = self.fresh(span);
        nodes.push(Node::Assign { target: obj, object });
        nodes.push(Node::Assign {
            target: prop,
            object: key,
        });
        let old = self.get(Self::name(obj), Self::name(prop), false, span);
        let value = self.expression(right)?;
        let value = self.assignment_value(old, operator, value, span)?;
        nodes.push(Node::Expr(self.helper(
            "set",
            vec![Self::name(obj), Self::name(prop), value],
            span,
        )));
        Ok(())
    }

    fn assignment_value(&mut self, left: ExprLoc, operator: &str, right: ExprLoc, span: Span) -> LowerResult<ExprLoc> {
        if operator == "=" {
            return Ok(right);
        }
        if operator == "+=" {
            return Ok(self.helper("add", vec![left, right], span));
        }
        let op = match operator {
            "-=" => Operator::Sub,
            "*=" => Operator::Mult,
            "/=" => Operator::Div,
            "%=" => Operator::Mod,
            _ => return Err(unsupported("assignment operator")),
        };
        Ok(ExprLoc::new(
            self.range(span),
            Expr::Op {
                left: Box::new(left),
                op,
                right: Box::new(right),
            },
        ))
    }

    fn range(&self, span: Span) -> CodeRange {
        CodeRange {
            filename: self.filename,
            start_byte: span.start,
            end_byte: span.end,
        }
    }

    fn identifier(&mut self, name: &str, span: Span) -> Identifier {
        Identifier::new(self.interns.intern(name), self.range(span))
    }

    fn binding(&mut self, binding: &BindingPattern<'_>) -> LowerResult<Identifier> {
        match binding {
            BindingPattern::BindingIdentifier(name) => self.user_identifier(name.name.as_str(), name.span),
            _ => Err(unsupported("destructuring bindings")),
        }
    }

    fn parameters(&mut self, params: &FormalParameters<'_>) -> LowerResult<ParsedSignature> {
        let mut args = Vec::new();
        for param in &params.items {
            let name = self.parameter_name(&param.pattern, param.span)?.name_id;
            if args.iter().any(|arg: &ParsedParam| arg.name == name) {
                return Err(unsupported("duplicate parameters"));
            }
            args.push(ParsedParam {
                name,
                default: Some(self.null(param.span)),
            });
        }
        Ok(ParsedSignature {
            args,
            var_args: Some(if let Some(rest) = &params.rest {
                self.binding(&rest.rest.argument)?.name_id
            } else {
                self.interns.intern("<js:extra>")
            }),
            ..ParsedSignature::default()
        })
    }

    fn suite(&mut self, statement: &Statement<'_>) -> LowerResult<Vec<ParseNode>> {
        let mut nodes = Vec::new();
        self.statement(statement, &mut nodes)?;
        Ok(nodes)
    }

    fn statement(&mut self, statement: &Statement<'_>, nodes: &mut Vec<ParseNode>) -> LowerResult<()> {
        let node = match statement {
            Statement::VariableDeclaration(declaration) => {
                self.declaration(declaration, nodes)?;
                return Ok(());
            }
            Statement::ExpressionStatement(statement) => {
                if let Expression::AssignmentExpression(assignment) = &statement.expression {
                    self.assignment(&assignment.left, assignment.operator.as_str(), &assignment.right, nodes)?;
                    return Ok(());
                }
                Node::Expr(self.expression(&statement.expression)?)
            }
            Statement::BlockStatement(block) => {
                for statement in &block.body {
                    self.statement(statement, nodes)?;
                }
                return Ok(());
            }
            Statement::FunctionDeclaration(function) => {
                if function.generator {
                    return Err(unsupported("generators"));
                }
                let name = function
                    .id
                    .as_ref()
                    .ok_or_else(|| unsupported("anonymous declarations"))?;
                let name = self.user_identifier(name.name.as_str(), name.span)?;
                let body = function
                    .body
                    .as_ref()
                    .ok_or_else(|| unsupported("function without a body"))?;
                let signature = self.parameters(&function.params)?;
                let body = self.function_body(body, &function.params)?;
                Node::FunctionDef {
                    def: RawFunctionDef {
                        name,
                        signature,
                        body,
                        is_async: function.r#async,
                    },
                    decorators: Vec::new(),
                }
            }
            Statement::ReturnStatement(statement) => {
                Node::Return(statement.argument.as_ref().map(|v| self.expression(v)).transpose()?)
            }
            Statement::IfStatement(statement) => Node::If {
                test: self.test(&statement.test)?,
                body: self.suite(&statement.consequent)?,
                or_else: statement
                    .alternate
                    .as_ref()
                    .map(|s| self.suite(s))
                    .transpose()?
                    .unwrap_or_default(),
            },
            Statement::WhileStatement(statement) => {
                let test = self.test(&statement.test)?;
                self.loop_updates.push(None);
                let body = self.suite(&statement.body)?;
                self.loop_updates.pop();
                Node::While {
                    test,
                    body,
                    or_else: Vec::new(),
                }
            }
            Statement::ForOfStatement(statement) if !statement.r#await => {
                let pattern = match &statement.left {
                    ForStatementLeft::VariableDeclaration(decl) if decl.declarations.len() == 1 => {
                        &decl.declarations[0].id
                    }
                    _ => return Err(unsupported("for-of target")),
                };
                let iter = self.expression(&statement.right)?;
                let temp = self.fresh(statement.span);
                self.loop_updates.push(None);
                let mut body = Vec::new();
                self.bind_pattern(pattern, Self::name(temp), &mut body)?;
                body.extend(self.suite(&statement.body)?);
                self.loop_updates.pop();
                Node::For {
                    target: UnpackTarget::Name(temp),
                    iter,
                    body,
                    or_else: Vec::new(),
                }
            }
            Statement::ForStatement(statement) => {
                if let Some(init) = &statement.init {
                    match init {
                        ForStatementInit::VariableDeclaration(decl) => self.declaration(decl, nodes)?,
                        init => {
                            nodes.push(Node::Expr(self.expression(init.to_expression())?));
                        }
                    }
                }
                let test = statement
                    .test
                    .as_ref()
                    .map(|e| self.test(e))
                    .transpose()?
                    .unwrap_or_else(|| ExprLoc::new(self.range(statement.span), Expr::Literal(Literal::Bool(true))));
                let update = statement.update.as_ref().map(|e| self.expression(e)).transpose()?;
                self.loop_updates.push(update.clone());
                let mut body = self.suite(&statement.body)?;
                self.loop_updates.pop();
                if let Some(update) = update {
                    body.push(Node::Expr(update));
                }
                Node::While {
                    test,
                    body,
                    or_else: Vec::new(),
                }
            }
            Statement::ThrowStatement(statement) => Node::Raise(Some(self.expression(&statement.argument)?)),
            Statement::TryStatement(statement) => {
                let has_finally = usize::from(statement.finalizer.is_some());
                self.finally_depth += has_finally;
                let mut body = Vec::new();
                for stmt in &statement.block.body {
                    self.statement(stmt, &mut body)?;
                }
                let mut handlers = Vec::new();
                if let Some(handler) = &statement.handler {
                    let name = handler.param.as_ref().map(|p| self.binding(&p.pattern)).transpose()?;
                    let mut body = Vec::new();
                    for stmt in &handler.body.body {
                        self.statement(stmt, &mut body)?;
                    }
                    handlers.push(ExceptHandler {
                        exc_type: None,
                        name,
                        body,
                    });
                }
                let mut finally = Vec::new();
                if let Some(finalizer) = &statement.finalizer {
                    for stmt in &finalizer.body {
                        self.statement(stmt, &mut finally)?;
                    }
                }
                self.finally_depth -= has_finally;
                Node::Try(Try {
                    body,
                    handlers,
                    or_else: Vec::new(),
                    finally,
                })
            }
            Statement::BreakStatement(statement) if statement.label.is_none() => Node::Break {
                position: self.range(statement.span),
            },
            Statement::ContinueStatement(statement) if statement.label.is_none() => {
                if let Some(Some(update)) = self.loop_updates.last() {
                    // The classic-for lowering cannot put its update after inlined finally cleanup.
                    if self.finally_depth > 0 {
                        return Err(unsupported("classic for continue across finally"));
                    }
                    nodes.push(Node::Expr(update.clone()));
                }
                Node::Continue {
                    position: self.range(statement.span),
                }
            }
            Statement::EmptyStatement(_) => Node::Pass,
            _ => return Err(unsupported("statement is outside the supported subset")),
        };
        nodes.push(node);
        Ok(())
    }

    fn string(&mut self, value: &str, span: Span) -> ExprLoc {
        ExprLoc::new(
            self.range(span),
            Expr::Literal(Literal::Str(self.interns.intern(value))),
        )
    }

    fn expression(&mut self, node: &Expression<'_>) -> LowerResult<ExprLoc> {
        let position = self.range(node.span());
        let expr = match node {
            Expression::NumericLiteral(value) => {
                if !value.value.is_finite() {
                    return Err(unsupported("non-finite numeric literals"));
                }
                // Keep small whole numbers in the shared integer representation.
                if value.value.fract() == 0.0 && value.value.abs() < 9_007_199_254_740_992.0 {
                    Expr::Literal(Literal::Int(value.value.to_i64().expect("bounded integral literal")))
                } else {
                    Expr::Literal(Literal::Float(value.value))
                }
            }
            Expression::BooleanLiteral(value) => Expr::Literal(Literal::Bool(value.value)),
            Expression::NullLiteral(_) => Expr::Literal(Literal::None),
            Expression::StringLiteral(value) => {
                return Ok(self.string(
                    value
                        .value
                        .as_str()
                        .ok_or_else(|| unsupported("lone UTF-16 surrogates"))?,
                    value.span,
                ));
            }
            Expression::Identifier(name) => {
                if name.name == "undefined" {
                    return Ok(self.null(name.span));
                }
                Expr::Name(self.user_identifier(name.name.as_str(), name.span)?)
            }
            Expression::ParenthesizedExpression(value) => return self.expression(&value.expression),
            Expression::ArrayExpression(array) => {
                let items = array
                    .elements
                    .iter()
                    .map(|item| match item {
                        ArrayExpressionElement::SpreadElement(spread) => {
                            self.expression(&spread.argument).map(SequenceItem::Unpack)
                        }
                        ArrayExpressionElement::Elision(hole) => Ok(SequenceItem::Value(self.null(hole.span))),
                        item => self.expression(item.to_expression()).map(SequenceItem::Value),
                    })
                    .collect::<LowerResult<Vec<_>>>()?;
                Expr::List(items)
            }
            Expression::BinaryExpression(binary) => {
                let left = Box::new(self.expression(&binary.left)?);
                let right = Box::new(self.expression(&binary.right)?);
                match binary.operator.as_str() {
                    "===" => Expr::Op {
                        left,
                        op: Operator::JsStrictEq,
                        right,
                    },
                    "!==" => {
                        let eq = ExprLoc::new(
                            position,
                            Expr::Op {
                                left,
                                op: Operator::JsStrictEq,
                                right,
                            },
                        );
                        Expr::Not(Box::new(eq))
                    }
                    "==" | "!=" | "<" | ">" | "<=" | ">=" => Expr::CmpOp {
                        left,
                        right,
                        op: match binary.operator.as_str() {
                            "==" => CmpOperator::Eq,
                            "!=" => CmpOperator::NotEq,
                            "<" => CmpOperator::Lt,
                            ">" => CmpOperator::Gt,
                            "<=" => CmpOperator::LtE,
                            _ => CmpOperator::GtE,
                        },
                    },
                    "+" => return Ok(self.helper("add", vec![*left, *right], binary.span)),
                    operator => Expr::Op {
                        left,
                        right,
                        op: match operator {
                            "+" => Operator::Add,
                            "-" => Operator::Sub,
                            "*" => Operator::Mult,
                            "/" => Operator::Div,
                            "%" => Operator::Mod,
                            "**" => Operator::Pow,
                            "&" => Operator::BitAnd,
                            "|" => Operator::BitOr,
                            "^" => Operator::BitXor,
                            _ => return Err(unsupported("binary operator")),
                        },
                    },
                }
            }
            Expression::LogicalExpression(logical) => {
                let left = self.expression(&logical.left)?;
                let temp = self.fresh(logical.span);
                let test_value = ExprLoc::new(
                    position,
                    Expr::Named {
                        target: temp,
                        value: Box::new(left),
                    },
                );
                let right = self.expression(&logical.right)?;
                let value = Self::name(temp);
                match logical.operator.as_str() {
                    "??" => Expr::IfElse {
                        test: Box::new(Self::null_test(test_value)),
                        body: Box::new(right),
                        orelse: Box::new(value),
                    },
                    "&&" => Expr::IfElse {
                        test: Box::new(self.helper("truth", vec![test_value], logical.span)),
                        body: Box::new(right),
                        orelse: Box::new(value),
                    },
                    _ => Expr::IfElse {
                        test: Box::new(self.helper("truth", vec![test_value], logical.span)),
                        body: Box::new(value),
                        orelse: Box::new(right),
                    },
                }
            }
            Expression::UnaryExpression(unary) => {
                if unary.operator.as_str() == "typeof" {
                    if matches!(&unary.argument, Expression::Identifier(name) if name.name == "undefined") {
                        return Ok(self.string("undefined", unary.span));
                    }
                    let arg = self.expression(&unary.argument)?;
                    return Ok(self.helper("typeof", vec![arg], unary.span));
                }
                let arg = Box::new(self.expression(&unary.argument)?);
                match unary.operator.as_str() {
                    "!" => Expr::Not(Box::new(self.helper("truth", vec![*arg], unary.span))),
                    "-" => Expr::UnaryMinus(arg),
                    "+" => return Ok(self.helper("number", vec![*arg], unary.span)),
                    "~" => Expr::UnaryInvert(arg),
                    _ => return Err(unsupported("unary operator")),
                }
            }
            Expression::ConditionalExpression(conditional) => Expr::IfElse {
                test: Box::new(self.test(&conditional.test)?),
                body: Box::new(self.expression(&conditional.consequent)?),
                orelse: Box::new(self.expression(&conditional.alternate)?),
            },
            Expression::ArrowFunctionExpression(arrow) => {
                let signature = self.parameters(&arrow.params)?;
                let body = match &arrow.body {
                    ArrowFunctionBody::FunctionBody(body) => self.function_body(body, &arrow.params)?,
                    body => {
                        self.enter_function(&arrow.params, None);
                        let loops = mem::take(&mut self.loop_updates);
                        let finally_depth = mem::take(&mut self.finally_depth);
                        let mut nodes = self.rest_prologue(&arrow.params)?;
                        nodes.push(Node::Return(Some(self.expression(body.to_expression())?)));
                        self.loop_updates = loops;
                        self.finally_depth = finally_depth;
                        self.leave_function(&mut nodes, position);
                        nodes
                    }
                };
                Expr::FunctionRaw {
                    name_id: self.interns.intern("<arrow>"),
                    signature,
                    body,
                    is_async: arrow.r#async,
                }
            }
            Expression::FunctionExpression(function) => return self.function_expression(function),
            Expression::ObjectExpression(object) => {
                let mut items = Vec::new();
                for property in &object.properties {
                    match property {
                        ObjectPropertyKind::SpreadProperty(spread) => {
                            items.push(DictItem::Unpack(self.expression(&spread.argument)?));
                        }
                        ObjectPropertyKind::ObjectProperty(property)
                            if property.kind == PropertyKind::Init && !property.method =>
                        {
                            let key = self.property_key(&property.key, property.computed)?;
                            let value = self.expression(&property.value)?;
                            items.push(DictItem::Pair(key, value));
                        }
                        ObjectPropertyKind::ObjectProperty(_) => {
                            return Err(unsupported("object methods and accessors"));
                        }
                    }
                }
                Expr::Dict(items)
            }
            Expression::ComputedMemberExpression(member) => {
                let object = self.expression(&member.object)?;
                let key = self.expression(&member.expression)?;
                return Ok(self.get(object, key, member.optional, member.span));
            }
            Expression::StaticMemberExpression(member) => {
                let object = self.expression(&member.object)?;
                let key = self.string(member.property.name.as_str(), member.property.span);
                return Ok(self.get(object, key, member.optional, member.span));
            }
            Expression::ChainExpression(chain) => {
                return match &chain.expression {
                    ChainElement::StaticMemberExpression(member) => {
                        let object = self.expression(&member.object)?;
                        let key = self.string(member.property.name.as_str(), member.property.span);
                        Ok(self.get(object, key, member.optional, member.span))
                    }
                    ChainElement::ComputedMemberExpression(member) => {
                        let object = self.expression(&member.object)?;
                        let key = self.expression(&member.expression)?;
                        Ok(self.get(object, key, member.optional, member.span))
                    }
                    ChainElement::CallExpression(call) => self.call(call),
                    _ => Err(unsupported("optional chain")),
                };
            }
            Expression::CallExpression(call) => return self.call(call),
            Expression::NewExpression(call) => {
                let Expression::Identifier(name) = &call.callee else {
                    return Err(unsupported("constructor"));
                };
                if !["Error", "TypeError", "RangeError", "Set", "Map", "Array"].contains(&name.name.as_str()) {
                    return Err(unsupported("constructor"));
                }
                let args = call
                    .arguments
                    .iter()
                    .map(|arg| match arg {
                        Argument::SpreadElement(spread) => self.expression(&spread.argument).map(CallArg::Unpack),
                        arg => self.expression(arg.to_expression()).map(CallArg::Value),
                    })
                    .collect::<LowerResult<Vec<_>>>()?;
                let callee = if name.name == "Array" {
                    self.identifier("_js_array", name.span)
                } else {
                    self.user_identifier(name.name.as_str(), name.span)?
                };
                Expr::Call {
                    callable: Callable::Name(callee),
                    args: Box::new(ArgExprs::new_generalized(args, Vec::new())),
                }
            }
            Expression::TemplateLiteral(template) => {
                let mut parts = Vec::new();
                for (i, quasi) in template.quasis.iter().enumerate() {
                    let text = quasi
                        .value
                        .cooked
                        .as_ref()
                        .and_then(|s| s.as_str())
                        .ok_or_else(|| unsupported("invalid template escape or lone surrogate"))?;
                    parts.push(SequenceItem::Value(self.string(text, quasi.span)));
                    if let Some(expr) = template.expressions.get(i) {
                        let value = self.expression(expr)?;
                        parts.push(SequenceItem::Value(self.helper("string", vec![value], expr.span())));
                    }
                }
                let list = ExprLoc::new(position, Expr::List(parts));
                Expr::AttrCall {
                    object: Box::new(self.string("", template.span)),
                    attr: EitherStr::Interned(self.interns.intern("join")),
                    args: Box::new(ArgExprs::One(list)),
                }
            }
            Expression::UpdateExpression(update) => {
                let target = match &update.argument {
                    SimpleAssignmentTarget::AssignmentTargetIdentifier(name) => {
                        self.user_identifier(name.name.as_str(), name.span)?
                    }
                    _ => return Err(unsupported("update target")),
                };
                self.write(target);
                let increment = update.operator.as_str() == "++";
                let value = Self::name(target);
                let value = self.helper("number", vec![value], update.span);
                let one = ExprLoc::new(position, Expr::Literal(Literal::Int(1)));
                let assigned = ExprLoc::new(
                    position,
                    Expr::Named {
                        target,
                        value: Box::new(ExprLoc::new(
                            position,
                            Expr::Op {
                                left: Box::new(value),
                                op: if increment { Operator::Add } else { Operator::Sub },
                                right: Box::new(one.clone()),
                            },
                        )),
                    },
                );
                if update.prefix {
                    return Ok(assigned);
                }
                Expr::Op {
                    left: Box::new(assigned),
                    op: if increment { Operator::Sub } else { Operator::Add },
                    right: Box::new(one),
                }
            }
            Expression::AssignmentExpression(assignment) => {
                let AssignmentTarget::AssignmentTargetIdentifier(name) = &assignment.left else {
                    return Err(unsupported("assignment expression target"));
                };
                let target = self.user_identifier(name.name.as_str(), name.span)?;
                self.write(target);
                let right = self.expression(&assignment.right)?;
                let value =
                    self.assignment_value(Self::name(target), assignment.operator.as_str(), right, assignment.span)?;
                Expr::Named {
                    target,
                    value: Box::new(value),
                }
            }
            Expression::AwaitExpression(value) => Expr::Await(Box::new(self.expression(&value.argument)?)),
            _ => return Err(unsupported("expression is outside the supported subset")),
        };
        Ok(ExprLoc::new(position, expr))
    }
}
