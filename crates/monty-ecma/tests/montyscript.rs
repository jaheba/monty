//! Deliberate MontyScript differences from ECMAScript use contract tests, not Node.

use monty_ecma::{JsDump, JsProgress, JsRepl, JsSessionRef, dump};
use monty_types::{MontyObject, NamedValues, PrintWriter, ResourceLimits};

fn evaluate(code: &str) -> MontyObject {
    let repl = JsRepl::new("contract.js", ResourceLimits::default()).unwrap();
    match repl.feed(code, &NamedValues::new(), PrintWriter::Disabled) {
        JsProgress::Complete { value, .. } => value,
        JsProgress::Failed { error, .. } => panic!("{}", error.summary()),
        JsProgress::Suspended(_) => panic!("unexpected suspension"),
    }
}

#[test]
fn scalar_length_indexing_slicing_and_methods_agree() {
    assert_eq!(evaluate("'a😀é'.length"), MontyObject::int(3));
    assert_eq!(evaluate("'a😀é'[1]"), MontyObject::string("😀"));
    assert_eq!(evaluate("'a😀é'.slice(1, 2)"), MontyObject::string("😀"));
    assert_eq!(evaluate("'a😀é'.substring(1, 2)"), MontyObject::string("😀"));
    assert_eq!(evaluate("'a😀é'.indexOf('é')"), MontyObject::int(2));
    assert_eq!(evaluate("'😀'.charCodeAt(0)"), MontyObject::int(128_512));
    assert_eq!(evaluate("'😀'.codePointAt(0)"), MontyObject::int(128_512));
    assert_eq!(evaluate("'a😀é'.split('').join('|')"), MontyObject::string("a|😀|é"));
    assert_eq!(evaluate("[...'a😀é'].join('|')"), MontyObject::string("a|😀|é"));
    assert_eq!(evaluate("'e\\u0301'.length"), MontyObject::int(2));
    assert_eq!(evaluate("'e\\u0301'.normalize()"), MontyObject::string("é"));
}

#[test]
fn regex_ranges_last_index_and_replacement_use_scalar_offsets() {
    assert_eq!(evaluate("/^.$/.test('😀')"), MontyObject::bool(true));
    assert_eq!(evaluate("'😀'.match(/./g).length"), MontyObject::int(1));
    assert_eq!(evaluate("'😀x😀'.split(/(?:)/).length"), MontyObject::int(3));
    assert_eq!(evaluate("/é/.exec('😀é').index"), MontyObject::int(1));
    assert_eq!(evaluate("'😀é'.search(/é/)"), MontyObject::int(1));
    assert_eq!(evaluate("'😀é'.match(/./g).join('|')"), MontyObject::string("😀|é"));
    assert_eq!(
        evaluate("let re = /./g; re.exec('😀é'); re.lastIndex"),
        MontyObject::int(1)
    );
    assert_eq!(
        evaluate("let re = /./g; re.lastIndex = 1; re.exec('😀é')[0]"),
        MontyObject::string("é")
    );
    assert_eq!(
        evaluate("'😀é'.replace(/é/, (text, i) => String(i))"),
        MontyObject::string("😀1")
    );
    assert_eq!(
        evaluate("/é/d.exec('😀é').indices[0].join(',')"),
        MontyObject::string("1,2")
    );
}

#[test]
fn surrogate_pairs_decode_and_lone_surrogates_raise_errors() {
    assert_eq!(evaluate(r"('\uD83D\uDE00')"), MontyObject::string("😀"));
    assert_eq!(evaluate(r#"JSON.parse('"\\ud83d\\ude00"')"#), MontyObject::string("😀"));
    assert_eq!(
        evaluate("String.fromCharCode(0xd83d, 0xde00)"),
        MontyObject::string("😀")
    );
    for code in [
        r"'\ud800'",
        r"'\udfff'",
        r"`\ud800`",
        r"({ '\ud800': 1 })",
        r#"JSON.parse('"\\ud800"')"#,
        "String.fromCharCode(0xd800)",
    ] {
        let repl = JsRepl::new("contract.js", ResourceLimits::default()).unwrap();
        let JsProgress::Failed { error, .. } = repl.feed(code, &NamedValues::new(), PrintWriter::Disabled) else {
            panic!("expected a surrogate error for {code}");
        };
        assert!(error.summary().contains("surrogate"), "{}", error.summary());
    }
}

#[test]
fn host_values_and_dumps_preserve_scalar_strings() {
    let mut inputs = NamedValues::new();
    inputs.push("text", MontyObject::string("a😀é"));
    let repl = JsRepl::new("contract.js", ResourceLimits::default()).unwrap();
    let JsProgress::Complete { repl, .. } = repl.feed("text", &inputs, PrintWriter::Disabled) else {
        panic!("expected imported text");
    };
    let bytes = dump(JsSessionRef::Idle(&repl)).unwrap();
    let JsDump::Idle(repl) = JsDump::load(&bytes).unwrap() else {
        panic!("expected idle dump");
    };
    let JsProgress::Complete { value, .. } = repl.feed("text.slice(1, 2)", &NamedValues::new(), PrintWriter::Disabled)
    else {
        panic!("expected restored text");
    };
    assert_eq!(value, MontyObject::string("😀"));
}

#[test]
fn previous_string_representation_dump_is_rejected() {
    let repl = JsRepl::new("contract.js", ResourceLimits::default()).unwrap();
    let mut bytes = dump(JsSessionRef::Idle(&repl)).unwrap();
    let version_offset = b"MONTYJS\0".len();
    bytes[version_offset..version_offset + 2].copy_from_slice(&1u16.to_le_bytes());
    let Err(error) = JsDump::load(&bytes) else {
        panic!("old dump must be rejected");
    };
    assert!(error.contains("version 1 is not supported"), "{error}");
}
