//! The `layout.treemap.patchwork` differential: our squarified treemap against **Graphviz's
//! own** `patchwork`, in the docker-only `ge-graphviz-oracle` image.
//!
//! **One command, one engine name.** `graph-cli oracle-graphviz --engine <name>` is the whole
//! differential; the engine is a parameter, not a command, so the next Graphviz engine is a
//! row in the ceilings table and not another subcommand. `oracle-twopi` is kept working as
//! an alias for `--engine twopi`: the twopi row of `scripts/orch/rows/p13-gv1.rows` runs
//! unchanged and still grades against `harness/oracle-twopi.py`'s own result file.
//!
//! **Two arms, two files, two owners.** `graph-cli emit-graphviz-fixtures --engine patchwork`
//! writes *our* coordinates, one line per seed, with a manifest carrying the tree
//! fingerprint. `harness/oracle-graphviz.py <fixtures> patchwork <out>` writes *Graphviz's*,
//! in the same shape, with its own manifest carrying the sha256 of what it wrote. This
//! command is the only thing that sees both, and it is where the comparison happens — so the
//! oracle side stays exactly the deterministic, byte-comparable artefact the ADR records
//! (`cmp` over two runs of it), untouched by any Rust.
//!
//! **The metric** is the largest absolute node-coordinate difference in points, after both
//! arms are rescaled onto the same bounding box: Graphviz's own node-centre bounding box is
//! the target and both arms are mapped onto it with one uniform scale. The rescale removes
//! the translation and the scale and nothing else — the tiling's shape survives, which is the
//! only thing this layout has.
//!
//! **The coordinates are another implementation's, in points**, and the ceiling is
//! [`CEILING`], the next power of ten above the worst gap measured over the 1000 gate seeds
//! (`docs/measurements/p13-gv1-patchwork.md`). `-Gstart` is inert for this engine — measured
//! over all 1000 seeds at start 1, 7 and 99, byte-identical — so a gap is an algorithmic
//! difference or nothing, never seed drift.
//!
//! Ponytail: the oracle's `-Tplain` output carries five significant digits, and Graphviz's own
//! drawing extent differs from the closed form by up to 3.62e-3 pt (measured over n = 1..100),
//! so byte-exact agreement against the oracle's *text* is not reachable and is not claimed.
//! The ceiling reflects the printed resolution, not a disagreement. The five analytically
//! determined small cases are instead pinned exactly against the closed form in
//! `crates/graph-core/src/layout/graphviz/patchwork/tests.rs`, and Graphviz's own printed
//! lines for the same five graphs are recorded in the measurements file.

use super::graphviz_arm::{Arm, gap, read_fixture, read_json, read_oracle};
use super::{Differential, coords};
use crate::evidence::Stamp;
use crate::runner::file_sha256;
use graph_core::layout::graphviz::patchwork;
use graph_core::{REFERENCE_DEGREE, gate_node_count, index_model, seeded_model};
use serde_json::{Value, json};
use std::path::Path;
use std::process::ExitCode;

/// The Graphviz engine's own name, which is also this differential's fixture and ledger
/// name — one string, so a `--engine` typo is a name that resolves to no layout rather than a
/// silently different comparison.
pub const ENGINE: &str = "patchwork";

/// `patchwork`'s half of the three-step shape the other differentials use: the fixtures to
/// emit, and the layout id the ceiling grades. The comparison itself is `graphviz_arm`, which
/// is engine-free, so the next engine is a row here and not a second copy of the metric.
const PATCHWORK: Differential = Differential {
    name: ENGINE,
    ceilings: &[("layout.treemap.patchwork", "patchwork", CEILING)],
    line,
};

/// The patchwork ceiling: the worst `max |ours - theirs|` in points over the 1000 gate seeds
/// is **6.61e-2**, rounded up to the next power of ten.
///
/// That figure is the oracle's own printed resolution and not a disagreement. `-Tplain`
/// writes five significant digits, so at the largest gate drawing — 10.34 inches across, 744
/// points — one printed digit is 0.001 inch, which is 0.072 points. The per-seed gaps track the
/// drawing's extent to within a factor of 1.07, the median is 5.1e-3, and 4 of the 1000 seeds
/// are exact. A tighter ceiling would be a claim about `-Tplain`'s formatter, not about the
/// layout. See `docs/measurements/p13-gv1-patchwork.md`.
pub(crate) const CEILING: f64 = 1e-1;

/// One seed's line: the gate's model, its bare graph structure for the harness to write DOT
/// from, and our own coordinates. The layout is closed form, so the emit's `--max-iter`
/// (ForceAtlas2's) does not reach it.
fn line(seed: u32, _max_iter: Option<u32>) -> Result<Value, String> {
    let n = gate_node_count(seed);
    let (nodes, edges) = seeded_model(seed, n, REFERENCE_DEGREE);
    let topology = index_model(&nodes, &edges).map_err(|e| e.to_string())?;
    let columns = topology.edges();
    let mut out =
        json!({ "seed": seed, "n": n, "source": columns.source, "target": columns.target });
    out[ENGINE] = coords(patchwork::ID, &nodes, &edges)?;
    Ok(out)
}

/// `graph-cli oracle-graphviz --engine patchwork`: reads both arms, grades the worst gap
/// against the ceiling, and records the verdict for the ledger.
pub fn check(engine: &str, fixtures: &Path, graphviz: &Path) -> ExitCode {
    match verdict(engine, fixtures, graphviz) {
        Ok(pass) => ExitCode::from(u8::from(!pass)),
        Err(err) => {
            eprintln!("oracle-graphviz --engine {engine}: could not run: {err}");
            ExitCode::from(2)
        }
    }
}

/// The fixture set this engine's `Differential` is, or the error naming what was asked for.
fn differential(engine: &str) -> Result<&'static Differential, String> {
    if engine == PATCHWORK.name {
        return Ok(&PATCHWORK);
    }
    Err(format!(
        "{engine}: no Graphviz differential; this build has {}",
        PATCHWORK.name
    ))
}

fn verdict(engine: &str, fixtures: &Path, graphviz: &Path) -> Result<bool, String> {
    let differential = differential(engine)?;
    let name = differential.name;
    let stamp = Stamp::take()?;
    let manifest = read_json(&fixtures.join(format!("{name}-manifest.json")))?;
    // The oracle's own manifest is what makes the comparison honest about *which* run of the
    // engine it is: the sha256 in it is over the file the harness wrote, so a re-run that
    // changed nothing is the same bytes and a re-run that changed something is refused rather
    // than silently compared.
    let oracle_manifest = read_json(&graphviz.join(format!("graphviz-{name}-manifest.json")))?;
    let oracle_file = format!("graphviz-{name}.jsonl");
    if oracle_manifest["engine"] != engine {
        return Err(format!(
            "the oracle's manifest names engine {}",
            oracle_manifest["engine"]
        ));
    }
    let oracle_path = graphviz.join(&oracle_file);
    if oracle_manifest["sha256"][&oracle_file] != json!(file_sha256(&oracle_path)?) {
        return Err("the oracle's output is not the run its manifest records".into());
    }
    let (cases, worst, exact) = compare(differential, fixtures, graphviz)?;
    let within = cases > 0 && worst <= CEILING;
    println!(
        "  layout.treemap.patchwork: {cases} cases, worst {worst:.3e}, ceiling {CEILING:.0e}: {}",
        if within { "ok" } else { "FAIL" }
    );
    println!("  {exact} of {cases} seeds exact, and the manifest's fingerprint is checked");
    if manifest["fingerprint"].as_str() != Some(stamp.fingerprint()) {
        return Err("the fixtures and the tree are not the same tree: re-emit".into());
    }
    let body = json!({
        "seeds": cases, "pass": within, "engine": engine,
        "functions": { "layout.treemap.patchwork": {
            "cases": cases, "declared": 0, "unexplained": u64::from(!within),
            "worst": worst, "ceiling": CEILING,
        }},
        "oracle": "Graphviz 16.1.0 patchwork -Tplain -Gstart=1",
        "tolerance": true,
    });
    stamp.still_current()?;
    crate::evidence::record(&stamp, &format!("oracle-{name}"), body)?;
    println!("{}", if within { "PASS" } else { "FAIL" });
    Ok(within)
}

/// One seed's gap: our arm's coordinates against Graphviz's, both rescaled onto Graphviz's
/// own bounding box by [`gap`].
fn seed_gap(ours: &mut Arm, theirs: &mut Arm, name: &str) -> Result<Option<f64>, String> {
    let (mine, other) = match (ours.next_row()?, theirs.next_row()?) {
        (Some(mine), Some(other)) => (mine, other),
        (None, None) => return Ok(None),
        (mine, other) => {
            let count = usize::from(mine.is_some()) + usize::from(other.is_some());
            return Err(format!(
                "{count} arm(s) ran past the other: re-run the harness"
            ));
        }
    };
    let theirs_points = read_oracle(&other, "nodes")?;
    gap(&read_fixture(&mine, name)?, &theirs_points).map(Some)
}

/// Both arms over every seed: the case count, the worst gap in points, and how many seeds
/// agreed exactly. One record of each file at a time, in lockstep, so a length mismatch is
/// refused rather than silently comparing the seeds both files happen to have.
fn compare(
    differential: &Differential,
    fixtures: &Path,
    graphviz: &Path,
) -> Result<(usize, f64, usize), String> {
    let name = differential.name;
    let mut ours = Arm::open(&fixtures.join(format!("{name}.jsonl")))?;
    let mut theirs = Arm::open(&graphviz.join(format!("graphviz-{name}.jsonl")))?;
    let (mut cases, mut worst, mut exact) = (0_usize, 0.0_f64, 0_usize);
    while let Some(gap) = seed_gap(&mut ours, &mut theirs, name)? {
        cases += 1;
        exact += usize::from(gap == 0.0);
        worst = worst.max(gap);
    }
    Ok((cases, worst, exact))
}

/// `graph-cli emit-graphviz-fixtures --engine patchwork`, the emit half. It is the generic
/// [`Differential`] emit with the engine resolved first, so a typo is refused before any file
/// is written.
pub fn emit(engine: &str, seeds: u32, out: &Path) -> ExitCode {
    let differential = match differential(engine) {
        Ok(d) => d,
        Err(err) => {
            eprintln!("emit-graphviz-fixtures: {err}");
            return ExitCode::from(2);
        }
    };
    super::emit(differential, seeds, None, out)
}

#[cfg(test)]
mod tests;
