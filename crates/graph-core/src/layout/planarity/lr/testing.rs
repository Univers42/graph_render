//! Phase 2: test the DFS orientation for a valid left-right partition
//! (`LRPlanarity.dfs_testing`, `add_constraints`, `remove_back_edges`). Iterative, same
//! reason as [`super::orient`]. Returns `false` the moment a conflict proves the graph
//! non-planar; `test_all` stops there without touching phase 3.

use super::{ConflictPair, Interval, Lr};

impl Lr<'_> {
    /// Tests every root's tree in turn (`for v in self.roots: if not dfs_testing(v): ...`).
    /// `ind`/`skip_init` are shared with [`super::orient`]'s DFS (one array apiece, keyed
    /// by node/slot, not per-phase) and phase 1 leaves both at their fully-advanced
    /// state, so this DFS needs its own fresh start here — exactly the fresh
    /// `defaultdict`s the reference allocates per `dfs_testing` call.
    pub(super) fn test_all(&mut self) -> bool {
        self.ind.fill(0);
        self.skip_init.fill(false);
        let roots = self.roots.clone();
        roots.iter().all(|&r| self.test_from(r))
    }

    /// `dfs_testing`, iterative.
    fn test_from(&mut self, start: u32) -> bool {
        let mut stack = vec![start];
        while let Some(v) = stack.pop() {
            let parent_slot = self.parent_edge[v as usize];
            let mut skip_final = false;
            let row = self.ordered[v as usize].clone();
            let mut i = self.ind[v as usize] as usize;
            while i < row.len() {
                let ei = row[i];
                if !self.skip_init[ei as usize] && !self.enter_edge(v, ei, &mut stack) {
                    self.ind[v as usize] = i as u32;
                    skip_final = true;
                    break;
                }
                if self.lowpt[ei as usize] < self.height[v as usize].expect("v has a height")
                    && !self.integrate_return_edge(i, ei, parent_slot)
                {
                    return false;
                }
                i += 1;
                self.ind[v as usize] = i as u32;
            }
            if !skip_final && let (Some(e), Some(u)) = (parent_slot, self.parent[v as usize]) {
                self.remove_back_edges(e, u);
            }
        }
        true
    }

    /// The init block of one loop iteration: records the stack bottom, and for a tree
    /// edge pushes `v` and its child back onto `stack` and reports it (caller must
    /// `break`); for a back edge pushes its trivial conflict pair.
    fn enter_edge(&mut self, v: u32, ei: u32, stack: &mut Vec<u32>) -> bool {
        self.stack_bottom[ei as usize] = self.top_id();
        let w = self.adjacency.slot_neighbour(ei);
        if self.parent_edge[w as usize] == Some(ei) {
            stack.push(v);
            stack.push(w);
            self.skip_init[ei as usize] = true;
            return false;
        }
        self.lowpt_edge[ei as usize] = Some(ei);
        let id = self.fresh_id();
        self.stack.push(ConflictPair::back_edge(id, ei));
        true
    }

    /// "integrate new return edges": `ei` has a return edge above the parent's height.
    fn integrate_return_edge(&mut self, i: usize, ei: u32, parent_slot: Option<u32>) -> bool {
        let Some(e) = parent_slot else { return true };
        if i == 0 {
            self.lowpt_edge[e as usize] = self.lowpt_edge[ei as usize];
            true
        } else {
            self.add_constraints(ei, e)
        }
    }

    /// `add_constraints`: merges `ei`'s return edges into the stack, `false` on conflict.
    fn add_constraints(&mut self, ei: u32, e: u32) -> bool {
        let mut p = ConflictPair {
            id: self.fresh_id(),
            ..ConflictPair::default()
        };
        let bottom = self.stack_bottom[ei as usize];
        loop {
            let mut q = self.stack.pop().expect("stack holds ei's own return edge");
            if !q.left.is_empty() {
                q.swap();
            }
            if !q.left.is_empty() {
                return false;
            }
            if self.lowpt[q.right.low.expect("q.right holds a return edge") as usize]
                > self.lowpt[e as usize]
            {
                if p.right.is_empty() {
                    p.right = q.right;
                } else if let Some(low) = p.right.low {
                    self.reference[low as usize] = q.right.high;
                }
                p.right.low = q.right.low;
            } else {
                self.reference[q.right.low.expect("checked above") as usize] =
                    self.lowpt_edge[e as usize];
            }
            if self.top_id() == bottom {
                break;
            }
        }
        self.merge_lower_conflicts(ei, &mut p);
        if !(p.left.is_empty() && p.right.is_empty()) {
            self.stack.push(p);
        }
        true
    }

    /// The second half of `add_constraints`: folds every conflicting pair below `ei`'s
    /// own return edges into `p.left`.
    fn merge_lower_conflicts(&mut self, ei: u32, p: &mut ConflictPair) -> bool {
        while self.top_conflicts(ei) {
            let mut q = self
                .stack
                .pop()
                .expect("top_conflicts checked a top exists");
            if q.right.conflicts_with(ei, &self.lowpt) {
                q.swap();
            }
            if q.right.conflicts_with(ei, &self.lowpt) {
                return false;
            }
            if let Some(low) = p.right.low {
                self.reference[low as usize] = q.right.high;
            }
            if let Some(low) = q.right.low {
                p.right.low = Some(low);
            }
            if p.left.is_empty() {
                p.left = q.left;
            } else if let Some(low) = p.left.low {
                self.reference[low as usize] = q.left.high;
            }
            p.left.low = q.left.low;
        }
        true
    }

    fn top_conflicts(&self, ei: u32) -> bool {
        self.stack.last().is_some_and(|top| {
            top.left.conflicts_with(ei, &self.lowpt) || top.right.conflicts_with(ei, &self.lowpt)
        })
    }

    /// `remove_back_edges(e)`: trims the stack once `v`'s (parent node `u`) loop over its
    /// own edges is done, and records the side of `e` itself if it has a return edge.
    pub(super) fn remove_back_edges(&mut self, e: u32, u: u32) {
        let hu = self.height[u as usize].expect("u is an ancestor, so it has a height");
        while self
            .stack
            .last()
            .is_some_and(|top| top.lowest(&self.lowpt) == hu)
        {
            let p = self.stack.pop().expect("checked above");
            if let Some(low) = p.left.low {
                self.side[low as usize] = -1;
            }
        }
        if let Some(p) = self.stack.pop() {
            let p = self.trim_conflict_pair(p, u);
            self.stack.push(p);
        }
        if self.lowpt[e as usize] < hu {
            let top = self
                .stack
                .last()
                .expect("e has a return edge, so a pair exists");
            let (hl, hr) = (top.left.high, top.right.high);
            self.reference[e as usize] = match (hl, hr) {
                (Some(l), Some(r)) if self.lowpt[l as usize] <= self.lowpt[r as usize] => Some(r),
                (Some(l), _) => Some(l),
                (None, r) => r,
            };
        }
    }

    /// Trims `p`'s two intervals of every return edge that ends at `u` itself.
    fn trim_conflict_pair(&mut self, mut p: ConflictPair, u: u32) -> ConflictPair {
        p.left = self.trim_interval(p.left, u, p.right.low);
        p.right = self.trim_interval(p.right, u, p.left.low);
        p
    }

    /// Trims one interval, wiring its lowest edge to `other_low` and flagging it `-1`
    /// (the opposite side) if the trim empties it out entirely.
    fn trim_interval(&mut self, mut side: Interval, u: u32, other_low: Option<u32>) -> Interval {
        while let Some(high) = side.high {
            if self.adjacency.slot_neighbour(high) != u {
                break;
            }
            side.high = self.reference[high as usize];
        }
        if side.high.is_none()
            && let Some(low) = side.low
        {
            self.reference[low as usize] = other_low;
            self.side[low as usize] = -1;
            side.low = None;
        }
        side
    }
}
