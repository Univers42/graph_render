//! Layer assignment: longest-path layering with slack reduction, then dummy vertices for spans over one layer (`hierarchical.py:313-414`), exact but for the dummy budget itself.
//!
//! Ponytail: `DUMMY_BUDGET` (200,000, `hierarchical.py:7`) is a resource cap, not a correctness rule; over budget the longest arcs draw straight and note `dag.dummy_budget_exceeded`, never silently — see `docs/decisions/sugiyama-heuristics.md` for the full write-up.
use super::acyclic::Arcs;
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
    /// Builds the ordering graph over `arcs` at layers `layer`, admitting spans up to `budget` dummies in total, smallest span first.
    pub(crate) fn build(arcs: &Arcs, layer: &[u32], budget: u32) -> Self {
        let (max_span, dummy_count) = budget_plan(arcs, layer, budget);
        materialize(arcs, layer, max_span, dummy_count)
    }
}
/// Longest-path layers (`hierarchical.py:313-334`) then slack-reduced (`hierarchical.py:336-369`): one layer index per real node. Parallel edges are kept separate rather than deduped like the reference's `(u, v)` set (Ponytail: more dummies than it on a multigraph, cosmetic — see `docs/decisions/sugiyama-heuristics.md`).
pub(crate) fn assign_layers(arcs: &Arcs) -> Vec<u32> {
    let n = arcs.node_count() as usize;
    let (mut preds, mut succs) = (vec![Vec::new(); n], vec![Vec::new(); n]);
    for e in 0..arcs.edge_count() {
        if !arcs.is_loop(e) {
            let (tail, head) = arcs.tail_head(e);
            succs[tail as usize].push(head);
            preds[head as usize].push(tail);
        }
    }
    let mut layer = longest_path_layers(arcs.node_count(), &succs, &preds);
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
fn budget_plan(arcs: &Arcs, layer: &[u32], budget: u32) -> (Option<u32>, u32) {
    let spans: Vec<u32> = (0..arcs.edge_count())
        .filter(|&e| !arcs.is_loop(e))
        .map(|e| {
            let (tail, head) = arcs.tail_head(e);
            layer[head as usize] - layer[tail as usize]
        })
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
struct ChainBuilder<'a> {
    up: Vec<Vec<u32>>,
    down: Vec<Vec<u32>>,
    layer_of: Vec<u32>,
    arcs: &'a Arcs<'a>,
    max_span: Option<u32>,
}
impl<'a> ChainBuilder<'a> {
    fn new(arcs: &'a Arcs<'a>, layer: &[u32], max_span: Option<u32>, dummy_count: u32) -> Self {
        let total = (arcs.node_count() + dummy_count) as usize;
        let mut layer_of = vec![0u32; total];
        layer_of[..layer.len()].copy_from_slice(layer);
        Self {
            up: vec![Vec::new(); total],
            down: vec![Vec::new(); total],
            layer_of,
            arcs,
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
    /// Routes edge `e`, allocating dummies from `next_dummy` if it needs a chain.
    fn place(&mut self, e: u32, next_dummy: &mut u32) -> (Route, Option<Note>) {
        if self.arcs.is_loop(e) {
            return (Route::Loop, None);
        }
        let (tail, head) = self.arcs.tail_head(e);
        let span = self.layer_of[head as usize] - self.layer_of[tail as usize];
        if span == 1 {
            self.direct(tail, head);
            return (Route::Direct, None);
        }
        if self.max_span.is_some_and(|max| span > max) {
            let note = Note {
                code: NoteCode::DummyBudgetExceeded,
                index: e,
            };
            return (Route::Straight, Some(note));
        }
        let (first, count) = (*next_dummy, span - 1);
        *next_dummy += count;
        self.chain(tail, head, first, count);
        (Route::Chain { first, count }, None)
    }
}
fn materialize(arcs: &Arcs, layer: &[u32], max_span: Option<u32>, dummy_count: u32) -> Layering {
    let mut builder = ChainBuilder::new(arcs, layer, max_span, dummy_count);
    let mut next_dummy = arcs.node_count();
    let mut route = Vec::with_capacity(arcs.edge_count() as usize);
    let mut notes = Vec::new();
    for e in 0..arcs.edge_count() {
        let (r, note) = builder.place(e, &mut next_dummy);
        route.push(r);
        notes.extend(note);
    }
    Layering {
        layer_of: builder.layer_of,
        up: builder.up,
        down: builder.down,
        route,
        notes,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::index_model;
    use crate::layout::sugiyama::acyclic::Acyclic;
    use crate::records::build::{edge, node};
    /// `(layering, layer_of)` for `nodes`/`edges` at `budget`.
    fn built(nodes: &[&str], edges: &[(&str, &str, &str)], budget: u32) -> (Layering, Vec<u32>) {
        let n: Vec<_> = nodes.iter().map(|id| node(id, "")).collect();
        let e: Vec<_> = edges.iter().map(|&(id, s, t)| edge(id, s, t)).collect();
        let t = index_model(&n, &e).expect("fits");
        let acyclic = Acyclic::of(&t);
        let arcs = Arcs::new(&t, &acyclic);
        let layer = assign_layers(&arcs);
        (Layering::build(&arcs, &layer, budget), layer)
    }
    #[test]
    fn a_chain_has_no_dummies_and_a_reversed_cycle_still_spans_forward() {
        let e = [("ab", "a", "b"), ("bc", "b", "c"), ("cd", "c", "d")];
        let (l, layer) = built(&["a", "b", "c", "d"], &e, DUMMY_BUDGET);
        assert_eq!(layer, [0, 1, 2, 3]);
        assert_eq!(l.route, [Route::Direct, Route::Direct, Route::Direct]);
        assert!(l.notes.is_empty());
        // c->a closes a cycle, reversing to a->c: still spans forward, so nothing goes Straight.
        let e2 = [("ab", "a", "b"), ("bc", "b", "c"), ("ca", "c", "a")];
        let (l2, layer2) = built(&["a", "b", "c"], &e2, DUMMY_BUDGET);
        assert!(l2.route.iter().all(|r| !matches!(r, Route::Straight)));
        assert!(layer2.iter().all(|&x| x <= 2));
    }
    #[test]
    fn a_multi_span_edge_gets_one_dummy_per_intermediate_layer() {
        let e = [
            ("ab", "a", "b"),
            ("bc", "b", "c"),
            ("cd", "c", "d"),
            ("ad", "a", "d"),
        ];
        let (l, layer) = built(&["a", "b", "c", "d"], &e, DUMMY_BUDGET);
        assert_eq!(layer, [0, 1, 2, 3]);
        assert_eq!(l.route[3], Route::Chain { first: 4, count: 2 });
        assert_eq!(l.up.len(), 6, "4 real + 2 dummy");
        assert_eq!(l.down[0], [1, 4], "a's direct edge, then its dummy chain");
        assert_eq!((l.up[4][0], l.down[4][0]), (0, 5));
        assert_eq!((l.up[5][0], l.down[5][0]), (4, 3));
    }
    #[test]
    fn a_lowered_budget_leaves_the_longest_spans_straight_and_noted() {
        // Two 2-layer edges (1 dummy each) plus a 4-layer edge (3 dummies): budget 2 admits
        // only the two smallest spans, leaving the longest one straight.
        let e = [
            ("ab", "a", "b"),
            ("bc", "b", "c"),
            ("cd", "c", "d"),
            ("de", "d", "e"),
            ("ac", "a", "c"),
            ("bd", "b", "d"),
            ("ae", "a", "e"),
        ];
        let (l, _) = built(&["a", "b", "c", "d", "e"], &e, 2);
        assert_eq!(l.route[4], Route::Chain { first: 5, count: 1 });
        assert_eq!(l.route[5], Route::Chain { first: 6, count: 1 });
        assert_eq!(l.route[6], Route::Straight, "the 4-layer span");
        assert_eq!(
            l.notes,
            [Note {
                code: NoteCode::DummyBudgetExceeded,
                index: 6
            }]
        );
    }
}
