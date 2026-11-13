//! Bounded annotation hints retained separately from Python execution semantics.

use ruff_python_ast::{self as ast, Expr};
use ruff_text_size::Ranged;

use crate::intern::StringId;

/// An annotation can suggest a fast path; its runtime value must still be guarded.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TypeHint {
    #[default]
    #[serde(rename = "U")]
    Unknown,
    #[serde(rename = "I")]
    Int,
    #[serde(rename = "F")]
    Float,
    #[serde(rename = "B")]
    Bool,
    #[serde(rename = "N")]
    None,
    #[serde(rename = "S")]
    Str,
    #[serde(rename = "L")]
    List(Box<Self>),
    #[serde(rename = "T")]
    Tuple(Box<Self>),
    #[serde(rename = "D")]
    Dict {
        #[serde(rename = "K")]
        key: Box<Self>,
        #[serde(rename = "V")]
        value: Box<Self>,
    },
    #[serde(rename = "O")]
    Union(Vec<Self>),
}

impl TypeHint {
    pub(crate) fn from_annotation(annotation: Option<&Expr>) -> Self {
        annotation.map_or(Self::Unknown, |annotation| {
            if annotation.range().len().to_u32() > 512 {
                return Self::Unknown;
            }
            Self::parse(annotation, 0, &mut 64)
        })
    }

    fn from_name(name: &str) -> Self {
        match name.trim() {
            "int" => Self::Int,
            "float" => Self::Float,
            "bool" => Self::Bool,
            "None" => Self::None,
            "str" => Self::Str,
            _ => Self::Unknown,
        }
    }

    fn parse(annotation: &Expr, depth: usize, remaining: &mut usize) -> Self {
        if depth >= 16 || *remaining == 0 {
            return Self::Unknown;
        }
        *remaining -= 1;
        match annotation {
            Expr::Name(name) => Self::from_name(name.id.as_str()),
            Expr::StringLiteral(value) => Self::from_name(&value.value.to_string()),
            Expr::NoneLiteral(_) => Self::None,
            Expr::Subscript(subscript) => {
                let name = match subscript.value.as_ref() {
                    Expr::Name(name) => name.id.as_str(),
                    Expr::Attribute(attribute) => attribute.attr.id(),
                    _ => return Self::Unknown,
                };
                match (name, subscript.slice.as_ref()) {
                    ("list" | "List", element) => Self::List(Box::new(Self::parse(element, depth + 1, remaining))),
                    ("tuple" | "Tuple", Expr::Tuple(tuple))
                        if tuple.elts.len() == 2 && matches!(tuple.elts[1], Expr::EllipsisLiteral(_)) =>
                    {
                        Self::Tuple(Box::new(Self::parse(&tuple.elts[0], depth + 1, remaining)))
                    }
                    ("dict" | "Dict", Expr::Tuple(tuple)) if tuple.elts.len() == 2 => Self::Dict {
                        key: Box::new(Self::parse(&tuple.elts[0], depth + 1, remaining)),
                        value: Box::new(Self::parse(&tuple.elts[1], depth + 1, remaining)),
                    },
                    ("Optional", element) => Self::Union(vec![Self::parse(element, depth + 1, remaining), Self::None]),
                    _ => Self::Unknown,
                }
            }
            Expr::BinOp(binary) if binary.op == ast::Operator::BitOr => Self::Union(vec![
                Self::parse(&binary.left, depth + 1, remaining),
                Self::parse(&binary.right, depth + 1, remaining),
            ]),
            _ => Self::Unknown,
        }
    }
}

/// Local layout and source hints persisted so restored code can rebuild its optimized form.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct FunctionHints {
    #[serde(rename = "P")]
    pub parameters: Vec<TypeHint>,
    #[serde(rename = "R")]
    pub returns: TypeHint,
    #[serde(rename = "L")]
    pub locals: u16,
    #[serde(rename = "B", default)]
    pub builtin_names: Vec<(StringId, BuiltinHint)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum BuiltinHint {
    #[serde(rename = "R")]
    Range,
    #[serde(rename = "L")]
    Len,
}

impl BuiltinHint {
    pub(crate) fn from_name(name: &str) -> Option<Self> {
        match name {
            "range" => Some(Self::Range),
            "len" => Some(Self::Len),
            _ => None,
        }
    }
}

impl FunctionHints {
    pub(crate) fn is_useful(&self) -> bool {
        self.returns != TypeHint::Unknown || self.parameters.iter().any(|hint| *hint != TypeHint::Unknown)
    }
}
