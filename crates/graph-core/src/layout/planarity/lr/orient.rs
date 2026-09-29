//! Phase 1: orient every edge by DFS (`LRPlanarity.dfs_orientation`), computing each
//! oriented slot's height, lowpoint, second lowpoint and nesting depth on the way.
//! Iterative — an explicit stack stands in for the call stack, so a 100k-node path does
//! not recurse 100k deep.

use super::Lr;
use std::cmp::Ordering;

impl Lr<'_> {
    /// Orients every component, each from its own unvisited root, ascending dense index
    /// (`for v in self.G: if height[v] is None: ...`).
    pub(super) fn orient_all(&mut self) {
        for v in 0..self.adjacency.node_count() {
            if self.height[v as usize].is_none() {
                self.height[v as usize] = Some(0);
                self.roots.push(v);
                self.orient_from(v);
            }
        }
    }

    /// `dfs_orientation`, iterative.
    fn orient_from(&mut self, start: u32) {
        let mut stack = vec![start];
        while let Some(v) = stack.pop() {
            let parent_slot = self.parent_edge[v as usize];
            let row = self.adjacency.row(v);
            let mut i = self.ind[v as usize] as usize;
            while i < row.len() {
                let w = row[i];
                let slot = self.adjacency.start(v) + i as u32;
                if !self.skip_init[slot as usize] {
                    let mirror = self.adjacency.slot(w, v);
                    if self.oriented[slot as usize] || self.oriented[mirror as usize] {
                        i += 1;
                        self.ind[v as usize] = i as u32;
                        continue;
                    }
                    if self.orient_edge(v, w, slot, &mut stack) {
                        break; // w is a fresh tree child: finish it before resuming v
                    }
                }
                self.settle_nesting_depth(v, slot, parent_slot);
                i += 1;
                self.ind[v as usize] = i as u32;
            }
        }
    }

    /// Orients slot `(v, w)`. Returns `true` when `w` is newly discovered (a tree edge:
    /// `v` must pause for `w`'s subtree), having already pushed both back onto `stack`.
    fn orient_edge(&mut self, v: u32, w: u32, slot: u32, stack: &mut Vec<u32>) -> bool {
        self.oriented[slot as usize] = true;
        let hv = self.height[v as usize].expect("v is on the stack, so it has a height");
        self.lowpt[slot as usize] = hv;
        self.lowpt2[slot as usize] = hv;
        if self.height[w as usize].is_some() {
            self.lowpt[slot as usize] = self.height[w as usize].expect("checked Some");
            return false;
        }
        self.parent_edge[w as usize] = Some(slot);
        self.parent[w as usize] = Some(v);
        self.height[w as usize] = Some(hv + 1);
        // `ind[v]` is left pointing at this same slot, so v resumes here once w's
        // subtree is done and falls straight into `settle_nesting_depth` for it.
        self.skip_init[slot as usize] = true;
        stack.push(v);
        stack.push(w);
        true
    }

    /// Nesting depth for `slot`, then folds it into the parent edge's lowpoints.
    fn settle_nesting_depth(&mut self, v: u32, slot: u32, parent_slot: Option<u32>) {
        let hv = self.height[v as usize].expect("v is on the stack, so it has a height");
        let chordal = i32::from(self.lowpt2[slot as usize] < hv);
        self.nesting_depth[slot as usize] = 2 * self.lowpt[slot as usize] as i32 + chordal;
        if let Some(e) = parent_slot {
            self.propagate_lowpoints(e, slot);
        }
    }

    /// Folds slot `vw`'s lowpoints into its parent edge `e`'s.
    fn propagate_lowpoints(&mut self, e: u32, vw: u32) {
        let (lp_e, lp_vw) = (self.lowpt[e as usize], self.lowpt[vw as usize]);
        match lp_vw.cmp(&lp_e) {
            Ordering::Less => {
                self.lowpt2[e as usize] = lp_e.min(self.lowpt2[vw as usize]);
                self.lowpt[e as usize] = lp_vw;
            }
            Ordering::Greater => {
                self.lowpt2[e as usize] = self.lowpt2[e as usize].min(lp_vw);
            }
            Ordering::Equal => {
                self.lowpt2[e as usize] = self.lowpt2[e as usize].min(self.lowpt2[vw as usize]);
            }
        }
    }

    /// Sorts each node's DG-out slots by nesting depth (`sorted(self.DG[v], key=...)`),
    /// run once with the raw depth for testing, again with the signed depth for the
    /// final rotation.
    pub(super) fn order_by_nesting_depth(&mut self) {
        for v in 0..self.adjacency.node_count() {
            let row = self.adjacency.row(v);
            let start = self.adjacency.start(v);
            let mut slots: Vec<u32> = (0..row.len() as u32)
                .map(|i| start + i)
                .filter(|&s| self.oriented[s as usize])
                .collect();
            slots.sort_by_key(|&s| self.nesting_depth[s as usize]);
            self.ordered[v as usize] = slots;
        }
    }
}
