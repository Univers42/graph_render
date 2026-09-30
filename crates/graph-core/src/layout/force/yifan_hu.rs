//! `layout.force.yifan_hu`: multilevel force layout. The graph is coarsened by repeated
//! matching, the coarsest level is laid out by the Barnes-Hut force simulation, and each
//! finer level starts from its parent's positions and is refined by the same simulation.
//!
//! This is NOT Graphviz `sfdp`. It borrows sfdp's multilevel idea (Hu 2005) and nothing
//! else: sfdp's own force model (the `K^2 / d` repulsion, adaptive step, its coarsening
//! by edge collapsing with weights) is not reproduced, and no `sfdp` output was
//! compared against. The per-level solver is `layout::force::barnes_hut`, reused, not a
//! second tick (`prompts/phase-06-iterative-spectral-mds.md`, section 4).

mod coarsen;
#[cfg(test)]
mod tests;

use super::barnes_hut::{golden_seed, settle};
use super::params::{ForceParams, TICKS};
use super::{SimpleGraph, simple_graph};
use crate::index::Topology;
use crate::layout::Geometry;
use crate::stage::{Stage, StageError};
use coarsen::coarsen;
use graph_contract::geometry::{EdgeGeometry, NodeGeometry};

/// Coarsening stops at or below this many nodes.
const MIN_COARSE: u32 = 16;
/// Hard bound on the number of coarsening steps.
const MAX_LEVELS: usize = 12;
/// Ticks and starting heat for each refinement of a finer level.
const REFINE: (u32, f64) = (48, 0.3);
const GOLDEN_ANGLE: f64 = 2.399963229728653;

/// Multilevel force layout over the Barnes-Hut simulation.
///
/// Ponytail: coarsening is a greedy matching and the refinement budget (48 ticks from
/// alpha 0.3) is fixed, not adaptive; a long, thin graph whose coarse layout is folded
/// can keep the fold, since refinement is local. It is not sfdp, so positions and
/// scale differ from Graphviz's for the same graph. Direction: cosmetic. Escape hatch:
/// `layout.force.barnes_hut` for a single-level solve.
pub struct YifanHu;

impl Stage for YifanHu {
    type Params = ForceParams;
    const ID: &'static str = "layout.force.yifan_hu";

    fn run(topology: &Topology, params: &Self::Params) -> Result<Geometry, StageError> {
        let (x, y) = multilevel(simple_graph(topology), topology.node_count(), *params);
        if x.iter().chain(&y).any(|v| !v.is_finite()) {
            return Err(StageError::NonFinite { column: "node.x" });
        }
        Ok(Geometry {
            nodes: NodeGeometry::Point {
                x: x.iter().map(|&v| v as f32).collect(),
                y: y.iter().map(|&v| v as f32).collect(),
            },
            edges: EdgeGeometry::Line,
            notes: Vec::new(),
        })
    }
}

/// Graphs finest first, and the map from each level's nodes to the next one's.
fn hierarchy(fine: SimpleGraph, n: u32) -> (Vec<(SimpleGraph, u32)>, Vec<Vec<u32>>) {
    let mut levels = vec![(fine, n)];
    let mut maps = Vec::new();
    while levels.len() < MAX_LEVELS {
        let (g, count) = levels.last().expect("one level");
        // Stall guard: under 10 percent shrink is not worth another level.
        if *count <= MIN_COARSE {
            break;
        }
        let c = coarsen(g, *count);
        if u64::from(c.node_count) * 10 > u64::from(*count) * 9 {
            break;
        }
        maps.push(c.map);
        levels.push((c.graph, c.node_count));
    }
    (levels, maps)
}

fn multilevel(fine: SimpleGraph, n: u32, params: ForceParams) -> (Vec<f64>, Vec<f64>) {
    let (levels, maps) = hierarchy(fine, n);
    let (top, top_n) = levels.last().expect("one level").clone();
    let mut pos = settle(top, params, golden_seed(top_n), (TICKS, 1.0));
    for k in (0..maps.len()).rev() {
        let start = prolong(&pos, &maps[k], params.link_distance);
        pos = settle(levels[k].0.clone(), params, start, REFINE);
    }
    pos
}

/// Each fine node starts at its coarse parent, nudged by a golden-angle offset of its
/// own index so two children of one parent do not coincide.
fn prolong(coarse: &(Vec<f64>, Vec<f64>), map: &[u32], link_distance: f64) -> (Vec<f64>, Vec<f64>) {
    let mut x = Vec::with_capacity(map.len());
    let mut y = Vec::with_capacity(map.len());
    for (i, &p) in map.iter().enumerate() {
        let angle = i as f64 * GOLDEN_ANGLE;
        x.push(coarse.0[p as usize] + 0.25 * link_distance * libm::cos(angle));
        y.push(coarse.1[p as usize] + 0.25 * link_distance * libm::sin(angle));
    }
    (x, y)
}
