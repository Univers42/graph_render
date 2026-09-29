//! The layout registry (`prompt.md` §8): each layout's capability id, its implementation
//! and the metadata the ledger publishes for it. Every [`Metadata`] field is required and
//! none is an `Option`, so a layout cannot be registered without declaring its tier,
//! stage, geometry, oracle, complexity, `scale_ceiling`, `degradation` and `ponytail`.
//! graph-cli's `capabilities` ledger takes its layout rows from [`LAYOUTS`], and its
//! `hashgate` hashes every layout listed here.

use crate::index::Topology;
use crate::layout::Geometry;
use crate::layout::force::BarnesHut;
use crate::layout::forceatlas2::ForceAtlas2;
use crate::layout::grid::Grid;
use crate::layout::{circle_packing, circular, tidy_tree, treemap};
use crate::stage::{Stage, StageError};
use graph_contract::geometry::{EdgeGeometryKind, NodeGeometryKind};

/// What the ledger says about a layout. Every field is required.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Metadata {
    /// Delivery tier.
    pub tier: u8,
    /// Pipeline stage.
    pub stage: &'static str,
    /// The node geometry kind it emits.
    pub nodes: NodeGeometryKind,
    /// The edge geometry kind it emits.
    pub edges: EdgeGeometryKind,
    /// The reference it is checked against.
    pub oracle: &'static str,
    /// Time complexity, stated and held.
    pub complexity: &'static str,
    /// Node count past which it stops being usable.
    pub scale_ceiling: u64,
    /// What happens past the ceiling.
    pub degradation: &'static str,
    /// Its Ponytail marker, or the reason none is owed.
    pub ponytail: &'static str,
}

/// One registered layout.
#[derive(Debug, Clone, Copy)]
pub struct Capability {
    /// Its capability id, which is also its hash-gate stage.
    pub id: &'static str,
    /// The layout at its default parameters: the run a hashed snapshot is pinned to.
    pub run: fn(&Topology) -> Result<Geometry, StageError>,
    /// Its ledger metadata.
    pub meta: Metadata,
}

/// Node count past which `layout.grid` stops being usable, and why it is this one.
///
/// Estimated, not measured on the target (`crates/graph-core/tests/memory.rs`,
/// `grid_pipeline_memory_per_node`): natively, the topology stage, the grid and the
/// snapshot's bytes peak at **919 B per node** at 100 000 synthetic nodes and 154 978
/// edges (91.9 MB), input records excluded. wasm32 addresses at most 4 GiB, so
/// 4 GiB / 919 B = 4.67 M nodes, rounded down to two figures. The grid's own `u32` limits bind far later: its lattice is exact
/// up to 2^32 − 1 nodes, and the id tables refuse past 2^32 − 1 bytes of text.
pub const GRID_CEILING: u64 = 4_600_000;

const GRID: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Line,
    oracle: "hand: the conventions worked by hand in graph-core layout/grid.rs, restated in f64 \
and checked per seed by graph-cli roundtrip; no third-party grid is a meaningful oracle \
(SciGraphs' _grid_layout fits its scale and starts at the origin)",
    complexity: "O(n)",
    scale_ceiling: GRID_CEILING,
    degradation: "past the ceiling wasm32 cannot allocate and the module traps (no partial \
result); natively, memory permitting, the snapshot refuses with SnapshotError::Capacity once an \
id table's text would pass 2^32-1 bytes — a refusal, never a wrap or a truncation",
    ponytail: "Ponytail (aspect): cols = ceil(sqrt(n)) is a convention, not a computation; when \
cols does not divide n the last row is ragged and the nodes' centroid sits off the origin (n = 3: \
(-1/6, -1/6)). Direction: cosmetic, never wrong — every node gets its own cell. Escape hatch: a \
layout that centres the last row, under its own id. Ponytail (scale_ceiling): an estimate — \
measured natively on 64-bit and projected onto wasm32's 4 GiB; re-measure with \
crates/graph-core/tests/memory.rs",
};

/// Node count past which the three hierarchy layouts (tidy tree, treemap, circular) stop
/// being usable, and why it is this one.
///
/// Measured, not estimated (`crates/graph-core/tests/memory.rs`,
/// `hierarchy_layout_pipeline_memory_per_node`): each holds within a few percent of the
/// grid's own 919 B/node at 100 000 synthetic nodes and 154 978 edges — tidy tree 933 B,
/// treemap 930 B, circular 922 B/node — because all three add only an O(n) hierarchy
/// repair (`layout/hierarchy.rs`) and O(n) geometry over the same topology and snapshot
/// substrate the grid does. wasm32 addresses at most 4 GiB, so 4 GiB / 933 B = 4.60 M
/// nodes at the heaviest of the three (tidy tree), rounded down to two figures, same as
/// `GRID_CEILING`. The hierarchy repair's own `u32` limit binds far later:
/// `Hierarchy::of` needs `n + 1` rows to fit `u32`, i.e. up to 2^32 − 2 nodes.
pub const HIERARCHY_LAYOUT_CEILING: u64 = 4_600_000;

const TIDY_TREE: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Polyline,
    oracle: "d3-hierarchy@3.1.2 tree() — an exact f64 port of tree.js's Buchheim/Jünger/ \
Leipert/Walker algorithm at d3's own defaults (size([1,1]), the default separation), stated in \
the module doc; byte-compared after Math.fround by harness/oracle-layouts.mjs over >=1000 seeds",
    complexity: "O(n)",
    scale_ceiling: HIERARCHY_LAYOUT_CEILING,
    degradation: "past the ceiling wasm32 cannot allocate and the module traps (no partial \
result); natively, memory permitting, the snapshot refuses with SnapshotError::Capacity once an \
id table's text would pass 2^32-1 bytes — a refusal, never a wrap or a truncation",
    ponytail: "No Ponytail on the algorithm: the port is exact, nothing here is a heuristic, an \
estimate or a fallback, so none is owed (module doc). Ponytail (scale_ceiling): measured, not \
estimated — see HIERARCHY_LAYOUT_CEILING's derivation and \
crates/graph-core/tests/memory.rs::hierarchy_layout_pipeline_memory_per_node.",
};

const TREEMAP: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Box,
    edges: EdgeGeometryKind::Line,
    oracle: "d3-hierarchy@3.1.2 treemap().tile(treemapSquarify) — an exact f64 port of \
squarify.js and hierarchy.sum at d3's own defaults (size([1,1]), no padding, no rounding), \
stated in the module doc; byte-compared after Math.fround by harness/oracle-layouts.mjs over \
>=1000 seeds",
    complexity: "O(n log n)",
    scale_ceiling: HIERARCHY_LAYOUT_CEILING,
    degradation: "past the ceiling wasm32 cannot allocate and the module traps (no partial \
result); natively, memory permitting, the snapshot refuses with SnapshotError::Capacity once an \
id table's text would pass 2^32-1 bytes — a refusal, never a wrap or a truncation",
    ponytail: "a non-positive or non-finite weight clamps to WEIGHT_EPSILON (1e-6) rather than \
vanishing or handing squarify a zero/NaN value — the oracle applies the identical clamp. \
Direction: cosmetic under-representation, a hairline never a wrong containment; escape hatch: \
fix the weight upstream (module doc). Ponytail (scale_ceiling): measured, not estimated — see \
HIERARCHY_LAYOUT_CEILING's derivation and \
crates/graph-core/tests/memory.rs::hierarchy_layout_pipeline_memory_per_node.",
};

const CIRCULAR: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Line,
    oracle: "hand: the ring/angle/radius conventions worked by hand in graph-core \
layout/circular.rs (docs/decisions/circular-conventions.md), restated independently in f64 and \
checked per seed by graph-cli roundtrip; no third-party circular/radial hierarchy layout is a \
meaningful byte-for-byte oracle (SciGraphs' own hierarchical.py normalises differently, per that \
decision doc)",
    complexity: "O(n)",
    scale_ceiling: HIERARCHY_LAYOUT_CEILING,
    degradation: "past the ceiling wasm32 cannot allocate and the module traps (no partial \
result); natively, memory permitting, the snapshot refuses with SnapshotError::Capacity once an \
id table's text would pass 2^32-1 bytes — a refusal, never a wrap or a truncation",
    ponytail: "the radius step and the start angle are conventions pinned by this module, not a \
computation with one right answer (docs/decisions/circular-conventions.md). Failing input: a \
ring holding many nodes at a small radius (a shallow, bushy tree) crowds them close together. \
Direction: cosmetic, never wrong — every node keeps its own ring and a distinct slot, so no two \
real nodes ever collide. Escape hatch: a variant that inflates the radius by ring population, \
under its own id (module doc). Ponytail (scale_ceiling): measured, not estimated — see \
HIERARCHY_LAYOUT_CEILING's derivation and \
crates/graph-core/tests/memory.rs::hierarchy_layout_pipeline_memory_per_node.",
};

/// Node count past which `layout.packing.circle` stops being usable, and why it is this
/// one — a different shape of ceiling than the other three, and much lower.
///
/// Labelled, not a hard memory wall: the exact Collins–Stephenson path (genuinely planar
/// input) is close to linear in `n`, like the other three layouts. But a random or dense
/// graph at synthetic-model density is essentially always non-planar (the planar bound is
/// `m <= 3n - 6`), so the realistic case takes `circle_packing/fallback.rs`'s two O(n^2)
/// relaxation passes. Measured natively, `--release`
/// (`crates/graph-core/tests/memory.rs::circle_packing_pipeline_memory_per_node`): one
/// pipeline call takes 304 ms at n=300, 2.81 s at n=1000, 22.63 s at n=3000 — the O(n^2)
/// shape shows in the timing and in peak memory, which does *not* hold flat per node the
/// way the other three layouts' does (3.2 KB/node at n=300 rising to 24.9 KB/node at
/// n=3000). Fitting that quadratic, a single call already crosses a 1-second budget
/// around n=600. 5,000 is a round, stated cutoff at which a fallback packing already
/// costs tens of seconds even natively; it is not measured directly at that size because
/// doing so is itself impractically slow — the same reason this ceiling exists.
pub const PACKING_CEILING: u64 = 5_000;

const PACKING: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Circle,
    edges: EdgeGeometryKind::Line,
    oracle: "hand + planarity certificate: the Collins-Stephenson exact path is checked against \
its own Euler-formula certificate (planarity::planar_embedding + triangulate_embedding); the \
per-seed hand oracle (graph-cli roundtrip) restates finite radii, no NaN/Inf, and edge tangency \
within tolerance whenever note code 3 is absent — no third-party packer is pinned to this exact \
convention",
    complexity: "O(n) exact path; O(n^2) per relaxation round on the non-planar fallback",
    scale_ceiling: PACKING_CEILING,
    degradation: "past the ceiling the fallback still runs and still returns finite geometry, \
never a refusal or a trap — it simply gets slower at O(n^2), with no built-in cutoff, so a \
caller must apply its own timeout; the exact planar path is unaffected and stays fast at any n \
this crate's u32 index space allows",
    ponytail: "the packing is exact only for planar input. The failing input is any graph with a \
K5 or K3,3 minor (or one whose planar embedding cannot be triangulated into a genuine disk, \
treated the same defensively). Direction: overlap, the dangerous one — the fallback does not \
guarantee tangency or non-overlap either. Escape hatch: read note code 3 off the snapshot; its \
absence is the only trustworthy sign the packing is exact (module doc; full account in \
docs/decisions/planarity-fallback.md). Ponytail (scale_ceiling): labelled, time-bound, not \
measured at the ceiling itself — see PACKING_CEILING's derivation.",
};

/// Node count past which `layout.force.barnes_hut` stops being usable, and why it is
/// this one.
///
/// **Time-bound, measured, not memory-bound** (`docs/measurements/phase06-force.md`;
/// reproduce with `cargo run --release --example force_dump -- seed 0 <n> barnes_hut`,
/// which times `Stage::run` only). Natively, release, x86_64, inside the toolchain
/// image, at 220 nodes / 329 edges 17.15 ms, 10 000 / 15 474 edges 2 271.35 ms, and
/// 100 000 / 154 978 edges **46 781.60 ms**. Theta-approximated many-body is
/// `O(n log n)` per tick and there are a fixed 112 of them, so 100 000 sits inside a
/// 60-second budget and 200 000 — extrapolated, *not* run, because it is itself
/// impractically slow — lands near 110 s and past it. 100 000 is round and stated at
/// the size actually measured.
///
/// Ponytail (scale_ceiling): the extrapolation past the measured point is an `O(n log n)`
/// reading of two ratios (45x nodes -> 132x time; 10x nodes -> 20.6x time, the latter
/// above `n log n`'s own ~12x because quadtree depth and collide's fixed-radius
/// neighbour queries both grow with n), not a measurement at 200 000. Unlike the grid's
/// and the hierarchy layouts' ceilings, this one is *not* a memory wall and is *not*
/// projected onto wasm32's 4 GiB: a caller inside wasm32 gets the same time curve in a
/// slower machine, with no new refusal to detect it by. The `u32` index space binds
/// far later (2^32 nodes).
pub const FORCE_CEILING: u64 = 100_000;

/// Node count past which `layout.forceatlas2` stops being usable, and why it is this
/// one — two orders of magnitude below [`FORCE_CEILING`], and for the same structural
/// reason the two ceilings differ at all.
///
/// **Time-bound, measured** (`docs/measurements/phase06-force.md`, same command with
/// `fa2`): 220 / 329 edges 6.58 ms, 1 000 / 1 541 edges 131.53 ms, 2 000 / 3 075 edges
/// 524.82 ms, 5 000 / 7 721 edges 3 301.05 ms, 10 000 / 15 474 edges **13 522.24 ms**.
/// The 5 000 -> 10 000 step is 4.09x for exactly 2x the nodes: this is a clean `O(n^2)`
/// (networkx 3.6's dense all-pairs repulsion, ported verbatim — there is no spatial
/// approximation in the algorithm being ported, unlike Barnes-Hut's theta-tree). Under a
/// 30-second budget, `10 000 * sqrt(30 / 13.522) = 14 880`, rounded down to two figures
/// and to a round 14 000. 14 000 is *not* run: 13.5 s is already the slowest thing this
/// branch measures, and the point is the shape, not the digit.
///
/// Ponytail (scale_ceiling): the projection past the last measured point assumes the
/// `O(n^2)` the two measured ratios support (3.97x and 4.09x for 2x nodes) holds at
/// larger n. It is the *algorithm's* cost, not this port's: `repulsion()` visits each
/// unordered pair once and never materialises an `n x n` matrix, so this is a time
/// ceiling and not a memory one. Past it nothing refuses; the layout keeps returning
/// finite geometry, just for tens of seconds and growing quadratically, so a caller must
/// apply its own timeout — the honest ceiling is a budget, and a budget needs a
/// timeout to be real.
pub const FA2_CEILING: u64 = 14_000;

const BARNES_HUT: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Line,
    oracle: "d3-force@3.0.0 src/{manyBody,link,center,collide}.js — a port of the frozen force \
    set (theta 0.9, charge -90, distanceMax 520, linkDistance 60, collideRadius 16, alphaDecay \
    0.06, velocityDecay 0.42) at TICKS=112, in Jacobi/gather form with a counter-based jiggle; \
    differentially compared by the d3-force arm of harness/oracle-layouts.mjs over >=1000 seeds. \
    Identity is NOT claimed and cannot be: link and collide are Jacobi gathers where d3 scatters \
    in visit order, and a force simulation amplifies a 1-ULP difference into a different \
    picture, so the gate is the stress metric (Pearson hop/euclid correlation over 32 max-min \
    pivots, margin -0.05 vs d3) — 'different, but not worse', measured in \
    docs/measurements/phase06-stress.md",
    complexity: "O(n log n) per tick x TICKS=112, so O(112 n log n) overall; collide's \
    fixed-radius neighbour query and the quadtree's depth growth sit on top of the \
    many-body term's own n log n",
    scale_ceiling: FORCE_CEILING,
    degradation: "past the ceiling there is no refusal and no trap: the layout still returns \
    finite geometry, it just takes longer than a 60-second budget and keeps growing — the caller \
    must apply its own timeout, exactly as at the ceiling itself. A non-finite position (D9) \
    refuses with StageError::NonFinite rather than reaching the snapshot",
    ponytail: "force layouts are CHAOTIC: the same graph with one node added or removed is a \
    different picture, not a perturbed one, and there is no failing input narrower than 'any \
    topology change'. Direction: cosmetic-but-surprising, never silently wrong — this layout is \
    graded on the stress metric, not on visual stability with the input. Escape hatch: a fixed \
    seed, and this stage's own determinism (the same topology run twice settles to the same \
    geometry every time, 4-way hash equal). Ponytail (theta): the opening angle trades accuracy \
    for speed; failing input is two dense well-separated clusters whose combined bounding box is \
    small relative to a far node's distance from them, where the tree treats a whole cluster as \
    one point mass too eagerly; direction is OVER-CLUMPING (distant structure collapsing \
    together), and theta is a frozen engine constant here, not a per-call knob. \
    Ponytail (scale_ceiling): time-bound and measured at 100 000 only; 200 000 is an O(n log n) \
    extrapolation, not a run",
};

const FA2: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Line,
    oracle: "networkx 3.6 forceatlas2_layout (networkx/drawing/layout.py:1604-1875) — a port of \
    the whole function at its default configuration (linlog=False, distributed_action=False, \
    strong_gravity=False, adjust_sizes=False, dim=2, weight=None), with swing/traction carried \
    cumulatively across iterations as the reference itself does; differentially compared against \
    the real library in the ge-python-oracle image (pinned networkx 3.6, numpy, scipy 1.16.2) \
    over >=1000 seeds, in harness/oracle-fa2.py. Two deviations from the reference, both stated \
    rather than hidden: initial positions come from graph-core's own seeded Mulberry32 instead \
    of numpy's global RNG (D5 — there is no global RNG to reach for), and an exact coincidence \
    (d2 == 0) is nudged apart by the counter hash so no Infinity/NaN factor can reach a node \
    (D9), which the reference's dense form has no guard for",
    complexity: "O(n^2) per iteration x max_iter=100, so O(100 n^2) worst case; attraction is \
    O(m), gravity is O(n), and neither changes the shape",
    scale_ceiling: FA2_CEILING,
    degradation: "past the ceiling there is no refusal and no trap either: the dense all-pairs \
    repulsion still returns finite geometry, it just takes tens of seconds and keeps growing \
    quadratically (13.5 s measured at 10 000 nodes), so the caller must apply its own timeout. \
    A non-finite position (D9) refuses with StageError::NonFinite rather than reaching the \
    snapshot",
    ponytail: "force layouts are CHAOTIC, identically to Barnes-Hut: one added node is a \
    different picture, not a perturbed one. Direction: cosmetic-but-surprising, never silently \
    wrong; the escape hatch is the same — a fixed seed and the stage's own run-to-run \
    determinism. Ponytail (early exit): the iteration count is not fixed the way Barnes-Hut's \
    TICKS is: run() stops as soon as a tick's total movement falls under networkx's own 1e-10, \
    so a quiet graph finishes in fewer than 100 iterations and two graphs of the same size can \
    do different amounts of work. Ponytail (scale_ceiling): time-bound, and the projection past \
    10 000 assumes the O(n^2) the measured ratios support holds at larger n",
};

/// Every registered layout, in the order the hash gate runs them.
pub static LAYOUTS: [Capability; 7] = [
    Capability {
        id: Grid::ID,
        run: run_default::<Grid>,
        meta: GRID,
    },
    Capability {
        id: "layout.tree.tidy",
        run: tidy_tree::run,
        meta: TIDY_TREE,
    },
    Capability {
        id: "layout.treemap.squarified",
        run: treemap::run,
        meta: TREEMAP,
    },
    Capability {
        id: "layout.circular.radial",
        run: circular::run,
        meta: CIRCULAR,
    },
    Capability {
        id: "layout.packing.circle",
        run: circle_packing::run,
        meta: PACKING,
    },
    Capability {
        id: BarnesHut::ID,
        run: run_default::<BarnesHut>,
        meta: BARNES_HUT,
    },
    Capability {
        id: ForceAtlas2::ID,
        run: run_default::<ForceAtlas2>,
        meta: FA2,
    },
];

/// The layout registered under `id`.
pub fn find(id: &str) -> Option<&'static Capability> {
    LAYOUTS.iter().find(|layout| layout.id == id)
}

fn run_default<S: Stage>(topology: &Topology) -> Result<Geometry, StageError> {
    S::run(topology, &S::Params::default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::grid::GridParams;
    use crate::stage::{gate_node_count, run_with, seeded_model};
    use crate::weights::REFERENCE_DEGREE;

    #[test]
    fn every_layout_is_a_layout_stage_with_its_metadata_filled() {
        let mut ids: Vec<_> = LAYOUTS.iter().map(|layout| layout.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), LAYOUTS.len(), "unique ids");
        for layout in &LAYOUTS {
            let m = layout.meta;
            assert!(layout.id.starts_with("layout."), "{}", layout.id);
            assert_eq!(m.stage, "layout");
            assert!(m.scale_ceiling > 0, "{}", layout.id);
            for text in [m.oracle, m.complexity, m.degradation, m.ponytail] {
                assert!(!text.trim().is_empty(), "{}", layout.id);
            }
        }
    }

    #[test]
    fn the_force_layouts_are_registered_with_the_ceilings_this_branch_measured() {
        let bh = find("layout.force.barnes_hut").expect("barnes-hut registered");
        let fa2 = find("layout.forceatlas2").expect("fa2 registered");
        assert_eq!(
            (bh.meta.nodes, bh.meta.edges),
            (NodeGeometryKind::Point, EdgeGeometryKind::Line)
        );
        assert_eq!(
            (fa2.meta.nodes, fa2.meta.edges),
            (NodeGeometryKind::Point, EdgeGeometryKind::Line)
        );
        assert_eq!(bh.meta.scale_ceiling, FORCE_CEILING);
        assert_eq!(fa2.meta.scale_ceiling, FA2_CEILING);
        assert!(
            bh.meta.complexity.contains("O(n log n)"),
            "{}",
            bh.meta.complexity
        );
        assert!(
            fa2.meta.complexity.contains("O(n^2)"),
            "{}",
            fa2.meta.complexity
        );
        // The two ceilings differ by the algorithmic shape, not by taste: theta-
        // approximated many-body against networkx's dense all-pairs form. The measured
        // ratio is 100_000 / 14_000 = 7.14x, so pin "materially below" at 5x rather
        // than inventing a round factor the measurements do not support. A row that
        // claimed one number for both would hide the whole point of shipping both.
        const {
            assert!(
                FA2_CEILING * 5 < FORCE_CEILING,
                "the dense FA2 ceiling must sit materially below the theta-tree one"
            );
        }
        for text in [bh.meta.oracle, fa2.meta.oracle] {
            assert!(
                text.contains("d3-force") || text.contains("networkx"),
                "{text}"
            );
        }
        for text in [bh.meta.degradation, fa2.meta.degradation] {
            assert!(
                text.contains("Barnes-Hut") || text.contains("refus"),
                "{text}"
            );
        }
        for text in [bh.meta.ponytail, fa2.meta.ponytail] {
            // The chaos marker is spelled in caps in both rows, as the phase prompt
            // requires force layouts to name it; match it case-insensitively.
            assert!(
                text.to_lowercase().contains("chaotic"),
                "a force layout must name the chaos, its direction and its escape hatch: {text}"
            );
            assert!(
                text.contains("Escape hatch") || text.contains("escape hatch"),
                "{text}"
            );
        }
    }

    #[test]
    fn a_registered_layout_emits_the_kinds_it_declares_at_its_default_parameters() {
        let (nodes, edges) = seeded_model(5, gate_node_count(5), REFERENCE_DEGREE);
        for layout in &LAYOUTS {
            let run = run_with(&nodes, &edges, layout.id, layout.run).expect("runs");
            let header = run.snapshot.header();
            assert_eq!(
                (header.node_kind, header.edge_kind),
                (layout.meta.nodes, layout.meta.edges)
            );
        }
        let grid = find("layout.grid").expect("registered");
        let by_hand = run_with(&nodes, &edges, "layout.grid", |t| {
            Grid::run(t, &GridParams::default())
        });
        assert_eq!(run_with(&nodes, &edges, grid.id, grid.run), by_hand);
        assert!(find("layout.none").is_none());
    }
}
