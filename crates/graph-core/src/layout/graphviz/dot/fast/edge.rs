//! Edge records moved out of `fast.rs`: the `Edge` struct with its `real`, `length` and
//! `slack` methods, plus the free `round` helper the position pass rounds with.

use super::node::Node;

/// One edge: real, virtual (a link of a chain), or auxiliary (a positioning constraint).
#[derive(Clone, Debug)]
pub struct Edge {
    /// `agtail`: the source node.
    pub tail: u32,
    /// `aghead`: the target node.
    pub head: u32,
    /// `ED_minlen`: the minimum rank (or x) separation, in points.
    pub minlen: i32,
    /// `ED_weight`: the constraint's weight in the network simplex.
    pub weight: i32,
    /// `ED_xpenalty`: the crossing penalty, 1 for every edge here.
    pub xpenalty: i32,
    /// `ED_count`: the number of real edges merged into this one.
    pub count: i32,
    /// `ED_cutvalue`: the simplex's cut value, valid only while the edge is a tree edge.
    pub cutvalue: i32,
    /// `ED_tree_index`: the edge's slot in the simplex's tree list, -1 off the tree.
    pub tree_index: i32,
    /// `ED_to_virt`: the first virtual edge of this real edge's chain.
    pub to_virt: Option<u32>,
    /// `ED_to_orig`: the real edge a virtual or reversed edge belongs to.
    pub to_orig: Option<u32>,
    /// False once the edge has been unhooked or absorbed; see the module doc.
    pub live: bool,
}

impl Edge {
    /// `dot_init_edge` at Graphviz's defaults: `minlen` 1 and `weight` 1
    /// (`dotinit.c:65,85`), `xpenalty` and `count` 1 (`dotinit.c:68`).
    pub fn real(tail: u32, head: u32) -> Self {
        Self {
            tail,
            head,
            minlen: 1,
            weight: 1,
            xpenalty: 1,
            count: 1,
            cutvalue: 0,
            tree_index: -1,
            to_virt: None,
            to_orig: None,
            live: true,
        }
    }

    /// `LENGTH(e)` (`ns.c:39`): the separation the edge currently has.
    pub fn length(&self, nodes: &[Node]) -> i32 {
        nodes[self.head as usize].rank - nodes[self.tail as usize].rank
    }

    /// `SLACK(e)` (`ns.c:40`): the room the edge has over its minimum.
    pub fn slack(&self, nodes: &[Node]) -> i32 {
        self.length(nodes) - self.minlen
    }
}

/// `ROUND` (`arith.h:48`): the reference's round-half-away-from-zero.
pub fn round(value: f64) -> i32 {
    if value >= 0.0 {
        (value + 0.5) as i32
    } else {
        (value - 0.5) as i32
    }
}
