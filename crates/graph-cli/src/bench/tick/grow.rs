//! `graph-cli tick --grow <BATCH>`: the wall time of carrying a live force session from a
//! smaller topology onto a bigger one, which is what a host sees when a batch of nodes and
//! their edges arrive mid-stage. The layout that already ran is not thrown away and rebuilt;
//! [`ForceSession::carry`] re-maps the old positions onto the new rows and keeps going.
//!
//! What is inside the timer, and separately timed: building the new topology with
//! `index_model` (its own column), and the carry itself. What is deliberately outside: the
//! seeded model, the old topology — the first `n - batch` nodes and only the edges with
//! both endpoints among them — the frozen session, and the three warm ticks that give it a
//! quadtree before any timing.
//!
//! Caveat: this times one carry into a fresh topology over an already-stepped session, not
//! a re-index of the old one and not a carry followed by a tick, so the number is the map and
//! nothing else. It is the median of three runs on a wall clock, on whatever host is
//! measured — the load average is printed on the row rather than assumed idle.

use crate::bench::campaign::median;
use crate::bench::tiers::markdown::loadavg;
use graph_core::layout::force::{ForceParams, ForceSession};
use graph_core::{EdgeRecord, NodeRecord, REFERENCE_DEGREE, Topology, index_model, seeded_model};
use std::collections::HashSet;
use std::process::ExitCode;
use std::time::Instant;

/// How many timed repetitions each of the two timed halves is measured over.
const REPEATS: usize = 3;

/// The table's header, printed once above the row.
pub const HEADER: &str = "| n | batch | m new | index ms | carry median ms | carry min ms | carry max ms | carried | new | load start | load end |\n|---|---|---|---|---|---|---|---|---|---|---|";

/// Exit 0 with the table on standard output, or 2 when the model or the carry failed.
pub fn report(n: u32, seed: u32, batch: u32) -> ExitCode {
    match measure(n, seed, batch) {
        Ok(row) => {
            println!("{HEADER}\n{row}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("grow: could not run: {e}");
            ExitCode::from(2)
        }
    }
}

/// One row: the untimed setup, then `REPEATS` of (index the whole model, carry onto it).
fn measure(n: u32, seed: u32, batch: u32) -> Result<String, String> {
    if batch >= n {
        return Err(format!(
            "n={n}, batch={batch}: the carry needs a node left behind"
        ));
    }
    let load_start = loadavg();
    let (nodes, edges) = seeded_model(seed, n, REFERENCE_DEGREE);
    let (old_nodes, old_edges) = prefix(&nodes, &edges, n - batch);
    let old = index_model(&old_nodes, &old_edges).map_err(|e| format!("old n={n}: {e}"))?;
    let mut session = ForceSession::from_frozen(&old, &ForceParams::default())
        .map_err(|e| format!("old n={n}: {e}"))?;
    session.step(3);
    let carried = old.node_count();
    let Samples {
        index_ms,
        carry_ms,
        new_nodes,
        new_edges,
    } = timed(&nodes, &edges, &old, &mut session)?;
    let (carry_min, carry_max) = span(&carry_ms);
    Ok(format!(
        "| {n} | {batch} | {new_edges} | {:.2} | {:.2} | {carry_min:.2} | {carry_max:.2} | \
         {carried} | {new_nodes} | {load_start} | {} |",
        median(index_ms),
        median(carry_ms),
        loadavg(),
    ))
}

/// The two sample lists, plus the whole model's shape as the last carry saw it.
struct Samples {
    /// One index time per repetition.
    index_ms: Vec<f64>,
    /// One carry time per repetition.
    carry_ms: Vec<f64>,
    /// Nodes in the whole model.
    new_nodes: u32,
    /// Edges in the whole model.
    new_edges: u32,
}

/// `REPEATS` of the two timed halves, one new topology per repetition: the index time and
/// the carry time that ran on that topology. Each is a separate sample list.
fn timed(
    nodes: &[NodeRecord],
    edges: &[EdgeRecord],
    old: &Topology,
    session: &mut ForceSession,
) -> Result<Samples, String> {
    let mut out = Samples {
        index_ms: Vec::with_capacity(REPEATS),
        carry_ms: Vec::with_capacity(REPEATS),
        new_nodes: 0,
        new_edges: 0,
    };
    for _ in 0..REPEATS {
        let started = Instant::now();
        let new = index_model(nodes, edges).map_err(|e| format!("n={}: {e}", nodes.len()))?;
        out.index_ms.push(ms_since(started));
        let started = Instant::now();
        session
            .carry(old, &new)
            .map_err(|e| format!("carry onto n={}: {e}", new.node_count()))?;
        out.carry_ms.push(ms_since(started));
        (out.new_nodes, out.new_edges) = (new.node_count(), new.edge_count());
    }
    Ok(out)
}

/// The first `keep` nodes, and only the edges with both endpoints among them. Node order is
/// the model's own, so the kept prefix keeps the dense rows `index_model` gave it, and that
/// is what lets the carry map ids across the two topologies.
fn prefix(
    nodes: &[NodeRecord],
    edges: &[EdgeRecord],
    keep: u32,
) -> (Vec<NodeRecord>, Vec<EdgeRecord>) {
    let kept: Vec<NodeRecord> = nodes.iter().take(keep as usize).cloned().collect();
    let ids: HashSet<&str> = kept.iter().map(|n| n.id.as_str()).collect();
    let edges: Vec<EdgeRecord> = edges
        .iter()
        .filter(|e| ids.contains(e.source.as_str()) && ids.contains(e.target.as_str()))
        .cloned()
        .collect();
    (kept, edges)
}

/// The lowest and highest sample, in that order; an empty list is `(0.0, 0.0)`.
fn span(samples: &[f64]) -> (f64, f64) {
    samples
        .iter()
        .fold((f64::INFINITY, 0.0_f64), |(lo, hi), &s| {
            (lo.min(s), hi.max(s))
        })
}

/// Milliseconds since `start`.
fn ms_since(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1e3
}
