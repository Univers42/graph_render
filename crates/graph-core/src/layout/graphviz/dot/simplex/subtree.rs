//! `feasible_tree`'s bookkeeping: the tight subtrees and the size-ordered heap they are
//! merged through (`ns.c:310-332, 419-574`).
//!
//! A *subtree* is a set of nodes reached from one seed by tight edges. They are collected
//! all at once, then merged smallest first, each merge going through the minimum-slack
//! edge that leaves the smallest one — which is what makes the resulting tree the
//! reference's and not merely a tight tree.
//!
//! The heap is a binary min-heap keyed on `size`, held **in place** over the subtree array
//! exactly as `STheap_t` does (`ns.c:552-560`): a subtree's `heap_index` is its slot, and
//! `None` means it has been taken off the heap, which is also how `STsetUnion` decides
//! which of two subtrees keeps its identity.
//!
//! Identity is a union-find over the array slots. Every merge leaves the *larger* subtree
//! as the representative, and on a tie the one on the head's side (`ns.c:434-451`); the
//! representative of a merge is always a subtree still on the heap, because exactly one of
//! the two is the one just extracted, so the caller's `heap_index` is always live.

/// `ND_subtree`'s "not in any tight subtree" value. The reference spells that a null
/// pointer; the slot index has to be an integer here, so it is -1.
pub const NO_TREE: i32 = -1;

/// One tight subtree: `subtree_t` (`ns.c:310-314`).
pub struct Subtree {
    /// `rep`: the node the search started from. Every walk over this subtree starts here,
    /// and the field is never rewritten by a merge — only `par` is.
    pub rep: u32,
    /// `size`: how many nodes it holds. The heap's key, and the only thing a merge uses to
    /// choose a representative.
    pub size: usize,
    /// `par`: the union-find parent slot, itself when this subtree is a root.
    pub par: usize,
    /// `heap_index`: its slot in the heap while it is on the heap, `None` once extracted.
    pub heap_index: Option<usize>,
}

/// `STsetFind` (`ns.c:424-432`): the subtree's root, compressing the path as it walks.
pub fn find(subtrees: &mut [Subtree], slot: usize) -> usize {
    let mut at = slot;
    while subtrees[at].par != at {
        let grand = subtrees[at].par;
        if subtrees[grand].par != grand {
            subtrees[at].par = grand;
        }
        at = subtrees[at].par;
    }
    at
}

/// `STsetUnion` (`ns.c:434-451`): the two roots merge into whichever is larger — and on a
/// tie into the head's — carrying the summed size. A subtree that is off the heap loses
/// either way, which is what keeps the representative on the heap for the caller. The
/// returned slot is the representative.
pub fn union(subtrees: &mut [Subtree], tail: usize, head: usize) -> usize {
    let rep = if subtrees[head].heap_index.is_none() {
        tail
    } else if subtrees[tail].heap_index.is_none() {
        head
    } else if subtrees[head].size < subtrees[tail].size {
        tail
    } else {
        head
    };
    let size = subtrees[tail].size + subtrees[head].size;
    subtrees[tail].par = rep;
    subtrees[head].par = rep;
    subtrees[rep].size = size;
    rep
}

/// `STheapify` (`ns.c:534-550`), children indexed the reference's way: slot `i` has
/// children `2(i+1)-1` and `2(i+1)`.
pub fn sift_down(subtrees: &mut [Subtree], size: usize, mut at: usize) {
    loop {
        let left = 2 * (at + 1) - 1;
        let right = 2 * (at + 1);
        let mut smallest = at;
        if left < size && subtrees[left].size < subtrees[smallest].size {
            smallest = left;
        }
        if right < size && subtrees[right].size < subtrees[smallest].size {
            smallest = right;
        }
        if smallest == at {
            return;
        }
        subtrees.swap(at, smallest);
        subtrees[at].heap_index = Some(at);
        subtrees[smallest].heap_index = Some(smallest);
        at = smallest;
    }
}

/// `STbuildheap` (`ns.c:552-560`): every slot on the heap, then sift down from the last
/// parent. `STbuildheap` runs on a whole array the caller has already filled.
pub fn build_heap(subtrees: &mut [Subtree]) {
    let size = subtrees.len();
    for (at, subtree) in subtrees.iter_mut().enumerate() {
        subtree.heap_index = Some(at);
    }
    let mut at = size / 2;
    loop {
        sift_down(subtrees, size, at);
        if at == 0 {
            return;
        }
        at -= 1;
    }
}

/// `STextractmin` (`ns.c:562-574`): the smallest subtree off the heap. The reference leaves
/// it at the vacated slot so the array can be walked to free it; the returned index is
/// that slot, and the new heap length is `size - 1`.
pub fn extract_min(subtrees: &mut [Subtree], size: usize) -> usize {
    let last = size - 1;
    subtrees[0].heap_index = None;
    subtrees.swap(0, last);
    subtrees[0].heap_index = Some(0);
    sift_down(subtrees, last, 0);
    last
}