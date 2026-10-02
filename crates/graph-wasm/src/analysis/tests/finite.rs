//! The JSON face is JSON or it is refused: a non-finite score or modularity (R6, F-95)
//! and an id needing escapes (F-97) over hand-built reports, since no registered analysis
//! can be made to emit either on demand.

use crate::analysis::{Column, Report};
use graph_contract::canonical_json::{Value, parse};

fn report(id: &'static str, values: Column, modularity: Option<f64>) -> Report {
    Report {
        id,
        values,
        converged: None,
        modularity,
        max: None,
    }
}

/// The face as the export would publish it, or `None` where the writer refuses.
fn written(report: &Report) -> Option<String> {
    report.to_json()
}

#[test]
fn a_non_finite_score_is_refused_never_written_as_json() {
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let text = written(&report("analysis.x", Column::F64(vec![0.5, bad]), None));
        assert_eq!(text, None, "{bad} is not a JSON number");
    }
}

#[test]
fn a_non_finite_modularity_is_refused_never_written_as_json() {
    let text = written(&report(
        "analysis.x",
        Column::U32(vec![0, 1]),
        Some(f64::NAN),
    ));
    assert_eq!(text, None, "NaN modularity is not a JSON number");
}

#[test]
fn an_id_is_escaped_into_a_json_string() {
    let id = "a\"b\\c\n";
    let text = written(&report(id, Column::U32(Vec::new()), None)).expect("finite");
    let Ok(Value::Object(members)) = parse(&text) else {
        panic!("not JSON: {text}");
    };
    let read = members.iter().find(|(key, _)| key == "id").map(|(_, v)| v);
    assert_eq!(read, Some(&Value::String(id.to_owned())), "{text}");
}
