//! What the stream stage holds natively, with no Node in the loop: that growing a session in
//! place is the same bytes as rebuilding the whole graph and carrying onto it, at every
//! batch of every fixture; that the stream comparison names the batch it diverged on and
//! refuses an arm it cannot compare; and that the Node arm's script cannot drift from this
//! crate's constants.
//!
//! The cross-target comparison is the gate command's own work. What is checked here is what
//! it rests on, so a failure names which half is wrong instead of reporting "diverged".

use super::*;
use crate::hashgate::knob::setting::setting;
use graph_core::Topology;
use graph_core::layout::force::LiveParams;
use serde_json::{Value, json};
use std::env::VarError;

/// The honest setting: no knob set, so `live_force_params` is the frozen set.
fn honest() -> Setting {
    setting(|_| Err(VarError::NotPresent)).expect("no knobs set is not an error")
}

/// [`Setting`] for a variable only known at run time.
fn setting_for(name: &'static str, value: &'static str) -> Result<Setting, String> {
    setting(|read| match read == name {
        true => Ok(value.to_owned()),
        false => Err(VarError::NotPresent),
    })
}

/// One arm from the text an arm printed, which is what [`Arm`] holds: the lines, without
/// their newlines, exactly as `run_lines` reads them off a child's stdout.
fn arm(name: &'static str, text: String) -> Arm {
    (
        name,
        text.lines().map(str::to_owned).collect::<Vec<String>>(),
    )
}

/// `documents[..up_to]` as **one** document, merged — what a consumer that never grew a
/// graph would have read. Test-only on purpose: the ABI has no way to express this, which is
/// the whole difference between the two arms.
fn merged(documents: &[Vec<u8>], up_to: usize) -> Vec<u8> {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    for document in &documents[..up_to] {
        let value: Value = serde_json::from_slice(document).expect("a JSON document");
        nodes.extend(value["nodes"].as_array().expect("nodes").clone());
        edges.extend(value["edges"].as_array().expect("edges").clone());
    }
    serde_json::to_vec(&json!({ "version": 1, "nodes": nodes, "edges": edges })).expect("writes")
}

fn build(document: &[u8]) -> Topology {
    service::build(document, Source::Ingest).expect("a fixture line builds")
}

/// The two position columns' `f64` bits, which is the only comparison that can tell two
/// sessions apart when they agree to within a rounding error.
fn bits(session: &ForceSession) -> (Vec<u64>, Vec<u64>) {
    (
        session.xs().iter().map(|v| v.to_bits()).collect(),
        session.ys().iter().map(|v| v.to_bits()).collect(),
    )
}

/// **The native-only check the stage exists to justify.** For every batch of every fixture,
/// a session grown in place must hold exactly the bits a session built over the whole graph
/// and carried onto it holds — twice: once straight after the grow, and once after the next
/// [`TICKS`] ticks, because a grow that agreed only before the tick would move the divergence
/// into the simulation and read as a force bug.
///
/// `carry` leaves `session` untouched, so the pre-batch topology and the pre-batch session are
/// both still available when the two comparisons run.
#[test]
fn every_grown_session_equals_a_rebuild_carried() {
    let params = LiveParams::default();
    for name in FIXTURES {
        let documents = documents(name).expect("fixtures are emitted");
        let mut topology = build(&documents[0]);
        let mut session = ForceSession::new(&topology, params).expect("in range");
        session.step(TICKS);
        for (batch, document) in documents.iter().enumerate().skip(1) {
            let before = build(&merged(&documents, batch));
            let rebuilt = build(&merged(&documents, batch + 1));
            // Taken *before* the grow, because `carry` is a method of the session that is
            // over `before` — it refuses a `from` whose row count is not its own.
            let mut carried = session.carry(&before, &rebuilt).expect("the carry");
            service::extend(&mut topology, document).expect("the batch extends");
            session.grow(&topology).expect("the session grows");
            assert_eq!(
                bits(&carried),
                bits(&session),
                "{name} batch {batch}: grow and carry must agree before the tick"
            );
            carried.step(TICKS);
            session.step(TICKS);
            assert_eq!(
                bits(&carried),
                bits(&session),
                "{name} batch {batch}: grow and carry must agree after {TICKS} ticks"
            );
        }
    }
}

/// A stage that compared nothing would also report equality, so the fixtures must actually be
/// read: every one of them, and every batch of each, through the real ABI reader.
#[test]
fn every_fixture_reaches_the_stage_with_its_batches() {
    let expected = [
        ("stream-small", 9usize),
        ("stream-hub", 6),
        ("stream-pow2", 6),
    ];
    for (name, lines) in expected {
        assert_eq!(documents(name).expect("emitted").len(), lines, "{name}");
    }
    let honest_lines = arm_lines(&honest(), Route::Json).expect("the honest run");
    for name in FIXTURES {
        for batch in 0..documents(name).expect("emitted").len() {
            assert!(
                honest_lines.contains(&format!("{STREAM_STAGE} {name} {batch} ")),
                "{name} batch {batch} is missing from the native arm's output"
            );
        }
    }
}

/// **The columns path's own judge.** The stage compares the columns arm against the others per
/// batch, and this is that comparison run natively over every fixture: the two appends must
/// print the same digest on every batch, which is byte-identity of the two paths' topologies
/// after each append — the one thing a divergence here could hide.
#[test]
fn the_columns_arm_agrees_with_the_json_arm_on_every_batch() {
    let json = arm_lines(&honest(), Route::Json).expect("the json arm runs");
    let columns = arm_lines(&honest(), Route::Columns).expect("the columns arm runs");
    assert_eq!(
        columns, json,
        "the two arms must print the same digest for every fixture and batch"
    );
    assert!(
        !json.is_empty(),
        "and they printed something: a stage that read no fixture would agree on nothing"
    );
}

/// The route is chosen by name, and anything else is refused rather than defaulted: an arm that
/// silently became the JSON one would make the stage's comparison vacuous.
#[test]
fn an_unknown_route_is_refused_by_name() {
    assert_eq!(parse_route(None), Ok(Route::Json));
    assert_eq!(parse_route(Some("json")), Ok(Route::Json));
    assert_eq!(parse_route(Some("columns")), Ok(Route::Columns));
    let err = parse_route(Some("yaml")).expect_err("yaml is not a reader");
    assert!(err.contains(ROUTE_ENV), "{err}");
    assert!(err.contains("yaml"), "{err}");
}

/// The comparison's own contract. Equal arms are `None`; a changed line is `Some` naming the
/// fixture and the batch; a ragged arm is `Err`, because a short arm is an arm that could not
/// run rather than one that agreed.
#[test]
fn first_divergence_names_the_batch_and_refuses_a_ragged_arm() {
    let equal: [Arm; 2] = [
        ("native run 1", vec!["a 0 0".to_owned()]),
        ("native run 2", vec!["a 0 0".to_owned()]),
    ];
    assert_eq!(first_divergence(&equal), Ok(None));

    let good = format!("{STREAM_STAGE} stream-small 2 {}", "0".repeat(64));
    let changed = format!("{STREAM_STAGE} stream-small 2 {}", "1".repeat(64));
    let arms: [Arm; 2] = [
        ("native run 1", vec![good.clone()]),
        ("wasm32 run 1", vec![changed]),
    ];
    assert_eq!(
        first_divergence(&arms),
        Ok(Some(
            "stream-small batch 2 (wasm32 run 1 differs from native run 1)".to_owned()
        ))
    );

    let short: [Arm; 2] = [
        ("native run 1", vec![good.clone(), good.clone()]),
        ("wasm32 run 1", vec![good.clone()]),
    ];
    let err = first_divergence(&short).expect_err("a short arm cannot be compared");
    assert!(err.contains("wasm32 run 1"), "{err}");
    assert!(err.contains("printed 1 lines, need 2"), "{err}");

    let one: [Arm; 1] = [("native run 1", vec![good.clone()])];
    assert!(
        first_divergence(&one).is_err(),
        "one arm has nothing to disagree with"
    );
}

/// **The negative control has teeth, natively.** Dropping batch 2 must move the native arm
/// off the honest line *and* be reported at exactly `stream-small batch 2` — the batch the
/// control names, and no earlier one, because batches `0..2` are still processed alike.
#[test]
fn the_dropped_batch_is_reported_as_a_divergence_at_that_batch() {
    let honest_text = arm_lines(&honest(), Route::Json).expect("the honest run");
    let dropped = setting_for(crate::hashgate::Knob::DropDelta.env(), "2").expect("parses");
    let dropped_text = arm_lines(&dropped, Route::Json).expect("the controlled run");
    assert_ne!(dropped_text, honest_text, "the control perturbed nothing");
    let honest_arm = arm("native run 1", honest_text);
    let dropped_arm = arm("native run 1 (dropped batch 2)", dropped_text);
    assert_eq!(
        batch_count(std::slice::from_ref(&dropped_arm)),
        batch_count(std::slice::from_ref(&honest_arm)),
        "a dropped batch must still print its line, or the arms are incomparable"
    );
    let arms: [Arm; 2] = [honest_arm, dropped_arm];
    let reported = first_divergence(&arms)
        .expect("comparable")
        .expect("diverged");
    assert!(
        reported.starts_with("stream-small batch 2 ("),
        "the first divergence must be the dropped batch: {reported}"
    );
}

/// Batch 0 is the initial graph, so a control that named it would drop nothing — refused at
/// the parse rather than run and reported as equality.
#[test]
fn a_zero_batch_index_is_refused_at_the_parse() {
    for value in ["0", "maybe", "-1", ""] {
        let err = setting_for(crate::hashgate::Knob::DropDelta.env(), value)
            .expect_err("batch 0 is not a delta and a typo is not a batch");
        assert!(err.contains("GM_MUTATE_DROP_DELTA"), "{value:?}: {err}");
    }
}

/// The Node arm is a separate script in another language, and the two agree on the stage id
/// and on which exports the stage needs. The tick count is passed in, never written into the
/// script, so there is one number for it in the world.
#[test]
fn the_wasm_stream_script_agrees_with_this_crate() {
    let script = std::fs::read_to_string(stream_script()).expect("the script is committed");
    assert!(
        script.contains(&format!("const STAGE = \"{STREAM_STAGE}\";")),
        "the stage id must be written identically in both languages"
    );
    assert!(
        !script.contains(&format!("{TICKS}")),
        "the tick count must be passed in, never written into the script"
    );
    for name in [
        "gm_graph_extend",
        "gm_force_session_grow",
        "gm_force_session_create",
        "gm_force_session_tick",
        "gm_release",
    ] {
        assert!(
            script.contains(name),
            "{name} is missing from the stream arm"
        );
    }
}
