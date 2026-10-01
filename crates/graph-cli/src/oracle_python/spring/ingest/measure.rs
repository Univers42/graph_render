//! The measuring half of `graph-cli oracle-spring`: both arms, one case at a time, and the
//! metric applied to each.
//!
//! **Two exact assertions live here,** and they are the part of this gate that no
//! statistic replaces: the reference's own rescale contract (`layout.py:646`) and its
//! one-node answer (`layout.py:618-624`), both checked on *both* arms, whatever start each
//! was given. A port that skipped the rescale would miss the extent by orders of magnitude,
//! not by a last bit, so the one-part-in-a-million slack costs the check nothing; ours
//! arrives already narrowed to `f32` by the snapshot.
//!
//! Ponytail: the quantiles are nearest-rank, never interpolated. A measurement this gate
//! reports is a measurement, and smoothing it into a value no sample took would make the
//! reported median and the gated median different numbers wearing the same name.

use super::Summary;
use crate::stress::metric::{Graph, correlate};
use serde_json::Value;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

/// The reference arm's output file: one `{"seed", "n", "spring": {x, y}}` line per case, in
/// the fixtures' own order.
const THEIRS: &str = "spring-theirs.jsonl";

/// One part in a million on the extent: ours arrives `f32`-narrowed by the snapshot, and
/// `4.9999999559` *is* `5.0` at `f32`'s precision.
const EXTENT_SLACK: f64 = 1e-6;

/// Both arms, one case at a time, in the fixtures' own order. `None` from [`deficit`] is a
/// case no correlation exists for — too small for a pair, or a component of one node —
/// and is counted, never fabricated into a number.
pub(super) fn run(dir: &Path, manifest: &Value) -> Result<Summary, String> {
    let mut mine = reader(dir, "spring.jsonl")?;
    let mut yours = reader(dir, THEIRS)?;
    let mut sum = Summary::default();
    let mut deficits: Vec<f64> = Vec::new();
    loop {
        let (a, b) = match (mine.next_line()?, yours.next_line()?) {
            (Some(a), Some(b)) => (a, b),
            (None, None) => break,
            (a, b) => {
                return Err(format!(
                    "the two arms hold different case counts ({}, {}): re-run the harness",
                    a.is_some(),
                    b.is_some()
                ));
            }
        };
        let ours: Value = parse(&a)?;
        let arm: Value = parse(&b)?;
        check_manifest(manifest, &ours)?;
        sum.cases += 1;
        match deficit(&ours, &arm)? {
            Some(d) => {
                deficits.push(d);
                sum.scored += 1;
                let below = nodes(&ours)? < 500;
                if d > sum.worst {
                    sum.worst = d;
                    sum.worst_seed = seed(&ours)?;
                }
                if below && d > sum.worst_below_500 {
                    sum.worst_below_500 = d;
                }
            }
            None => sum.degenerate += 1,
        }
    }
    deficits.sort_by(f64::total_cmp);
    sum.median = percentile(&deficits, 0.5);
    sum.p90 = percentile(&deficits, 0.9);
    Ok(sum)
}

/// One case's deficit, or `None` when a correlation does not exist on either arm. The two
/// exact invariants are checked first and refuse the run rather than being scored: they are
/// the answers a statistic cannot stand in for.
pub(super) fn deficit(ours: &Value, theirs: &Value) -> Result<Option<f64>, String> {
    let n = nodes(ours)?;
    if n != nodes(theirs)? || seed(ours)? != seed(theirs)? {
        return Err("the two arms disagree about which case this is: re-run the harness".into());
    }
    let scale = ours["params"]["scale"].as_f64().unwrap_or(0.0);
    let mine = pairs(ours)?;
    let arm = pairs(theirs)?;
    if mine.len() != n || arm.len() != n {
        return Err("an arm answered with the wrong node count".into());
    }
    rescale_contract(&mine, scale, "ours")?;
    rescale_contract(&arm, scale, "networkx")?;
    let graph = graph(ours, n)?;
    let (Some(ours_r), Some(theirs_r)) = (correlate(&graph, &mine), correlate(&graph, &arm)) else {
        return Ok(None);
    };
    Ok(Some((theirs_r - ours_r).max(0.0)))
}

/// The graph both arms laid out, checked against the fixture's own node count: the metric
/// compares positions against *this* graph's hop distances, so a graph that does not cover
/// every node would score the drawing against the wrong distances.
pub(super) fn graph(case: &Value, n: usize) -> Result<Graph, String> {
    let read = |key: &str| -> Result<Vec<u32>, String> {
        case[key]
            .as_array()
            .ok_or_else(|| format!("no {key} column"))?
            .iter()
            .map(|v| {
                v.as_u64()
                    .map(|n| n as u32)
                    .ok_or_else(|| format!("{key} is not a node"))
            })
            .collect()
    };
    let (source, target) = (read("source")?, read("target")?);
    if source.len() != target.len() {
        return Err("the edge columns hold different counts".into());
    }
    let edges: Vec<[u32; 2]> = source.iter().zip(&target).map(|(&s, &t)| [s, t]).collect();
    if edges.is_empty() {
        // `Graph::from_edges` takes its node count from the edges it is given, so an
        // edgeless graph — a lone node, or `n` nodes none joined — has to be named
        // outright or it would be scored as a graph of no nodes.
        return Ok(Graph::with_nodes(n));
    }
    let graph = Graph::from_edges(&edges);
    if graph.len() != n {
        return Err(format!(
            "the edge columns describe {} nodes, not {n}",
            graph.len()
        ));
    }
    Ok(graph)
}

/// The reference's own rescale contract (`layout.py:646`), on one arm: a one-node graph is
/// the origin (`layout.py:618-624`), and anything larger is centred and spans exactly
/// `scale`, whatever start it was given.
fn rescale_contract(points: &[(f64, f64)], scale: f64, label: &str) -> Result<(), String> {
    if points.len() == 1 {
        return match points[0] {
            (0.0, 0.0) => Ok(()),
            at => Err(format!(
                "{label}: a one-node graph is the origin, not {at:?}"
            )),
        };
    }
    let extent = centred_extent(points);
    let slack = EXTENT_SLACK * f64::max(1.0, scale);
    match (extent - scale).abs() <= slack {
        true => Ok(()),
        false => Err(format!(
            "{label}: spans {extent}, not the requested scale {scale}"
        )),
    }
}

/// The largest magnitude over both axes after subtracting each axis's mean — the
/// reference's own `lim` (`layout.py:1900-1920`). Summed in ascending index order (D3).
pub(super) fn centred_extent(points: &[(f64, f64)]) -> f64 {
    let count = points.len() as f64;
    let (mut mx, mut my) = (0.0, 0.0);
    for &(x, y) in points {
        mx += x;
        my += y;
    }
    let (mx, my) = (mx / count, my / count);
    let mut extent = 0.0_f64;
    for &(x, y) in points {
        extent = extent.max((x - mx).abs()).max((y - my).abs());
    }
    extent
}

/// One arm's node positions: the `spring` `{x, y}` columns, as pairs.
fn pairs(case: &Value) -> Result<Vec<(f64, f64)>, String> {
    let read = |axis: &str| -> Result<Vec<f64>, String> {
        case["spring"][axis]
            .as_array()
            .ok_or_else(|| format!("no spring.{axis} column"))?
            .iter()
            .map(|v| {
                v.as_f64()
                    .ok_or_else(|| format!("spring.{axis} is not a number"))
            })
            .collect()
    };
    let (x, y) = (read("x")?, read("y")?);
    if x.len() != y.len() {
        return Err("the two columns hold different counts".into());
    }
    Ok(x.into_iter().zip(y).collect())
}

fn nodes(case: &Value) -> Result<usize, String> {
    case["n"]
        .as_u64()
        .map(|n| n as usize)
        .ok_or_else(|| "no n".into())
}

fn seed(case: &Value) -> Result<u32, String> {
    case["seed"]
        .as_u64()
        .map(|s| s as u32)
        .ok_or_else(|| "no seed".into())
}

/// The fixture must be the one this manifest fingerprinted, or the arm measured other
/// graphs than the ones the tree's own stamp covers.
pub(super) fn check_manifest(manifest: &Value, case: &Value) -> Result<(), String> {
    let stamped = manifest["seeds"].as_u64().unwrap_or(0);
    if seed(case)? as u64 >= stamped {
        return Err("a case past the manifest's seed count: re-emit the fixtures".into());
    }
    Ok(())
}

fn reader(dir: &Path, name: &str) -> Result<BufReader<File>, String> {
    File::open(dir.join(name))
        .map(BufReader::new)
        .map_err(|e| format!("{name}: {e}"))
}

fn parse(line: &str) -> Result<Value, String> {
    serde_json::from_str(line).map_err(|e| format!("a case line is not json: {e}"))
}

/// The next non-empty line, or `None` at end of file. Blank lines are skipped rather than
/// parsed, so a trailing newline is not a case.
trait NextLine {
    fn next_line(&mut self) -> Result<Option<String>, String>;
}

impl NextLine for BufReader<File> {
    fn next_line(&mut self) -> Result<Option<String>, String> {
        for line in self.lines() {
            let text = line.map_err(|e| e.to_string())?;
            if !text.trim().is_empty() {
                return Ok(Some(text));
            }
        }
        Ok(None)
    }
}

/// The `q` quantile of an ascending vector, by the nearest-rank reading (no interpolation):
/// a measurement this gate reports is a measurement, not a smoothed one.
pub(super) fn percentile(sorted: &[f64], q: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let rank = (q * sorted.len() as f64).ceil().max(1.0) as usize;
    sorted[rank.min(sorted.len()) - 1]
}
