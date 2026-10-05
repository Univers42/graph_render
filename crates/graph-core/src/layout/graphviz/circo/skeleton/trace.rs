//! A test-only trace of the skeleton pass, printed in the instrumented reference's own format
//! — every line kind both arms produce, so the two diff line for line
//! (`docs/measurements/p13-gv1-circo.md` §9).
//!
//! The whole module is behind `cfg(test)` and so is every call site, so the layout the library
//! builds has no trace in it at all. A test turns it on with [`enable`], handing it one block's
//! node names so a block-local index prints as the node name the reference prints for the same
//! node; every print sits behind [`for_block`], so only that block traces.
//!
//! Ponytail: one process-wide `OnceLock` for the names and one `AtomicBool` for the switch.
//! Failing input: two blocks of the *same* node count traced at once, the second reading its
//! indices through the first's names. Direction: a wrong node name in a test log. Escape
//! hatch: only `tests/tree_order.rs` calls [`enable`], once, for the 74-node block.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

use super::Work;

static ON: AtomicBool = AtomicBool::new(false);
static NAMES: OnceLock<Vec<u32>> = OnceLock::new();

/// Turns the trace on, with the block's node names: local index `i` is `block.nodes[i]`.
pub(super) fn enable(block: &[u32]) {
    if NAMES.set(block.to_vec()).is_err() {
        panic!("the skeleton trace is already enabled for another block");
    }
    ON.store(true, Ordering::Relaxed);
}

/// Whether a block of `count` nodes is the one the trace was enabled for. Every print in this
/// module is behind one of these, so a second block stays out of the log.
pub(super) fn for_block(count: usize) -> bool {
    ON.load(Ordering::Relaxed) && NAMES.get().is_some_and(|block| block.len() == count)
}

/// A block-local index as the node name the reference prints.
fn name(local: u32) -> String {
    let block = NAMES.get().expect("the trace is enabled before it prints");
    format!("n{}", block[local as usize])
}

/// A block-local index list as `[n0 n1 n2]`.
fn names(local: &[u32]) -> String {
    let mut out = String::new();
    for &node in local {
        out.push(' ');
        out.push_str(&name(node));
        out.push(' ');
    }
    out
}

/// `tr_dl`: the degree list as `name/DEGREE`, front to back.
pub(super) fn dl(tag: &str, list: &[u32], work: &Work) {
    let mut out = format!("TRACE {tag} dl [");
    for &node in list {
        out.push_str(&format!("{}/{} ", name(node), work.degree[node as usize]));
    }
    println!("{out}]");
}

/// One `remove_pair_edges` round: the node taken off the back and the row it walks.
pub(super) fn round(number: usize, current: u32, work: &Work) {
    let mut row = String::new();
    for &edge in &work.rows[current as usize] {
        if work.live[edge as usize] {
            row.push(' ');
            row.push_str(&name(work.other(edge, current)));
        }
    }
    println!("TRACE round {number} currnode={} row:{row}", name(current));
}

/// The three numbers `find_pair_edges` reads before it classifies anything.
pub(super) struct Pairing {
    pub(super) node: u32,
    pub(super) degree: i32,
    pub(super) count: i32,
}

/// `find_pair_edges`' classification line: which neighbours are joined to another neighbour
/// (`with`) and which are not (`without`).
pub(super) fn pairs(pairing: &Pairing, lists: (&[u32], &[u32])) {
    let (with, without) = lists;
    println!(
        "TRACE find_pair_edges n={} deg={} edge_cnt={} with=[{}] without=[{}]",
        name(pairing.node),
        pairing.degree,
        pairing.count,
        names(with).trim_start(),
        names(without).trim_start()
    );
}

/// The degree top-up `find_pair_edges` asks `pair_up` for.
pub(super) fn diff(value: i32) {
    println!("TRACE find_pair_edges diff={value}");
}

/// An edge `pair_up` adds to the working copy `g`.
pub(super) fn added(tail: u32, head: u32) {
    println!("TRACE   add to g: {} -- {}", name(tail), name(head));
}

/// An edge `find_pair_edges` drops from the spanning-tree graph `outg`.
pub(super) fn deleted(tail: u32, head: u32) {
    println!("TRACE   delete from outg: {} -- {}", name(tail), name(head));
}

/// `tr_nodes` and the `outg rows` block, once the skeleton pass is done.
pub(super) fn outg(work: &Work) {
    let mut line = String::from("TRACE outg nodes agfstnode order:");
    for node in 0..work.degree.len() as u32 {
        line.push(' ');
        line.push_str(&name(node));
    }
    println!("{line}");
    println!("TRACE outg rows:");
    for node in 0..work.degree.len() as u32 {
        let kept: Vec<String> = work
            .kept_row(node)
            .iter()
            .map(|&other| format!("{}->{}", name(node), name(other)))
            .collect();
        println!("TRACE   row {}: {}", name(node), kept.join(" "));
    }
}

/// Every node's tree parent. The rows `dfs` walks are [`outg`]'s — `blockpath.c:327` walks
/// `g`, the graph the tree is built in, and `agsubedge` only records which of them it took —
/// so the parents below are the whole of the tree, and `outg rows` is the walk.
pub(super) fn parents(parents: &[u32]) {
    for (node, &parent) in parents.iter().enumerate() {
        let parent = if parent == u32::MAX {
            "NULL".to_string()
        } else {
            name(parent)
        };
        println!("TRACE parent {} -> {}", name(node as u32), parent);
    }
}

/// One leaf `find_longest_path` hands to `measure_distance`.
pub(super) fn leaf(node: u32) {
    println!("TRACE leaf {}", name(node));
}

/// One step of the branch-node scan: this node's `DISTONE + DISTTWO`.
pub(super) fn scan(node: u32, length: u32) {
    println!("TRACE longest scan {} length={length}", name(node));
}

/// The branch node the scan settled on.
pub(super) fn branch(common: Option<u32>) {
    let common = common.map_or("NULL".to_string(), name);
    println!("TRACE common={common}");
}

/// `DISTONE`, `DISTTWO`, `LEAFONE` and `LEAFTWO` at the branch node.
pub(super) fn dist(node: u32, dists: (u32, u32), leaves: (u32, u32)) {
    let named = |leaf: u32| {
        if leaf == u32::MAX {
            "NULL".to_string()
        } else {
            name(leaf)
        }
    };
    println!(
        "TRACE branch {} DISTONE={} DISTTWO={} LEAFONE={} LEAFTWO={}",
        name(node),
        dists.0,
        dists.1,
        named(leaves.0),
        named(leaves.1)
    );
}
