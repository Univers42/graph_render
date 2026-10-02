//! The lowlink walk itself: one explicit frame stack per component, finding the biconnected
//! components and hanging them on the block-cutpoint tree.
//!
//! [`Walk`] is the reference's recursive `dfs` as an explicit stack — see `blocks`' module doc
//! for the three jobs it does at once. [`walk_component`] is the per-component driver that
//! builds one and appends its blocks to the forest.

use super::super::Block;
use super::super::graph::Derived;
use super::Found;
use super::node_count;

/// The component `component`, as a block tree, appended to `found`.
pub(super) fn walk_component(derived: &Derived, component: Vec<u32>, found: &mut Found) -> usize {
    let nodes = component.len();
    let mut slot = vec![u32::MAX; node_count(&component)];
    for (at, &node) in component.iter().enumerate() {
        slot[node as usize] = at as u32;
    }
    let mut walk = Walk {
        derived,
        slot,
        val: vec![0; nodes],
        low: vec![0; nodes],
        parent: vec![u32::MAX; nodes],
        block_of: vec![u32::MAX; nodes],
        stack: Vec::new(),
        order: 0,
        base: found.blocks.len(),
        blocks: Vec::new(),
        list: Vec::new(),
        parent_flag: vec![false; nodes],
    };
    walk.step(component[0]);
    for block in &mut walk.blocks {
        // `agfstnode` walks `g->n_seq`, an ordered dictionary keyed on **AGSEQ** — the node's
        // creation number (`node.c:46`, `agsubnodeseqcmpf`, `node.c:290-299`). Every node of
        // the derived graph is created once, in `circomps`' first loop and in `agfstnode`
        // order, so `AGSEQ` is the dense node index and **every** `agfstnode` walk over any
        // subgraph of the derived graph — a block's included — comes out ascending. That is
        // not the order `addNode` inserted the nodes in, and the difference decides which
        // node a block's circle starts from.
        block.nodes.sort_unstable();
    }
    let local = walk.list.first().copied().unwrap_or(0);
    walk.assemble(local);
    let root = walk.base + local;
    for (at, flag) in walk.parent_flag.iter().enumerate() {
        if *flag {
            found.parent_flag[component[at] as usize] = true;
        }
    }
    found.blocks.append(&mut walk.blocks);
    root
}

/// One depth-first frame: the node, its neighbour row, and how far into it we are.
struct Frame {
    /// The node's **global** index, which is what `Derived` indexes by.
    node: u32,
    /// Its neighbours, global too — they are translated once, on the way into the arrays.
    neighbours: Vec<u32>,
    at: usize,
}

/// The walk itself, kept as its own type so [`Walk::step`] can stay inside the line cap.
struct Walk<'a> {
    derived: &'a Derived,
    /// Global node -> this component's local index, `u32::MAX` outside it. Every array below
    /// is component-sized and indexed locally, so a component that does not start at node 0
    /// works exactly as one that does.
    slot: Vec<u32>,
    /// `VAL(n)`: discovery order, `0` for unvisited.
    val: Vec<u32>,
    /// `LOWVAL(n)`: the lowest `VAL` reachable from `n` by tree edges then one back edge.
    low: Vec<u32>,
    /// `PARENT(n)`: the **node** that discovered `n`, `u32::MAX` for the root. Node ids,
    /// not local indices: every array around it is component-sized, and this one is read
    /// back through [`Walk::local`] like any other node.
    parent: Vec<u32>,
    /// `BLOCK(n)`: the block holding `n`, `u32::MAX` while it has none.
    block_of: Vec<u32>,
    /// `estack`: the discovered nodes whose edges have not been popped into a block yet.
    stack: Vec<u32>,
    /// `orderCount`, the next `VAL`.
    order: u32,
    /// Where this component's blocks start in the forest-wide list, so a component-local
    /// block index can be turned into a global one on the way out.
    base: usize,
    blocks: Vec<Block>,
    /// `state->bl`: the blocks in creation order, with the root's pushed to the front.
    list: Vec<usize>,
    /// `PARENT_F`, set by [`Walk::assemble`].
    parent_flag: Vec<bool>,
}

impl Walk<'_> {
    /// This component's local index of a node that is in it.
    fn local(&self, node: u32) -> usize {
        let at = self.slot[node as usize];
        assert!(at != u32::MAX, "node {node} is not in this component");
        at as usize
    }

    /// The depth-first walk from `start`.
    fn step(&mut self, start: u32) {
        self.enter(start);
        let mut frames = vec![Frame {
            node: start,
            neighbours: self.derived.neighbours(start),
            at: 0,
        }];
        while !frames.is_empty() {
            let last = frames.len() - 1;
            let node = frames[last].node;
            match frames[last].next() {
                None => {
                    frames.pop();
                    let parent_is_root = frames.len() == 1;
                    if let Some(parent) = frames.last() {
                        let parent = parent.node;
                        self.link(parent, node, parent_is_root);
                    }
                }
                Some(other) if self.val[self.local(other)] == 0 => {
                    let there = self.local(other);
                    self.parent[there] = node;
                    self.stack.push(other);
                    self.enter(other);
                    frames.push(Frame {
                        node: other,
                        neighbours: self.derived.neighbours(other),
                        at: 0,
                    });
                }
                Some(other) => {
                    // `blocktree.c:102-104`: a back edge only lowers `LOWVAL` when it does not
                    // point at the node's own parent — that edge was the tree edge, and its
                    // subtree is already accounted for.
                    let (at, there) = (self.local(node), self.local(other));
                    if self.parent[at] != other {
                        let low = self.low[at].min(self.val[there]);
                        self.low[at] = low;
                    }
                }
            }
        }
        // `blocktree.c:106-110`: the walk's root is an artificial cut point, so it joins no
        // block of its own unless it landed in one — and if it did not, it gets a one-node
        // block, pushed to the **front** of the list so it becomes the tree's root.
        let me = self.local(start);
        if self.block_of[me] == u32::MAX {
            let at = self.new_block();
            self.blocks[at].nodes.push(start);
            self.block_of[me] = at as u32;
            self.list.insert(0, at);
        }
    }

    /// `LOWVAL(u) = min(LOWVAL(u), LOWVAL(v))`, then the articulation test at `u` for `v`.
    fn link(&mut self, parent: u32, child: u32, parent_is_root: bool) {
        let (at, there) = (self.local(parent), self.local(child));
        let child_low = self.low[there];
        let low = self.low[at].min(child_low);
        self.low[at] = low;
        if child_low >= self.val[at] {
            self.close_block(parent, child, parent_is_root);
        }
    }

    /// `LOWVAL(u) = VAL(u) = orderCount++`.
    fn enter(&mut self, node: u32) {
        let at = self.local(node);
        self.order += 1;
        self.val[at] = self.order;
        self.low[at] = self.order;
    }

    /// Pops the edges down to and including the one `child` was discovered on, putting every
    /// head it passes into one block — the reference's `do { … } while (ep != e)`.
    fn close_block(&mut self, parent: u32, child: u32, parent_is_root: bool) {
        let mut block: Option<usize> = None;
        loop {
            let head = self
                .stack
                .pop()
                .expect("the stack holds the edge we came in on");
            let me = self.local(head);
            if self.block_of[me] == u32::MAX {
                let at = *block.get_or_insert_with(|| self.new_block());
                self.blocks[at].nodes.push(head);
                self.block_of[me] = at as u32;
            }
            if head == child {
                break;
            }
        }
        let Some(at) = block else { return };
        // `blocktree.c:94-95`: the cut point joins its child's block only when that block is
        // bigger than one node. A one-node block is the reference's own degenerate case.
        let me = self.local(parent);
        if self.block_of[me] == u32::MAX && self.blocks[at].nodes.len() > 1 {
            self.blocks[at].nodes.push(parent);
            self.block_of[me] = at as u32;
        }
        let holds_parent = self.block_of[me] == at as u32;
        if parent_is_root && holds_parent {
            self.list.insert(0, at);
        } else {
            self.list.push(at);
        }
    }

    /// A block with no nodes yet, appended to the forest.
    fn new_block(&mut self) -> usize {
        self.blocks.push(Block::empty());
        self.blocks.len() - 1
    }

    /// `createBlocktree` (`blocktree.c:143-179`): hang every block but the first on the block
    /// holding its earliest-discovered node, and mark that node as the block's anchor.
    fn assemble(&mut self, root: usize) {
        let list = self.list.clone();
        for at in list.into_iter().skip(1) {
            let Some((anchor, owner)) = self.anchor(at) else {
                continue;
            };
            // `SET_PARENT(parent)` marks the node **in the parent block** the child hangs off,
            // which is what `BLK_PARENT` reads back (`block.h:51`) and what `circpos.c` later
            // matches a child against by node id.
            let flag = self.parent[self.local(anchor)];
            let local = self.local(flag);
            self.blocks[at].child_node = anchor;
            self.blocks[at].hangs_at = flag;
            self.blocks[at].parent = Some(owner);
            self.parent_flag[local] = true;
            self.blocks[owner].children.push(at);
        }
        self.blocks[root].parent = None;
    }

    /// The block's minimum-`VAL` node, and the block that holds that node's parent: the
    /// reference scans `agfstnode` of the block and keeps the first strict minimum
    /// (`blocktree.c:163-171`), so a tie is broken by the block's own node order.
    fn anchor(&self, at: usize) -> Option<(u32, usize)> {
        let (&first, rest) = self.blocks[at].nodes.split_first()?;
        let mut anchor = first;
        let mut lowest = self.val[self.local(first)];
        for &node in rest {
            if self.val[self.local(node)] < lowest {
                anchor = node;
                lowest = self.val[self.local(node)];
            }
        }
        // `PARENT(anchor)` is a node of this component, so it goes back through `slot`;
        // the walk's own root has none (`u32::MAX`), and a block anchored there has no block
        // above it to hang on — the reference never reaches that case either, since the root
        // block is the one `list` puts first.
        let parent = self.parent[self.local(anchor)];
        if parent == u32::MAX {
            return None;
        }
        let owner = self.block_of[self.local(parent)];
        if owner == u32::MAX {
            return None;
        }
        Some((anchor, owner as usize))
    }
}

impl Frame {
    /// The next neighbour of this frame's node, or `None` when its row is spent.
    fn next(&mut self) -> Option<u32> {
        let at = self.at;
        self.at += 1;
        self.neighbours.get(at).copied()
    }
}
