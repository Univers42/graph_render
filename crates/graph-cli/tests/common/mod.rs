//! The process helpers every `graph-cli` integration test shares. Each file under
//! `tests/` is its own crate and names its own gates directory; what must not drift
//! between them is the knob list, since a copy missing a knob lets that control leak in
//! from the environment and turn an honest run red (or a control run green).

use std::path::Path;
use std::process::{Command, Output};

/// Every negative-control variable `graph-cli hashgate` reads.
pub const KNOBS: [&str; 6] = [
    "GM_MUTATE_REFERENCE_DEGREE",
    "GM_MUTATE_GRID_SPACING",
    "GM_MUTATE_SUGIYAMA_LAYER_SPACING",
    "GM_MUTATE_NODE_COUNT",
    "GM_MUTATE_FORCE_THETA",
    "GM_MUTATE_FA2_SCALING_RATIO",
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
