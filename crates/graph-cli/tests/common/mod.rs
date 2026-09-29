//! The process helpers every `graph-cli` integration test shares. Each file under
//! `tests/` is its own crate and names its own gates directory; what must not drift
//! between them is the knob list, since a copy missing a knob lets that control leak in
//! from the environment and turn an honest run red (or a control run green).

use std::path::Path;
use std::process::{Command, Output};

/// Every negative-control variable `graph-cli hashgate` reads.
///
/// Extended additively as stages gained a control of their own: the four Phase 3 layouts
/// (tidy tree, treemap, circular, circle packing) each have one, so `hashgate` turns red
/// on a stage the way it already did for Barnes-Hut, ForceAtlas2, the grid, the layered
/// drawing and the topology. The order is the order of `hashgate::Knob::ALL`, which the
/// unit test `each_knob_names_its_own_variable_and_record` pins against this list's twin
/// in `crates/graph-cli/src/hashgate/tests/knob.rs`.
pub const KNOBS: [&str; 10] = [
    "GM_MUTATE_REFERENCE_DEGREE",
    "GM_MUTATE_GRID_SPACING",
    "GM_MUTATE_SUGIYAMA_LAYER_SPACING",
    "GM_MUTATE_NODE_COUNT",
    "GM_MUTATE_FORCE_THETA",
    "GM_MUTATE_FA2_SCALING_RATIO",
    "GM_MUTATE_TREE_TIDY_NODES",
    "GM_MUTATE_TREEMAP_NODES",
    "GM_MUTATE_CIRCULAR_NODES",
    "GM_MUTATE_PACKING_SCALE",
];

/// `graph-cli` recording under `gates`, never `target/gates` (a test run must not stand
/// in for a real gate's evidence), with every knob cleared.
pub fn command(gates: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_graph-cli"));
    command.env("GM_GATES_DIR", gates);
    for knob in KNOBS {
        command.env_remove(knob);
    }
    command
}

/// `graph-cli args` under `gates` with every knob unset but `mutate`, if given.
pub fn graph_cli(gates: &Path, args: &[&str], mutate: Option<(&str, &str)>) -> Output {
    let mut command = command(gates);
    command.args(args);
    if let Some((knob, value)) = mutate {
        command.env(knob, value);
    }
    command.output().expect("graph-cli runs")
}

pub fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}
