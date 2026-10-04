//! The per-rank node rows the mincross pass reads and writes.
//!
//! Graphviz keeps one array per rank on the graph (`GD_rank`), and three passes walk it:
//! the initial ordering installs nodes into it, the transpose and reorder passes swap
//! neighbours inside one rank's slot, and the crossing counter caches one count per band of
//! ranks. This module is that array, as a value the driver threads through the passes rather
//! than as fields on [`Fast`] — the reference hangs it on the graph, but here it is pass
//! state that stops existing when the pass returns.
//!
//! **The row is bigger than the window.** Every rank's slots are allocated once, for the
//! whole graph, and each connected component is then given a *window* into the same array at
//! the position after the previous component's window: component 0 fills the row from the
//! left, component 1 from where component 0 stopped, and so on. That is why the orders a
//! component sees are window-relative and why `install_complete_ranks` has to renumber
//! everything at the end. A port with one array per component would need a second merge pass
//! to produce the same numbers.
//!
//! **One slot per rank is left empty.** The allocation counts the nodes and the crossing
//! slots a rank needs and adds one more, so the last slot of every row is never written. It
//! is the reference's NULL terminator, and the end of the row after the last component is
//! found by walking to it. It is [`NONE`] here.
//!
//! Determinism: every index is a dense node or rank index, the rows are filled in the order
//! the passes install into them, and nothing here reads a clock, a hash order or a random
//! number (`prompt.md` §6 D1-D10).

use super::fast::Fast;

/// The slot a row never fills: the reference's NULL, and the end-of-row marker.
pub const NONE: u32 = u32::MAX;

/// One rank's row: the whole array, the component's window into it, and the cached count of
/// crossings with the band below.
pub struct Row {
    /// The row itself, `NONE` in every slot no component has reached.
    pub av: Vec<u32>,
    /// Where the current component's window starts in [`Row::av`].
    pub v0: u32,
    /// How many of the window's slots hold a node.
    pub n: u32,
    /// The window's capacity: the rank's own count plus the one spare slot.
    pub an: u32,
    /// Crossings between this rank and the one below it, as last counted.
    pub cache_nc: i64,
    /// Whether [`Row::cache_nc`] still describes the order now in the window. Every swap
    /// inside a rank clears it, and so does a fresh ordering of that rank.
    pub valid: bool,
    /// Whether the transpose pass may visit this rank in the round it is running.
    pub candidate: bool,
}

/// Every rank's row, plus the two pieces of scratch the pass needs: the node list of the
/// component being ordered, and one reusable buffer for a node's neighbour values.
pub struct Ranks {
    /// Indexed by rank. One row past the last real rank, because the transpose pass asks
    /// whether the rank *below* is empty and the reference's allocation has that slot.
    pub rows: Vec<Row>,
    /// The highest rank in use, `GD_maxrank`. The lowest is always 0 here: the rank pass
    /// normalises it, and the reference only moves the lowest rank for a cluster.
    max: usize,
    /// The node list of the component being ordered, in the order the components pass
    /// produced it.
    nlist: Vec<u32>,
    /// The buffer one node's median values are gathered into, reused for every node.
    scratch: Vec<i32>,
}

/// The highest rank any node sits on, as a row index. An empty graph has one (empty) rank.
pub fn max_rank(g: &Fast) -> usize {
    g.nodes
        .iter()
        .map(|n| n.rank.max(0) as usize)
        .max()
        .unwrap_or(0)
}

/// Every node's rank as a row index, or 0 for a node the rank pass never placed.
pub fn row_of(g: &Fast, node: u32) -> usize {
    g.nodes[node as usize].rank.max(0) as usize
}

impl Ranks {
    /// Size every row for the whole graph: one slot per node on the rank, one per edge that
    /// spans it (which is where the chain dummies go), and one spare.
    ///
    /// The count is over the *input* edges, which is what the reference walks — a merged
    /// multi-edge spans the ranks its twin does without needing slots of its own, so the
    /// count is an upper bound. An over-long row costs nothing: the spare slots stay
    /// [`NONE`] and the row is cut back to what was filled.
    pub fn allocate(g: &Fast) -> Self {
        let max = max_rank(g);
        let mut counts = vec![0u32; max + 2];
        for (node, record) in g.nodes.iter().enumerate() {
            counts[row_of(g, node as u32)] += 1;
            for &edge in &g.orig_out[node] {
                span_counts(g, &g.edges[edge as usize], &mut counts);
            }
        }
        let rows = (0..=max + 1)
            .map(|r| Row {
                av: vec![NONE; counts[r] as usize + 1],
                v0: 0,
                n: counts[r] + 1,
                an: counts[r] + 1,
                cache_nc: 0,
                valid: false,
                candidate: false,
            })
            .collect();
        Self { rows, max, nlist: Vec::new(), scratch: Vec::new() }
    }

    /// The highest rank in use.
    pub fn max(&self) -> usize {
        self.max
    }

    /// The component currently being ordered, in the order the components pass gave it.
    pub fn nlist(&self) -> &[u32] {
        &self.nlist
    }

    /// The median-value buffer, emptied by the caller before it is filled.
    pub fn scratch(&mut self) -> &mut Vec<i32> {
        &mut self.scratch
    }

    /// Take a component: its node list, and — every component after the first — a window
    /// past everything the previous one filled. The window's spare slot is not carried
    /// over, so a component can never overwrite the [`NONE`] that ends the row.
    pub fn enter_component(&mut self, index: usize, nlist: &[u32]) {
        self.nlist = nlist.to_vec();
        if index == 0 {
            return;
        }
        for row in &mut self.rows {
            row.v0 += row.n;
            row.n = 0;
        }
    }

    /// How many nodes the window of rank `r` holds. The row past the last real rank answers
    /// 1, as the reference's spare slot does: the transpose pass asks this to find out
    /// whether the band below a rank is empty, and the top rank's band is always empty.
    pub fn len(&self, r: usize) -> usize {
        self.rows.get(r).map_or(0, |row| row.n as usize)
    }

    /// The `i`th node of rank `r`'s window.
    pub fn get(&self, r: usize, i: usize) -> u32 {
        self.rows[r].av[(self.rows[r].v0 + i as u32) as usize]
    }

    /// Put `node` in the `i`th slot of rank `r`'s window.
    pub fn set(&mut self, r: usize, i: usize, node: u32) {
        let v0 = self.rows[r].v0 as usize;
        self.rows[r].av[v0 + i] = node;
    }

    /// Rank `r`'s window, as it stands.
    pub fn window(&self, r: usize) -> &[u32] {
        let row = &self.rows[r];
        let v0 = row.v0 as usize;
        &row.av[v0..v0 + row.n as usize]
    }

    /// Rank `r`'s window, mutable, for the one pass that reorders the array itself.
    pub fn window_mut(&mut self, r: usize) -> &mut [u32] {
        let v0 = self.rows[r].v0 as usize;
        let n = self.rows[r].n as usize;
        &mut self.rows[r].av[v0..v0 + n]
    }

    /// Put `node` at the right-hand end of its own rank's window and number it there. The
    /// slot is the one the window's spare occupies if the row is full, so this answers
    /// whether there was room.
    pub fn append(&mut self, g: &mut Fast, node: u32) -> bool {
        let r = row_of(g, node);
        let i = self.rows[r].n;
        if i >= self.rows[r].an {
            return false;
        }
        self.set(r, i as usize, node);
        g.nodes[node as usize].order = i32::try_from(i).unwrap_or(i32::MAX);
        self.rows[r].n = i + 1;
        true
    }

    /// Swap two nodes of one rank: each takes the other's slot, and each is renumbered to
    /// the slot it now holds, so `order` and the window never disagree.
    pub fn swap(&mut self, g: &mut Fast, v: u32, w: u32) {
        let r = row_of(g, v);
        let (vi, wi) = (g.nodes[v as usize].order, g.nodes[w as usize].order);
        let (vi, wi) = (vi as usize, wi as usize);
        g.nodes[v as usize].order = i32::try_from(wi).unwrap_or(i32::MAX);
        self.set(r, wi, v);
        g.nodes[w as usize].order = i32::try_from(vi).unwrap_or(i32::MAX);
        self.set(r, vi, w);
    }

    /// Remember the current order as the best one seen so far.
    pub fn save_best(&self, g: &mut Fast) {
        for r in 0..=self.max {
            for &node in self.window(r) {
                g.nodes[node as usize].saveorder = g.nodes[node as usize].order;
            }
        }
    }

    /// Go back to the remembered order: each window is renumbered from what was saved and
    /// then sorted back into that order, and every crossing count is dropped, because a
    /// restored order has never been counted.
    pub fn restore_best(&mut self, g: &mut Fast) {
        for r in 0..=self.max {
            for i in 0..self.len(r) {
                let node = self.get(r, i);
                g.nodes[node as usize].order = g.nodes[node as usize].saveorder;
            }
        }
        for r in 0..=self.max {
            self.rows[r].valid = false;
            self.window_mut(r).sort_by_key(|&node| g.nodes[node as usize].order);
        }
    }

    /// Make every rank a whole rank again: the window becomes the whole row, the row is cut
    /// back to the slots that were filled, and the orders become positions in the whole
    /// rank. This is the last step of the pass, and the orders the next pass reads are these.
    pub fn install_complete_ranks(&mut self, g: &mut Fast) {
        for r in 0..=self.max {
            self.rows[r].v0 = 0;
            let mut filled = 0;
            while filled < self.rows[r].av.len() {
                let node = self.rows[r].av[filled];
                if node == NONE {
                    break;
                }
                g.nodes[node as usize].order = i32::try_from(filled).unwrap_or(i32::MAX);
                filled += 1;
            }
            self.rows[r].n = filled as u32;
        }
    }
}

/// The one slot per intervening rank an input edge claims: a chain's dummy has to land
/// somewhere, and the rank it lands on is the one the edge jumps over.
fn span_counts(g: &Fast, edge: &super::fast::Edge, counts: &mut [u32]) {
    let (mut low, mut high) = (row_of(g, edge.tail), row_of(g, edge.head));
    if low > high {
        std::mem::swap(&mut low, &mut high);
    }
    for r in (low + 1)..high {
        counts[r] += 1;
    }
}