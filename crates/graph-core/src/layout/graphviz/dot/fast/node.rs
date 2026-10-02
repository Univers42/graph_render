//! Node records moved out of `fast.rs`: `Kind` (`ND_node_type`), `Coord` (`ND_coord`'s
//! `pointf`) and the `Node` struct with its `normal` / `virtual_node` constructors.

/// `ND_node_type`: a real node, or a virtual (dummy) one standing in for an edge that
/// spans more than one rank.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// `NORMAL`.
    Normal,
    /// `VIRTUAL`, created by `class2`'s `make_chain` or by `acyclic`'s `reverse_edge`.
    Virtual,
}

/// A point in points: `ND_coord`'s `pointf`.
#[derive(Clone, Copy, Default, Debug)]
pub struct Coord {
    /// The x coordinate, in points.
    pub x: f64,
    /// The y coordinate, in points.
    pub y: f64,
}

/// One node, real or virtual.
#[derive(Clone, Debug)]
pub struct Node {
    /// `ND_rank`: the layer, 0-based; the x coordinate once `set_xcoords` has run.
    pub rank: i32,
    /// `ND_order`: the index within the rank's vlist.
    pub order: i32,
    /// The `ND_order` as of the last `save_best`, restored by `restore_best`.
    pub saveorder: i32,
    /// `ND_coord`: the emitted centre, in points.
    pub coord: Coord,
    /// `ND_lw`: half the node's width to the left of its centre, in points.
    pub lw: f64,
    /// `ND_rw`: half the width to the right, in points.
    pub rw: f64,
    /// `ND_ht`: the full height, in points.
    pub ht: f64,
    /// `ND_mval`: the median `mincross` sorts by, -1 where there is none.
    pub mval: f64,
    /// `ND_node_type`.
    pub kind: Kind,
    /// `ND_weight_class`: how many edges touch this node, capped at 2 by `class2`.
    pub weight_class: i32,
    /// `ND_mark`, the DFS / component marker. Its meaning is per pass.
    pub mark: bool,
    /// `ND_onstack`, the cycle-breaking marker.
    pub onstack: bool,
    /// `ND_flat_out`: edges from this node to one on the *same* rank.
    pub flat_out: Vec<u32>,
    /// `ND_flat_in`: the same, reversed.
    pub flat_in: Vec<u32>,
    /// `ND_other`: self-loops, which take part in no pass but the self-edge width.
    pub other: Vec<u32>,
}

impl Node {
    /// A real node with the given box. `lw`/`rw` are the halves of Graphviz's default
    /// `0.75 x 0.5` inch node (`const.h`'s `DEFAULT_NODEWIDTH` / `DEFAULT_NODEHEIGHT`),
    /// which is what every node gets while its rendered label fits inside it.
    pub fn normal(rank: i32, lw: f64, rw: f64, ht: f64) -> Self {
        Self {
            rank,
            order: 0,
            saveorder: 0,
            coord: Coord::default(),
            lw,
            rw,
            ht,
            mval: -1.0,
            kind: Kind::Normal,
            weight_class: 0,
            mark: false,
            onstack: false,
            flat_out: Vec::new(),
            flat_in: Vec::new(),
            other: Vec::new(),
        }
    }

    /// `virtual_node` (`fastgr.c:200-213`) gives a virtual node a one-point box, and
    /// `class2`'s `incr_width` then adds `nodesep / 2` to each side, so its width is
    /// `nodesep + 1` points.
    pub fn virtual_node(nodesep: f64) -> Self {
        let half = (nodesep / 2.0).trunc();
        let mut node = Self::normal(0, 1.0 + half, 1.0 + half, 1.0);
        node.kind = Kind::Virtual;
        node
    }
}
