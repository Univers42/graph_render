//! Level of detail (`prompts/phase-09-scale-bench.md` §2): given a viewport and a node
//! count, decide what is worth drawing — and hand the front **hints as columns**, never a
//! mutated topology. The motor does not destroy data to make rendering cheaper; a front
//! may ignore every hint here and still be correct, only slower.
//!
//! The thresholds and the shapes are the reference's (`SciGraphs/engine/scigraphs_engine/
//! lod.py`): a tier per node from its on-screen size, a tier per edge from its on-screen
//! width, and a budget filled greedily in descending order of an importance key, with the
//! rule that the mask is never empty. Two deliberate differences, both forced by having
//! no camera: the "screen size" here is **world units inside the viewport rectangle**
//! (2D, no projection matrix, no perspective), and the tier is chosen from the **node
//! count** rather than from a projected radius, because a headless motor has no pixels.
//! What survives the translation is the part that is a policy — the budget rule and the
//! never-empty guarantee — not the numbers, which are restated as [`LodParams`] fields
//! rather than frozen here.
//!
//! **Ponytail: these thresholds are a heuristic, and the direction it fails in is the
//! dangerous one.** Failing input: a graph whose important nodes are low-degree — a
//! dependency graph's entry points, a star's hub the budget happens to rank low, a
//! graph whose meaning lives in a handful of `degree == 1` nodes — where a degree-ranked
//! label budget hides exactly what a reader came for. Direction: **hiding meaningful
//! nodes**, which is the direction that loses information rather than merely losing
//! polish. Escape hatch: the hints are advisory — a front that ignores [`Hints`] draws
//! everything and is slower, never wrong. Second heuristic, same shape: edge decimation
//! is a stride over edge index, so a graph whose edges are laid out so that one stride
//! class carries every long-range edge loses all of its long-range structure.
//!
//! Cost: `O(n + m)`, one pass each, no spatial structure. Phase 8's `grid_index` is the
//! substrate a *culling* query wants, and it is not on this branch's base
//! (`docs/reports/phase-09-progress.md` records this as a deviation, with the second
//! spatial structure deliberately **not** built in its place); a viewport test per node
//! is the honest `O(n)` answer until that index exists.

use crate::index::Topology;

/// What a front is told to draw. Three masks and a tier, all dense and in index order, so
/// the whole thing is hashable and byte-comparable like any other column set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hints {
    /// Per node: `1` when the node's disc meets the viewport.
    pub visible: Vec<u8>,
    /// Per node: `1` when this node both is visible and is inside the label budget.
    pub labelled: Vec<u8>,
    /// Per edge: `1` when both endpoints are visible and the edge is not decimated away.
    pub edges: Vec<u8>,
    /// The tier the node count selected.
    pub tier: Tier,
}

/// How much of the graph the front is being asked to draw. Ordered least to most
/// reduced, so `tier <= Tier::NoLabels` is a comparison and not a match.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tier {
    /// Everything visible, every edge drawn, labels up to the budget.
    Full,
    /// Edges decimated; labels suppressed entirely.
    NoLabels,
    /// Edges decimated to a stride; only the highest-degree nodes keep a label.
    Decimated,
    /// Node count past the last threshold: draw nodes and labels, no edges.
    Clustered,
}

/// The viewport rectangle in world units, and the node radius the cull test uses.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    /// Left edge.
    pub x0: f64,
    /// Top edge.
    pub y0: f64,
    /// Right edge.
    pub x1: f64,
    /// Bottom edge.
    pub y1: f64,
    /// Every node's radius; a node is kept when its disc *straddles* the rectangle.
    pub radius: f64,
}

impl Viewport {
    /// A rectangle from its lower-left corner and its size. A negative size is taken as
    /// its magnitude, so a caller that swaps two corners gets the viewport it meant.
    pub fn from_size(x0: f64, y0: f64, width: f64, height: f64, radius: f64) -> Self {
        Self {
            x0,
            y0,
            x1: x0 + width.abs(),
            y1: y0 + height.abs(),
            radius: radius.abs(),
        }
    }
}

/// The policy: which thresholds, which budget. Every field is the caller's, so the
/// heuristics above are visible at the call site instead of frozen in the motor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LodParams {
    /// Where the front is looking.
    pub viewport: Viewport,
    /// Node count at or below which the graph is drawn whole.
    pub full_nodes: u32,
    /// Node count at or below which labels are suppressed.
    pub no_label_nodes: u32,
    /// Node count at or below which edges are decimated; past it, none are drawn.
    pub decimated_nodes: u32,
    /// How many visible nodes may carry a label.
    pub label_budget: u32,
    /// One edge in `edge_stride` survives decimation, counted by ascending edge index.
    pub edge_stride: u32,
}

impl Default for LodParams {
    /// Defaults from the phase's own size ladder: a graph today's editors open is
    /// drawn whole, and each step past a threshold is one order of magnitude of nodes.
    fn default() -> Self {
        Self {
            viewport: Viewport {
                x0: -1.0e6,
                y0: -1.0e6,
                x1: 1.0e6,
                y1: 1.0e6,
                radius: 16.0,
            },
            full_nodes: 2_000,
            no_label_nodes: 20_000,
            decimated_nodes: 200_000,
            label_budget: 64,
            edge_stride: 8,
        }
    }
}

impl LodParams {
    /// The tier `n` nodes select: the highest reduction the count has earned.
    pub fn tier(&self, n: u32) -> Tier {
        if n <= self.full_nodes {
            Tier::Full
        } else if n <= self.no_label_nodes {
            Tier::NoLabels
        } else if n <= self.decimated_nodes {
            Tier::Decimated
        } else {
            Tier::Clustered
        }
    }
}

/// The hints for a graph at `x, y` (one position per node, in dense index order).
///
/// Panics-free by construction: a non-finite position is treated as *not visible* rather
/// than propagated, because a NaN comparison is false and would otherwise silently read
/// as "inside the viewport" on one side of the test and "outside" on the other.
pub fn hints(t: &Topology, x: &[f64], y: &[f64], params: &LodParams) -> Hints {
    let n = t.node_count() as usize;
    let visible: Vec<u8> = (0..n)
        .map(|i| u8::from(meets(params.viewport, x.get(i).copied(), y.get(i).copied())))
        .collect();
    let tier = params.tier(t.node_count());
    Hints {
        labelled: label_mask(t, &visible, params, tier),
        edges: edge_mask(t, &visible, params, tier),
        visible,
        tier,
    }
}

/// Whether a node's disc at `(x, y)` straddles the rectangle. A missing or non-finite
/// position is not visible: the alternative is a mask that depends on which side of the
/// comparison the NaN fell.
fn meets(viewport: Viewport, x: Option<f64>, y: Option<f64>) -> bool {
    let (Some(x), Some(y)) = (x, y) else {
        return false;
    };
    if !x.is_finite() || !y.is_finite() {
        return false;
    }
    let r = viewport.radius;
    x + r >= viewport.x0 && x - r <= viewport.x1 && y + r >= viewport.y0 && y - r <= viewport.y1
}

/// The label mask: the visible nodes, the `label_budget` most important of them, ties
/// broken by ascending dense index.
///
/// "Important" is degree, descending — the reference's `apply_budget` filled its budget
/// greedily in descending `order_key` (`lod.py:82-92`) and guaranteed a non-empty mask;
/// both are kept, and the key is degree. The Ponytail marker above names what that key
/// costs when importance and degree disagree.
fn label_mask(t: &Topology, visible: &[u8], params: &LodParams, tier: Tier) -> Vec<u8> {
    let n = t.node_count() as usize;
    let mut mask = vec![0_u8; n];
    if tier == Tier::NoLabels {
        return mask;
    }
    let mut order: Vec<u32> = (0..n as u32)
        .filter(|&i| visible[i as usize] == 1)
        .collect();
    // Descending degree, then ascending index: a total order, so the mask does not depend
    // on the sort's stability or on the input order (D2).
    order.sort_by_key(|&i| (std::cmp::Reverse(degree_of(t, i)), i));
    let budget = usize::try_from(params.label_budget).unwrap_or(usize::MAX);
    for &i in order.iter().take(budget) {
        mask[i as usize] = 1;
    }
    // The reference's never-empty guarantee (`lod.py:88-90`): a budget of zero still lets
    // the most important visible node keep its label, so a front never has to draw a
    // viewport with nothing named in it.
    if mask.iter().all(|&m| m == 0)
        && let Some(&first) = order.first()
    {
        mask[first as usize] = 1;
    }
    mask
}

/// The edge mask: both endpoints visible, and — past the full tier — one edge in
/// `edge_stride` counted by ascending edge index.
fn edge_mask(t: &Topology, visible: &[u8], params: &LodParams, tier: Tier) -> Vec<u8> {
    let edges = t.edges();
    let stride = usize::try_from(params.edge_stride).unwrap_or(1).max(1);
    (0..t.edge_count() as usize)
        .map(|e| {
            let kept =
                visible[edges.source[e] as usize] == 1 && visible[edges.target[e] as usize] == 1;
            let decimated = match tier {
                Tier::Full => false,
                Tier::NoLabels | Tier::Decimated => e % stride != 0,
                Tier::Clustered => true,
            };
            u8::from(kept && !decimated)
        })
        .collect()
}

/// A node's undirected degree: both CSR rows' lengths, so a parallel edge and a
/// hierarchy edge each count once per direction they appear in. `u32::MAX` overflow is
/// not reachable: two CSR row lengths are bounded by the edge count.
fn degree_of(t: &Topology, node: u32) -> u32 {
    let out = t.out().row(node).len() as u32;
    let inbound = t.inbound().row(node).len() as u32;
    out.saturating_add(inbound)
}

#[cfg(test)]
mod tests;
