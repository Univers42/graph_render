//! Layer assignment: longest-path layering with slack reduction, then dummy vertices for
//! spans over one layer (`hierarchical.py:313-414`), exact but for the dummy budget itself.
//!
//! **Over the distinct arc list, not over edges.** The reference's `arcs` is a `set` of
//! `(u, v)` pairs (`hierarchical.py:306`), so `k` parallel edges between two nodes are one
//! arc: one neighbour on each side here, one dummy chain, one entry in `up`/`down`. `Route`
//! stays per edge, so every parallel edge is still drawn — through the one chain they share.
//!
//! Ponytail: `DUMMY_BUDGET` (200,000, `hierarchical.py:7`) is a resource cap, not a
//! correctness rule; over budget the longest arcs draw straight and note
//! `dag.dummy_budget_exceeded`, never silently — see `docs/decisions/sugiyama-heuristics.md`
//! for the full write-up.
use super::acyclic::ArcList;
use graph_contract::notes::{Note, NoteCode};
/// The reference's budget (`hierarchical.py:7`); tests pass a smaller one directly, so an overflow does not need a 200k-edge fixture.
pub(crate) const DUMMY_BUDGET: u32 = 200_000;
/// How edge `e` is drawn, decided once here and read by `coords`/`routing`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Route {
    /// A self-loop: excluded from layering, drawn with no interior points.
    Loop,
    /// One layer step: no interior points.
    Direct,
    /// `count` dummy vertices, dense ids `first..first + count`, tail to head.
    Chain { first: u32, count: u32 },
    /// Span exceeded the dummy budget: drawn straight, `dag.dummy_budget_exceeded` noted.
    Straight,
}
/// The ordering graph: real nodes `0..n` then dummies `n..`, every arc (direct or through its dummy chain) as `up`/`down` adjacency.
pub(crate) struct Layering {
    pub(crate) layer_of: Vec<u32>,
    pub(crate) up: Vec<Vec<u32>>,
    pub(crate) down: Vec<Vec<u32>>,
    pub(crate) route: Vec<Route>,
    pub(crate) notes: Vec<Note>,
}
impl Layering {
    /// Builds the ordering graph over `list` at layers `layer`, admitting spans up to
    /// `budget` dummies in total, smallest span first.
    pub(crate) fn build(list: &ArcList, layer: &[u32], budget: u32) -> Self {
        let plan = budget_plan(list, layer, budget);
        materialize(list, layer, plan)
    }
}

/// What [`budget_plan`] decided: the admitted span ceiling and the dummies it admits.
type Plan = (Option<u32>, u32);
/// Longest-path layers (`hierarchical.py:313-334`) then slack-reduced (`hierarchical.py:336-369`): one layer index per real node. Over the **distinct** arc list, as the reference is: its `arcs` is a `set` of `(u, v)` pairs (`hierarchical.py:306`), so `k` parallel edges between two nodes are one neighbour on each side here too, and a parallel edge does not shift the median [`reduce_slack`] slides toward.
pub(crate) fn assign_layers(list: &ArcList) -> Vec<u32> {
    let n = list.nodes as usize;
    let (mut preds, mut succs) = (vec![Vec::new(); n], vec![Vec::new(); n]);
    for &(tail, head, _) in &list.arcs {
        succs[tail as usize].push(head);
        preds[head as usize].push(tail);
    }
    let mut layer = longest_path_layers(list.nodes, &succs, &preds);
    reduce_slack(&preds, &succs, &mut layer);
    layer
}
/// Rank by longest path from a source, over an index-head Kahn walk (never `Vec::remove(0)`).
fn longest_path_layers(n: u32, succs: &[Vec<u32>], preds: &[Vec<u32>]) -> Vec<u32> {
    let mut in_degree: Vec<u32> = preds.iter().map(|p| p.len() as u32).collect();
    let mut layer = vec![0u32; n as usize];
    let mut queue: Vec<u32> = (0..n).filter(|&v| in_degree[v as usize] == 0).collect();
    let mut head = 0usize;
    while let Some(&u) = queue.get(head) {
        head += 1;
        let below = layer[u as usize] + 1;
        for &v in &succs[u as usize] {
            layer[v as usize] = layer[v as usize].max(below);
            in_degree[v as usize] -= 1;
            if in_degree[v as usize] == 0 {
                queue.push(v);
            }
        }
    }
    layer
}
/// Slides each node toward the median of its neighbours' layers within its free window, up to four passes (`hierarchical.py:336-369`), then compresses unused layer indices onto `0..k` (sorted + deduped + binary search, no `HashMap`, D4).
fn reduce_slack(preds: &[Vec<u32>], succs: &[Vec<u32>], layer: &mut [u32]) {
    for _ in 0..4 {
        let moved: u32 = (0..layer.len() as u32)
            .map(|node| u32::from(slide(node, preds, succs, layer)))
            .sum();
        if moved == 0 {
            break;
        }
    }
    let mut used = layer.to_vec();
    used.sort_unstable();
    used.dedup();
    for slot in layer.iter_mut() {
        *slot = used.binary_search(slot).expect("present") as u32;
    }
}
/// One node's slide, `true` when it moved; skips a node with no successor to pull.
fn slide(node: u32, preds: &[Vec<u32>], succs: &[Vec<u32>], layer: &mut [u32]) -> bool {
    let below = &succs[node as usize];
    if below.is_empty() {
        return false;
    }
    let above = &preds[node as usize];
    let low = above
        .iter()
        .map(|&u| layer[u as usize] + 1)
        .max()
        .unwrap_or(0);
    let high = below
        .iter()
        .map(|&v| layer[v as usize])
        .min()
        .unwrap_or(0)
        .saturating_sub(1);
    if high <= low {
        return false;
    }
    let mut around: Vec<u32> = above
        .iter()
        .chain(below)
        .map(|&x| layer[x as usize])
        .collect();
    around.sort_unstable();
    let target = around[around.len() / 2].clamp(low, high);
    if target == layer[node as usize] {
        return false;
    }
    layer[node as usize] = target;
    true
}
/// The admitted span ceiling and dummies needed (`hierarchical.py:381-389`): `(None, needed)` when every arc fits `budget`; otherwise the largest span still admitted (smallest spans first) paired with the dummy count it actually admits.
fn budget_plan(list: &ArcList, layer: &[u32], budget: u32) -> Plan {
    let spans: Vec<u32> = list
        .arcs
        .iter()
        .map(|&(tail, head, _)| layer[head as usize] - layer[tail as usize])
        .collect();
    let needed: u64 = spans.iter().map(|&s| u64::from(s - 1)).sum();
    if needed <= u64::from(budget) {
        return (None, needed as u32);
    }
    let mut sorted = spans.clone();
    sorted.sort_unstable();
    let mut used = 0u64;
    for span in sorted {
        used += u64::from(span - 1);
        if used > u64::from(budget) {
            let max_span = (span - 1).max(1);
            let count = spans
                .iter()
                .filter(|&&s| s <= max_span)
                .map(|&s| s - 1)
                .sum();
            return (Some(max_span), count);
        }
    }
    unreachable!("needed > budget implies the loop returns")
}
/// The growing ordering graph and what [`Self::place`] needs to route each edge.
struct ChainBuilder {
    up: Vec<Vec<u32>>,
    down: Vec<Vec<u32>>,
    layer_of: Vec<u32>,
    max_span: Option<u32>,
}
impl ChainBuilder {
    fn new(node_count: u32, layer: &[u32], max_span: Option<u32>, dummy_count: u32) -> Self {
        let total = (node_count + dummy_count) as usize;
        let mut layer_of = vec![0u32; total];
        layer_of[..layer.len()].copy_from_slice(layer);
        Self {
            up: vec![Vec::new(); total],
            down: vec![Vec::new(); total],
            layer_of,
            max_span,
        }
    }
    fn direct(&mut self, tail: u32, head: u32) {
        self.down[tail as usize].push(head);
        self.up[head as usize].push(tail);
    }
    /// Links `tail` to `head` through `count` fresh dummies starting at `first`.
    fn chain(&mut self, tail: u32, head: u32, first: u32, count: u32) {
        let base = self.layer_of[tail as usize];
        let mut previous = tail;
        for step in 0..count {
            let dummy = first + step;
            self.layer_of[dummy as usize] = base + step + 1;
            self.direct(previous, dummy);
            previous = dummy;
        }
        self.direct(previous, head);
    }
    /// Routes one distinct arc `tail -> head`, allocating dummies from `next_dummy` if it
    /// needs a chain.
    fn place(&mut self, arc: (u32, u32, u32), next_dummy: &mut u32) -> (Route, Option<Note>) {
        let (tail, head, edge) = arc;
        let span = self.layer_of[head as usize] - self.layer_of[tail as usize];
        if span == 1 {
            self.direct(tail, head);
            return (Route::Direct, None);
        }
        if self.max_span.is_some_and(|max| span > max) {
            let note = Note {
                code: NoteCode::DummyBudgetExceeded,
                index: edge,
            };
            return (Route::Straight, Some(note));
        }
        let (first, count) = (*next_dummy, span - 1);
        *next_dummy += count;
        self.chain(tail, head, first, count);
        (Route::Chain { first, count }, None)
    }
}
/// The ordering graph over the distinct arcs, and a [`Route`] per **edge**: every edge in an
/// arc's range takes that arc's route, so `k` parallel edges share the one dummy chain the
/// reference's arc set gives them, and a self-loop is `Route::Loop`.
fn materialize(list: &ArcList, layer: &[u32], (max_span, dummy_count): Plan) -> Layering {
    let mut builder = ChainBuilder::new(list.nodes, layer, max_span, dummy_count);
    let mut next_dummy = list.nodes;
    let mut routes: Vec<Option<Route>> = vec![None; list.edges as usize];
    let mut notes = Vec::new();
    for &(tail, head, ref edges) in &list.arcs {
        let route = builder.place((tail, head, edges.start), &mut next_dummy);
        notes.extend(route.1);
        for e in edges.clone() {
            routes[e as usize] = Some(route.0);
        }
    }
    // An edge with no entry is a self-loop: it is in no arc, so it was never routed.
    let route = routes
        .into_iter()
        .map(|r| r.unwrap_or(Route::Loop))
        .collect();
    Layering {
        layer_of: builder.layer_of,
        up: builder.up,
        down: builder.down,
        route,
        notes,
    }
}

#[cfg(test)]
mod tests;
