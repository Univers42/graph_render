//! The six igraph-family layouts end to end: each stage 4-way hashed, and each with its
//! own negative control going red on only its stage.
//!
//! Split out of `cli.rs` to stay under the house's 300-line limit, and because these six
//! are one thing `cli_p3.rs`'s four are not: none of them takes a parameter the gate can
//! move (each module pins its own defaults), so all six controls are the node-re-draw
//! probe, and the interesting claim is that six controls which look alike each still move
//! exactly one stage. A shared control would move all six at once and name none.
//!
//! Needs `node`, the `wasm32-unknown-unknown` target and `harness/node_modules`, as
//! `cli.rs` does.

mod common;

use common::stdout;
use std::process::Output;

fn gates_dir() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("gm-cli-igraph-{}", std::process::id()))
}

fn graph_cli(args: &[&str], mutate: Option<(&str, &str)>) -> Output {
    common::graph_cli(&gates_dir(), args, mutate)
}

fn record(name: &str) -> String {
    std::fs::read_to_string(gates_dir().join(format!("{name}.json"))).expect("recorded")
}

/// The six stages, in `hashgate`'s own order. Spelled out by id rather than derived from
/// the binary: a list read off the binary would agree with the binary by construction, and
/// the claim being made is that these six are hashed under exactly these names.
const IGRAPH: [&str; 6] = [
    "layout.force.fruchterman_reingold",
    "layout.force.kamada_kawai",
    "layout.force.graphopt",
    "layout.force.davidson_harel",
    "layout.force.lgl",
    "layout.force.drl",
];

/// The stages that are none of the six, and that a per-layout control must therefore
/// leave byte-identical. `transport.wasm.columnar` is here on purpose: it restates the
/// grid's bytes, so it is the check that a stage-scoped control never reached the gate's
/// shared model. Named by id, and only by id: an earlier version of this file counted
/// them, and the count moved the moment a layout was registered.
const OTHERS: [&str; 32] = [
    "topology",
    "layout.grid",
    "layout.tree.tidy",
    "layout.treemap.squarified",
    "layout.circular.radial",
    "layout.packing.circle",
    "layout.spectral",
    "layout.mds.pivot",
    "layout.force.barnes_hut",
    "layout.forceatlas2",
    "layout.dag.sugiyama",
    "layout.random",
    "layout.circular.ring",
    "layout.spiral",
    "layout.bipartite",
    "layout.force.yifan_hu",
    "analysis.components.weak",
    "analysis.components.strong",
    "analysis.communities.louvain",
    "analysis.centrality.degree",
    "analysis.centrality.closeness",
    "analysis.centrality.betweenness",
    "analysis.centrality.eigenvector",
    "analysis.depth.bfs",
    "post.bundle.fdeb",
    "post.bundle.mingle",
    "post.route.grid",
    "post.style.straight",
    "post.style.orthogonal",
    "post.style.quadratic",
    "post.style.bezier",
    "transport.wasm.columnar",
];

/// A control, the value that must move its stage, the stage, and the record it writes.
const CONTROLS: [(&str, &str, &str, &str); 6] = [
    (
        "GM_MUTATE_FORCE_FRUCHTERMAN_REINGOLD_NODES",
        "1",
        "layout.force.fruchterman_reingold",
        "hashgate-control-force-fruchterman-reingold-nodes",
    ),
    (
        "GM_MUTATE_FORCE_KAMADA_KAWAI_NODES",
        "1",
        "layout.force.kamada_kawai",
        "hashgate-control-force-kamada-kawai-nodes",
    ),
    (
        "GM_MUTATE_FORCE_GRAPHOPT_NODES",
        "1",
        "layout.force.graphopt",
        "hashgate-control-force-graphopt-nodes",
    ),
    (
        "GM_MUTATE_FORCE_DAVIDSON_HAREL_NODES",
        "1",
        "layout.force.davidson_harel",
        "hashgate-control-force-davidson-harel-nodes",
    ),
    (
        "GM_MUTATE_FORCE_LGL_NODES",
        "1",
        "layout.force.lgl",
        "hashgate-control-force-lgl-nodes",
    ),
    (
        "GM_MUTATE_FORCE_DRL_NODES",
        "1",
        "layout.force.drl",
        "hashgate-control-force-drl-nodes",
    ),
];

/// The honest run first: a control that "goes red" against a gate that was already red
/// proves nothing.
#[test]
fn the_six_igraph_stages_are_4_way_compiled_and_hashed() {
    let honest = graph_cli(&["hashgate", "--seeds", "4"], None);
    assert_eq!(honest.status.code(), Some(0), "{}", stdout(&honest));
    let out = stdout(&honest);
    for stage in IGRAPH {
        assert!(
            out.contains(&format!("  {stage}: 4-way equal on 4/4 seeds")),
            "{stage} did not go 4-way equal: {out}"
        );
    }
    let record = record("hashgate");
    for stage in IGRAPH {
        assert!(
            record.contains(&format!("\"{stage}\": 4")),
            "{stage} is missing from the record: {record}"
        );
    }
}

/// Each layout's own control, end to end: the wasm arm runs the compiled-in defaults and
/// cannot see the variable, so a wired knob surfaces as exactly the cross-target
/// divergence — on the stage it is filed under and nowhere else.
///
/// **The seed count is not incidental.** `--seeds 4` gives models of 2 to 6 nodes, which
/// is enough for a force layout to have two bodies to push apart; at 2 nodes the picture
/// is a point and any perturbation is a no-op, the vacuous-control failure mode.
#[test]
fn each_igraph_layouts_own_control_goes_red_on_only_its_stage() {
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
        for other in OTHERS.into_iter().chain(IGRAPH).filter(|s| *s != stage) {
            assert!(
                out.contains(&format!("  {other}: 4-way equal on {SEEDS}/{SEEDS} seeds")),
                "{knob} must not move {other}: {out}"
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

/// A typo, a zero and a doubled control are refused, as for every other per-stage control:
/// a control that falls back to the default, or perturbs by nothing, would pass as green.
#[test]
fn the_igraph_controls_are_refused_on_a_typo_a_zero_or_a_doubled_knob() {
    for (knob, value) in [
        (CONTROLS[0].0, "one"),
        (CONTROLS[1].0, "-1"),
        (CONTROLS[2].0, "0"),
        (CONTROLS[3].0, ""),
    ] {
        let run = graph_cli(&["hashgate", "--seeds", "2"], Some((knob, value)));
        assert_eq!(
            run.status.code(),
            Some(2),
            "{knob}={value} must not pass as a control"
        );
    }
    // Two of the six together, and one of the six with an unrelated per-stage control: the
    // rule is one at a time, and a shared `stage_nodes` slot would silently let the last
    // one win if it were not enforced.
    for (a, b) in [
        (CONTROLS[0].0, CONTROLS[1].0),
        (CONTROLS[0].0, "GM_MUTATE_CIRCULAR_NODES"),
    ] {
        let both = common::command(&gates_dir())
            .args(["hashgate", "--seeds", "2"])
            .env(a, "1")
            .env(b, "1")
            .output()
            .expect("runs");
        assert_eq!(both.status.code(), Some(2), "{a} and {b}: one at a time");
    }
}
