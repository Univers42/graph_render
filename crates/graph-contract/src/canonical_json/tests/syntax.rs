use super::*;

#[test]
fn every_json_syntax_fault_is_refused_with_its_offset() {
    let fault = |text: &str| match parse::parse(text) {
        Err(JsonError::Syntax { at, what }) => (at, what),
        other => panic!("{text:?}: {other:?}"),
    };
    assert_eq!(fault(""), (0, "the text ends where a value should be"));
    assert_eq!(fault("[1,]"), (3, "not the start of a value"));
    assert_eq!(fault("[01]"), (2, "expected , or ] in an array"));
    assert_eq!(fault("-"), (1, "a number needs digits"));
    assert_eq!(fault("1."), (2, "a fraction needs digits"));
    assert_eq!(fault("1e+"), (3, "an exponent needs digits"));
    assert_eq!(
        fault("{\"a\":1,\"a\":2}"),
        (7, "a key repeated in one object")
    );
    assert_eq!(fault("{1:2}"), (1, "expected a key"));
    assert_eq!(fault("{\"a\" 1}"), (5, "expected : after a key"));
    assert_eq!(
        fault("{\"a\":1 \"b\":2}"),
        (7, "expected , or } in an object")
    );
    assert_eq!(
        fault("\"a\u{1}\""),
        (2, "a raw control character in a string")
    );
    assert_eq!(fault("\"abc"), (4, "the text ends inside a string"));
    assert_eq!(fault("\"\\x\""), (3, "an unknown escape"));
    assert_eq!(fault("\"\\u12\""), (3, "\\u needs four hex digits"));
    assert_eq!(fault("\"\\ud83d\""), (7, "an unpaired surrogate"));
    assert_eq!(fault("\"\\ud83d\\u0041\""), (13, "an unpaired surrogate"));
    assert_eq!(fault("\"\\ude80\""), (7, "an unpaired surrogate"));
    assert_eq!(fault("tru"), (0, "not the start of a value"));
    assert_eq!(fault("NaN"), (0, "not the start of a value"));
    assert_eq!(fault("1 2"), (2, "text after the value"));
    let deep = "[".repeat(parse::MAX_DEPTH as usize + 2);
    assert_eq!(fault(&deep), (parse::MAX_DEPTH + 1, "nested too deep"));
}

#[test]
fn the_reader_keeps_every_json_value_it_accepts() {
    let value = parse::parse(
        " {\"n\":null,\"t\":true,\"f\":false,\"x\":-0.5E-3,\"s\":\"\\b\\f\\n\\r\\t\\/\\\\\\\"\",\"a\":[[]],\"o\":{}} ",
    );
    let expected = Value::Object(vec![
        ("n".into(), Value::Null),
        ("t".into(), Value::Bool(true)),
        ("f".into(), Value::Bool(false)),
        ("x".into(), Value::Number("-0.5E-3".into())),
        ("s".into(), Value::String("\u{8}\u{c}\n\r\t/\\\"".into())),
        ("a".into(), Value::Array(vec![Value::Array(vec![])])),
        ("o".into(), Value::Object(vec![])),
    ]);
    assert_eq!(value, Ok(expected));
    let deep = format!(
        "{}{}",
        "[".repeat(parse::MAX_DEPTH as usize + 1),
        "]".repeat(parse::MAX_DEPTH as usize + 1)
    );
    assert!(parse::parse(&deep).is_ok(), "MAX_DEPTH itself is read");
}
