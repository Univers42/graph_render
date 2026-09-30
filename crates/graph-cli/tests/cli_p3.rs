//! The four Phase 3 one-shot layouts end to end: each stage 4-way hashed, each with its
//! own negative control going red on only its stage. Split out of `cli.rs` to stay under
//! the house's 300-line limit.
//!
//! Needs `node`, the `wasm32-unknown-unknown` target and `harness/node_modules`, as
//! `cli.rs` does.
//!
//! **Why these four needed a knob at all.** Tidy tree, treemap and circular take no
//! parameters by design — their modules pin every convention and say so — and circle
//! packing's `scale` is the one published parameter among the four. Before this, their
//! stages were backed only by `GM_MUTATE_NODE_COUNT`, which grows the gate's one shared
//! model and therefore moves every stage that is a function of the topology: eleven
//! stages at once, so a red run says nothing about *which* stage a divergence came from.
//! A stage-specific control says exactly that, and the assertion below is the point:
//! every other stage stays `4-way equal on N/N seeds` while one does not.

mod common;

use common::{KNOBS, stdout};
use std::process::Output;

fn gates_dir() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("gm-cli-p3-gates-{}", std::process::id()))
}

fn graph_cli(args: &[&str], mutate: Option<(&str, &str)>) -> Output {
    common::graph_cli(&gates_dir(), args, mutate)
}

fn record(name: &str) -> String {
    std::fs::read_to_string(gates_dir().join(format!("{name}.json"))).expect("recorded")
}

/// The eight stages that are none of Phase 3's four, and that a per-layout control must
/// therefore leave byte-identical. `transport.wasm.columnar` is here on purpose: it
/// restates the grid's bytes, so it is the check that a stage-scoped control never
/// reached the gate's shared model.
const OTHERS: [&str; 8] = [
    "topology",
    "layout.grid",
    "layout.spectral",
    "layout.mds.pivot",
    "layout.force.barnes_hut",
    "layout.forceatlas2",
    "layout.dag.sugiyama",
    "transport.wasm.columnar",
];

/// The four Phase 3 stages, in `hashgate`'s own order.
const P3: [&str; 4] = [
    "layout.tree.tidy",
    "layout.treemap.squarified",
    "layout.circular.radial",
    "layout.packing.circle",
];

/// A knob, a value that must move its stage, and the record it writes.
const CONTROLS: [(&str, &str, &str, &str); 4] = [
    (
        "GM_MUTATE_TREE_TIDY_NODES",
        "1",
        "layout.tree.tidy",
        "hashgate-control-tree-tidy-nodes",
    ),
    (
        "GM_MUTATE_TREEMAP_NODES",
        "1",
        "layout.treemap.squarified",
        "hashgate-control-treemap-nodes",
    ),
    (
        "GM_MUTATE_CIRCULAR_NODES",
        "1",
        "layout.circular.radial",
        "hashgate-control-circular-nodes",
    ),
    (
        "GM_MUTATE_PACKING_SCALE",
        "9",
        "layout.packing.circle",
        "hashgate-control-packing-scale",
    ),
];

/// The honest run first: a control that "goes red" against a gate that was already red
/// proves nothing, and neither does one against a stage the gate does not hash.
#[test]
fn the_four_p3_stages_are_4_way_compiled_and_hashed() {
    let honest = graph_cli(&["hashgate", "--seeds", "4"], None);
    assert_eq!(honest.status.code(), Some(0), "{}", stdout(&honest));
    let out = stdout(&honest);
    for stage in P3 {
        assert!(
            out.contains(&format!("  {stage}: 4-way equal on 4/4 seeds")),
            "{stage} did not go 4-way equal: {out}"
        );
    }
    let record = record("hashgate");
    for stage in P3 {
        assert!(
            record.contains(&format!("\"{stage}\": 4")),
            "{stage} is missing from the record: {record}"
        );
    }
}

/// Each Phase 3 layout's own negative control, end to end: the wasm arm runs the
/// compiled-in defaults and cannot see the variable, so a wired knob surfaces as exactly
/// the cross-target divergence — and on the stage it is filed under and nowhere else.
///
/// **The seed count is not incidental.** `gate_node_count(seed)` is `2 + seed % 600`, so
/// `--seeds 4` gives models of 2 to 6 nodes. That is enough for squarify to have more
/// than one box and for a packing to have more than one circle — at 2 nodes both are a
/// single element and *any* perturbation is a no-op, which is the vacuous-control failure
/// mode `cli_force.rs` documents at the other end of the range.
#[test]
fn each_p3_layouts_own_control_goes_red_on_only_its_stage() {
    const SEEDS: &str = "4";
    for (knob, value, stage, record_name) in CONTROLS {
        let run = graph_cli(&["hashgate", "--seeds", SEEDS], Some((knob, value)));
        assert_eq!(
            run.status.code(),
            Some(1),
            "{knob}={value}: {}",
            stdout(&run)
        );
        let out = stdout(&run);
        let line = out
            .lines()
            .find(|l| l.trim_start().starts_with(&format!("{stage}:")))
            .unwrap_or_else(|| panic!("{stage} is not reported: {out}"));
        let equal: usize = line
            .rsplit_once("on ")
            .and_then(|(_, rest)| rest.split('/').next())
            .and_then(|n| n.trim().parse().ok())
            .unwrap_or_else(|| panic!("no seed count in {line:?}"));
        assert!(equal < 4, "{knob} did not go red on {stage}: {line}");
        // Every other stage, and only the one named above, moved.
        for other in OTHERS {
            assert!(
                out.contains(&format!("  {other}: 4-way equal on {SEEDS}/{SEEDS} seeds")),
                "{knob} must not move {other}: {out}"
            );
        }
        for p3 in P3.into_iter().filter(|p| *p != stage) {
            assert!(
                out.contains(&format!("  {p3}: 4-way equal on {SEEDS}/{SEEDS} seeds")),
                "{knob} must not move the other Phase 3 stage {p3}: {out}"
            );
        }
        let control = record(record_name);
        assert!(control.contains("\"pass\": false"), "{control}");
        assert!(
            control.contains(&format!("\"mutation\": \"{knob}\"")),
            "{control}"
        );
        assert!(
            control.contains(&format!("\"{stage}\": 0")),
            "the control's own record must carry the stage's count: {control}"
        );
    }
}

/// One control at a time, and no falling back to the default: a run that quietly ignored
/// a mistyped variable would exit 0 and be read as a passing control.
#[test]
fn the_p3_controls_are_refused_on_a_typo_a_zero_or_a_doubled_knob() {
    for (knob, value) in [
        (CONTROLS[0].0, "one"),
        (CONTROLS[1].0, "-1"),
        (CONTROLS[2].0, "0"),
        (CONTROLS[3].0, "wide"),
    ] {
        let run = graph_cli(&["hashgate", "--seeds", "2"], Some((knob, value)));
        assert_eq!(
            run.status.code(),
            Some(2),
            "{knob}={value} must not pass as a control"
        );
    }
    let both = common::command(&gates_dir())
        .args(["hashgate", "--seeds", "2"])
        .env(CONTROLS[0].0, "1")
        .env(CONTROLS[2].0, "1")
        .output()
        .expect("runs");
    assert_eq!(both.status.code(), Some(2), "one control at a time");
}

/// The knob list carries every Phase 3 control. `common::KNOBS` is the list every test
/// binary clears, so a control missing from it leaks in from the environment and turns an
/// honest run red — or a control run green. The list has since grown past Phase 3 to the
/// eight ANALYSIS and seven POST stage controls and the compute-tier control, so its length
/// is pinned to the whole set rather than to these four.
#[test]
fn the_knob_list_carries_every_p3_control_and_nothing_else() {
    for (knob, _, _, _) in CONTROLS {
        assert!(KNOBS.contains(&knob), "{knob} is missing from KNOBS");
    }
    assert_eq!(
        KNOBS.len(),
        27,
        "ten parameter controls, then the fifteen stage controls, then the two compute-tier \
         controls: {KNOBS:?}"
    );
}
