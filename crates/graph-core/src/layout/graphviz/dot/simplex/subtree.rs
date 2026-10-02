//! `feasible_tree`'s bookkeeping: the tight subtrees and the size-ordered heap they are
//! merged through (`ns.c:310-332, 419-574`).
//!
//! A *subtree* is a set of nodes reached from one seed by tight edges. They are collected
//! all at once, then merged smallest first, each merge going through the minimum-slack edge
//! that leaves the smallest one — which is what makes the resulting tree the reference's and
//! not merely a tight tree.
//!
//! Two structures, deliberately not one. `Subtree`s live in a `Vec` whose slot never moves,
//! and `par` is a slot index: that is the union-find, and a find has to be able to follow a
//! link without looking anything up by name. The **heap is a separate `Vec` of slot indices**
//! that permutes freely. The reference keeps both in one array of pointers, which is why it
//! can swap heap entries without noticing the difference; an index-based union-find in the
//! same array would follow a link into whichever subtree the swap moved there.
//!
//! Identity is decided in `union`: the two merge into the larger subtree, and on a tie into
//! the head's. A subtree that is off the heap loses either way, which is what keeps the
//! representative on the heap — the caller needs its `heap_index` — and it always is, because
//! exactly one of the two merged is the one just extracted.
//!
//! Determinism: the heap is a plain binary min-heap compared on the subtree size, and every
//! tie in the reference's `STheapify` resolves to the same child here, so the merge order is
//! the reference's.

/// `ND_subtree`'s "not in any tight subtree" value. The reference spells that a null
/// pointer; the slot index has to be an integer here, so it is -1.
pub const NO_TREE: i32 = -1;

/// One tight subtree: `subtree_t` (`ns.c:310-314`).
pub struct Subtree {
    /// `rep`: the node the search started from. Every walk over this subtree starts here,
    /// and a merge never rewrites it — only `par` moves.
    pub rep: u32,
    /// `size`: how many nodes it holds. The heap's key, and the only thing a merge uses to
    /// choose a representative.
    pub size: usize,
    /// `par`: the union-find parent slot, itself when this subtree is a root.
    pub par: usize,
    /// `heap_index`: its position in the heap while it is on the heap, `None` once
    /// extracted.
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
/// either way. The returned slot is the representative.
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

/// `STheapify` (`ns.c:534-550`), children indexed the reference's way: position `i` has
/// children `2(i+1)-1` and `2(i+1)`. Only the first `size` positions of `heap` are live.
pub fn sift_down(heap: &mut [usize], subtrees: &mut [Subtree], size: usize, mut at: usize) {
    loop {
        let left = 2 * (at + 1) - 1;
        let right = 2 * (at + 1);
        let mut smallest = at;
        if left < size && subtrees[heap[left]].size < subtrees[heap[smallest]].size {
            smallest = left;
        }
        if right < size && subtrees[heap[right]].size < subtrees[heap[smallest]].size {
            smallest = right;
        }
        if smallest == at {
            return;
        }
        heap.swap(at, smallest);
        subtrees[heap[at]].heap_index = Some(at);
        subtrees[heap[smallest]].heap_index = Some(smallest);
        at = smallest;
    }
}

/// `STextractmin` (`ns.c:562-574`): the smallest subtree off the heap. The reference leaves
/// it at the vacated slot so the array can still be walked to free it, and returns the
/// pointer; here the subtree's **slot** is returned, which is what identifies it — the
/// vacated heap position does not. The new heap length is the `size` the caller had, less
/// one.
pub fn extract_min(heap: &mut [usize], subtrees: &mut [Subtree], size: usize) -> usize {
    let last = size - 1;
    let taken = heap[0];
    subtrees[taken].heap_index = None;
    heap[0] = heap[last];
    subtrees[heap[0]].heap_index = Some(0);
    heap[last] = taken;
    sift_down(heap, subtrees, last, 0);
    taken
}

/// `STbuildheap` (`ns.c:552-560`): every slot on the heap, then sift down from the last
/// parent. `heap` is the caller's array of every slot in its natural order.
pub fn build_heap(heap: &mut [usize], subtrees: &mut [Subtree]) {
    let size = heap.len();
    for (at, &slot) in heap.iter().enumerate() {
        subtrees[slot].heap_index = Some(at);
    }
    let mut at = size / 2;
    loop {
        sift_down(heap, subtrees, size, at);
        if at == 0 {
            return;
        }
        at -= 1;
    }
}