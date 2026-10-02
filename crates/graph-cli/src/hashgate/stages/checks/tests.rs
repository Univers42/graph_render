//! The refusals [`super`] makes before any hashing: a node count that cannot be indexed and
//! a stage list that cannot be printed. Split out by the house's 300-line limit; every test
//! here is a *negative control* in the sense the gate uses the word — a refusal that is
//! missing fails loudly, a refusal that has been weakened fails here.

use crate::hashgate::stage_bytes;
use crate::hashgate::stage_bytes_threaded;
use crate::hashgate::tests::honest;
use crate::hashgate::{Knob, Setting};
use graph_core::layout::tidy_tree;

/// `GM_MUTATE_NODE_COUNT` at its maximum, which is what a `u32` sum wraps on.
///
/// The value is `u32::MAX` rather than something near it because the *release* build wraps
/// silently and the *debug* build panics: the finding this pins was a release-only
/// corruption, and `6 + 4294967295` only wraps on the second addition.
const WRAPPING: u32 = u32::MAX;

/// `gate_node_count(4)` — the seed's own share of the sum, which the refusal spells out.
const GATE: u32 = 6;

/// A node count past the `u32` index space is refused, naming the variable and every term.
///
/// **This is the gate's worst failure mode if it is missing.** Wrapping does not crash and
/// does not diverge: it silently *shrinks* the model, every stage is hashed from a two-node
/// graph instead of a four-billion-node one, both arms agree, and the gate exits 0 having
/// measured nothing. So the assertion is on the refusal text as well as the `Err` — a
/// refusal that did not name the variable would still leave the reader guessing which knob
/// to change, and one that did not spell out the sum would leave them unable to tell
/// whether the seed's own count was part of it.
#[test]
fn a_node_count_past_the_u32_index_space_is_refused_by_name() {
    let absurd = Setting {
        extra_nodes: WRAPPING,
        control: Some(Knob::NodeCount),
        ..honest()
    };
    for (what, err) in refusals(&absurd) {
        assert!(
            err.contains("GM_MUTATE_NODE_COUNT"),
            "{what}: the refusal must name the variable: {err}"
        );
        assert!(
            err.contains(&format!("{GATE} for seed 4 plus {WRAPPING}")),
            "{what}: the refusal must spell out the sum: {err}"
        );
    }
}

/// The same rule for a per-stage control's own extra nodes, which is added to the shared
/// one rather than replacing it — a second addition, and so a second place the sum can wrap.
///
/// `control` is set here as the parser would set it: the refusal names the variable the run
/// is *under*, and a hand-built `Setting` that left it `None` would be asking the refusal to
/// name a knob nothing had claimed.
#[test]
fn a_per_stage_control_that_would_wrap_the_sum_is_refused_by_name() {
    let absurd = Setting {
        stage_nodes: Some((tidy_tree::ID, WRAPPING)),
        control: Some(Knob::TreeTidyNodes),
        ..honest()
    };
    for (what, err) in refusals(&absurd) {
        assert!(
            err.contains("GM_MUTATE_TREE_TIDY_NODES"),
            "{what}: the refusal must name the per-stage variable: {err}"
        );
        assert!(
            err.contains(&format!("{GATE} for seed 4 plus {WRAPPING}")),
            "{what}: the seed's own count is part of the sum: {err}"
        );
    }
}

/// Both arms, because both draw this count: the native arm through [`stage_bytes`] and the
/// threaded arm through [`stage_bytes_threaded`], which is the one reader in another file.
fn refusals(setting: &Setting) -> Vec<(&'static str, String)> {
    vec![
        ("native", stage_bytes(4, setting).expect_err("refused")),
        (
            "threaded",
            stage_bytes_threaded(4, setting, 3).expect_err("refused"),
        ),
    ]
}
