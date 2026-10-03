#[cfg(test)]
use super::*;

const SPECS: [ParamSpec; 2] = [
    ParamSpec {
        name: "niter",
        kind: ParamKind::Int,
        min: 1.0,
        max: 10.0,
        default: 5.0,
        step: 1.0,
        doc: "iterations",
    },
    ParamSpec {
        name: "scale",
        kind: ParamKind::Float,
        min: 0.0,
        max: 100.0,
        default: 1.5,
        step: 0.5,
        doc: "extent",
    },
];

fn buffer(values: &[f64]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .collect::<Vec<u8>>()
}

#[test]
fn the_defaults_are_a_buffer_the_rule_accepts() {
    let view = ParamsView::new(&SPECS);
    let bytes = buffer(&view.defaults());
    assert_eq!(view.buffer_len(), 16);
    assert_eq!(view.values(&bytes), Ok(vec![5.0, 1.5]));
}

#[test]
fn out_of_range_non_finite_and_non_integral_are_all_refused() {
    let view = ParamsView::new(&SPECS);
    for (bad, index) in [
        (buffer(&[0.0, 1.5]), 0),
        (buffer(&[11.0, 1.5]), 0),
        (buffer(&[5.5, 1.5]), 0),
        (buffer(&[f64::NAN, 1.5]), 0),
        (buffer(&[f64::INFINITY, 1.5]), 0),
        (buffer(&[5.0, 100.5]), 1),
        (buffer(&[5.0, -0.5]), 1),
    ] {
        assert_eq!(
            view.values(&bad),
            Err(ParamsError::OutOfRange { index }),
            "{bad:?}"
        );
    }
}

#[test]
fn the_two_ends_of_the_range_are_inside_it() {
    let view = ParamsView::new(&SPECS);
    assert!(view.validate(&buffer(&[1.0, 0.0])).is_ok());
    assert!(view.validate(&buffer(&[10.0, 100.0])).is_ok());
}

#[test]
fn a_bool_is_a_flag_and_an_int_is_whole() {
    const FLAG: [ParamSpec; 1] = [ParamSpec {
        name: "on",
        kind: ParamKind::Bool,
        min: 0.0,
        max: 1.0,
        default: 0.0,
        step: 1.0,
        doc: "flag",
    }];
    let view = ParamsView::new(&FLAG);
    assert!(view.validate(&buffer(&[1.0])).is_ok());
    assert!(view.validate(&buffer(&[0.5])).is_err());
}

#[test]
fn the_wrong_length_is_malformed_and_an_empty_layout_takes_nothing() {
    let view = ParamsView::new(&SPECS);
    assert_eq!(view.validate(&[]), Err(ParamsError::Malformed));
    assert_eq!(view.validate(&[0u8; 8]), Err(ParamsError::Malformed));
    assert_eq!(view.validate(&[0u8; 24]), Err(ParamsError::Malformed));
    let none = ParamsView::new(&[]);
    assert!(none.validate(&[]).is_ok());
    assert_eq!(none.validate(&[0u8; 8]), Err(ParamsError::NotAccepted));
}

#[test]
fn the_schema_round_trips_through_its_own_encoding() {
    let view = ParamsView::new(&SPECS);
    let bytes = view.encode();
    let mut at = 4;
    let count = u32::from_le_bytes(bytes[at - 4..at].try_into().unwrap()) as usize;
    assert_eq!(count, 2);
    for spec in SPECS {
        let (name, next) = take_str(&bytes, at);
        assert_eq!(name, spec.name);
        at = next;
        assert_eq!(bytes[at], spec.kind.tag());
        at += 1;
        for want in [spec.min, spec.max, spec.default, spec.step] {
            let mut word = [0u8; 8];
            word.copy_from_slice(&bytes[at..at + 8]);
            assert_eq!(f64::from_le_bytes(word), want);
            at += 8;
        }
        let (doc, next) = take_str(&bytes, at);
        assert_eq!(doc, spec.doc);
        at = next;
    }
    assert_eq!(at, bytes.len(), "the encoding has no trailing bytes");
}

#[test]
fn every_kind_tag_round_trips_and_an_unknown_tag_is_refused() {
    for kind in [ParamKind::Int, ParamKind::Float, ParamKind::Bool] {
        assert_eq!(ParamKind::from_tag(kind.tag()), Some(kind));
    }
    assert_eq!(ParamKind::from_tag(3), None);
}

fn take_str(bytes: &[u8], at: usize) -> (&str, usize) {
    let len = u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
    let text = std::str::from_utf8(&bytes[at + 4..at + 4 + len]).expect("utf-8");
    (text, at + 4 + len)
}
