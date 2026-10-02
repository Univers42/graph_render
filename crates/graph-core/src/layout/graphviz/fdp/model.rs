//! The working state `fdp` iterates on, and the seeded placement it starts from.
//!
//! The reference keeps this in a *derived* graph (`layout.c:380-527`) whose nodes carry a
//! `dndata` record of `deg`, `wdeg` and an incremental displacement. For a flat graph with
//! no clusters the derived graph is the input graph plus one derived node per input node
//! and one derived edge per undirected pair, so this module holds that state directly: two
//! position columns, two displacement columns, and the deduplicated edge list.
//!
//! The edges are deduplicated and oriented **tail < head**, which is not what the reference
//! does — it orients by node pointer (`layout.c:471-472`). The pair is arbitrary:
//! `applyAttr` is exactly antisymmetric, so `(p, q)` and `(q, p)` deposit the same two
//! additions with the same signs. Only the *order* of the additions into the displacement
//! column is a free choice, and that order is fixed here by edge index — the DOT declaration
//! order — held in a CSR so a node's out-edges are one contiguous ascending run. That CSR is
//! what the two force passes iterate, because the reference iterates *a node and then its
//! out-edges* (`tlayout.c:373-377`, `xlayout.c:203-212`) and not the flat edge list, and that
//! nesting is the order the additions land in.

use std::collections::BTreeSet;

use crate::index::Topology;

use super::rng::Rand48;
use super::{EXP_FACTOR, K, START_SEED};

/// Positions, displacements and the deduplicated edges, one slot per node.
pub(super) struct Model {
    /// Node x in inches — the reference computes in inches throughout and multiplies by 72
    /// only when it reports (`finalCC`, `layout.c:169-172`).
    pub(super) x: Vec<f64>,
    /// Node y, in inches.
    pub(super) y: Vec<f64>,
    /// This tick's x displacement, zeroed at the head of every pass.
    pub(super) dx: Vec<f64>,
    /// This tick's y displacement.
    pub(super) dy: Vec<f64>,
    /// One `(tail, head)` pair per undirected edge, `tail < head`, in declaration order.
    pub(super) edges: Vec<(u32, u32)>,
    /// CSR offsets into [`Model::out`], one per node plus a sentinel, so a node's
    /// out-edges are one contiguous run.
    pub(super) out_at: Vec<u32>,
    /// Each node's out-edge heads, ascending, in declaration order.
    pub(super) out: Vec<u32>,
    /// Node count, `nG` in the reference's terms.
    pub(super) count: u32,
}

impl Model {
    /// `fdp_init_node_edge` + `initPositions`: the derived graph, then the seeded random
    /// placement of every node in the box of half-extent `Wd = Ht`.
    ///
    /// `initPositions` with no ports and no `pos` attributes takes its last branch
    /// (`tlayout.c:558-563`): two `drand48` draws per node, x before y, in node order,
    /// mapped onto `[-Wd, Wd] x [-Ht, Ht]`. `Wd = Ht = EXPFACTOR * (K * (sqrt(n) + 1) / 2)`
    /// is the only place the node count enters, and it is `sqrt` and not a loop, so the
    /// whole placement is one ordered gather pass (D10).
    pub(super) fn new(topology: &Topology) -> Self {
        let count = topology.node_count();
        let mut model = Self {
            x: vec![0.0; count as usize],
            y: vec![0.0; count as usize],
            dx: vec![0.0; count as usize],
            dy: vec![0.0; count as usize],
            edges: Vec::new(),
            out_at: vec![0; count as usize + 1],
            out: Vec::new(),
            count,
        };
        model.edges = undirected(topology);
        model.index_out_edges();
        model.place();
        model
    }

    /// Fill the CSR from the edge list, so a node's out-edges are the heads it is the low
    /// endpoint of.
    ///
    /// The reference's derived graph orients an edge by node **pointer**
    /// (`layout.c:471-472`), so a node's out-edges there are an arbitrary subset of its
    /// neighbours. Orienting every edge low-to-high instead is the same set of forces —
    /// `applyAttr` is exactly antisymmetric — and it makes the per-node run ascending,
    /// which is a fixed order whatever the allocator did.
    fn index_out_edges(&mut self) {
        let count = self.count as usize;
        let mut degree = vec![0_u32; count];
        for &(tail, _) in &self.edges {
            degree[tail as usize] += 1;
        }
        let mut at = 0;
        for (node, &d) in degree.iter().enumerate() {
            self.out_at[node] = at;
            at += d;
        }
        self.out_at[count] = at;
        self.out = vec![0; at as usize];
        let mut cursor = self.out_at.clone();
        for &(tail, head) in &self.edges {
            self.out[cursor[tail as usize] as usize] = head;
            cursor[tail as usize] += 1;
        }
    }

    /// The seeded placement. `srand48(START_SEED)` is `tlayout.c:487`; the seed is the
    /// `-Gstart` value, and for this engine it is **not** inert — see the module header.
    fn place(&mut self) {
        let half = EXP_FACTOR * (K * ((self.count as f64).sqrt() + 1.0) / 2.0);
        let mut rng = Rand48::new(START_SEED);
        for i in 0..self.count as usize {
            self.x[i] = half * (2.0 * rng.next_f64() - 1.0);
            self.y[i] = half * (2.0 * rng.next_f64() - 1.0);
        }
    }

    /// Zero both displacement columns. Every pass begins here, which is what makes the
    /// accumulation gather form: nothing is ever read back from the previous tick.
    pub(super) fn clear_displacement(&mut self) {
        self.dx.fill(0.0);
        self.dy.fill(0.0);
    }
}

/// The graph's undirected edges, deduplicated and oriented low-to-high.
///
/// The reference's derived graph takes one edge per `agfstout` walk with `hd == tl`
/// skipped (`layout.c:464-478`), so a self-loop drops out and a pair that cgraph already
/// stored once stays once. `Topology` may hold both directions of a pair, so the dedup is
/// by unordered key. The key set is a `BTreeSet` and the output a `Vec`, so neither the
/// membership test nor the emission order can depend on a hash map's iteration order (D2).
fn undirected(topology: &Topology) -> Vec<(u32, u32)> {
    let columns = topology.edges();
    let mut seen = BTreeSet::new();
    let mut pairs: Vec<(u32, u32)> = Vec::with_capacity(topology.edge_count() as usize);
    for e in 0..topology.edge_count() as usize {
        let (a, b) = (columns.source[e], columns.target[e]);
        let pair = (a.min(b), a.max(b));
        if a != b && seen.insert(pair) {
            pairs.push(pair);
        }
    }
    pairs
}
