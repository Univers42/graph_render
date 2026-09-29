//! The Buchheim/Jünger/Leipert/Walker walk itself: `TreeNode`'s per-node fields
//! (`super::Walk`'s [`State`]) and the four passes `tree.js`'s `tree(root)` runs over
//! them (`firstWalk`/`apportion`/`secondWalk`/the fixed-size rescale). Split out of
//! `tidy_tree.rs` for the house line limit — see that module's docs for the port's
//! conventions and the oracle call sequence; nothing here departs from either.

use crate::layout::hierarchy::Hierarchy;

mod contour;

/// Every per-node field `firstWalk`/`apportion`/`secondWalk` read or wrote, indexed by
/// dense id (`0..=n`, row `n` the virtual root's when there is one). Field names match
/// `TreeNode` in `tree.js` (`z` prelim, `m` mod, `c` change, `s` shift, `t` thread).
pub(super) struct State {
    pub(super) z: Vec<f64>,
    pub(super) m: Vec<f64>,
    c: Vec<f64>,
    s: Vec<f64>,
    pub(super) x: Vec<f64>,
    pub(super) y: Vec<f64>,
    /// The `.a` field: each node's ancestor pointer, self by default.
    anc: Vec<u32>,
    /// The `.A` field: a parent's cached default ancestor across its children.
    big_a: Vec<Option<u32>>,
    thread: Vec<Option<u32>>,
    /// Position among its own parent's children (`TreeNode.i`); 0 for the root.
    sibling_index: Vec<u32>,
    /// The **layout** parent — read from [`Hierarchy::children`], not
    /// [`Hierarchy::parent`]: D-H's `parent()` is `None` for every root, virtual-rooted
    /// or not (it tracks a real kept hierarchy edge only), but `firstWalk`/`apportion`
    /// need a true tree parent for a root hung off the virtual root too. `None` only for
    /// the layout's own root.
    parent: Vec<Option<u32>>,
}

impl State {
    fn new(rows: u32) -> Self {
        let zeros = vec![0.0; rows as usize];
        Self {
            z: zeros.clone(),
            m: zeros.clone(),
            c: zeros.clone(),
            s: zeros.clone(),
            x: zeros.clone(),
            y: zeros,
            anc: (0..rows).collect(),
            big_a: vec![None; rows as usize],
            thread: vec![None; rows as usize],
            sibling_index: vec![0; rows as usize],
            parent: vec![None; rows as usize],
        }
    }
}

/// The algorithm's working context: the repaired forest, borrowed, and its own mutable
/// per-node state.
pub(super) struct Walk<'h> {
    h: &'h Hierarchy,
    pub(super) st: State,
}

impl<'h> Walk<'h> {
    pub(super) fn new(h: &'h Hierarchy) -> Self {
        Self {
            h,
            st: State::new(h.node_count() + 1),
        }
    }

    /// `t.eachAfter(firstWalk); t.parent.m = -t.z; t.eachBefore(secondWalk);` then the
    /// fixed-size rescale — `tree.js`'s `tree(root)`, `root` being `self.h`'s `root`.
    pub(super) fn layout(&mut self, root: u32) {
        self.index_siblings();
        for v in postorder(self.h, root) {
            self.first_walk(v);
        }
        self.second_walk(root);
        self.normalize(root);
    }

    /// `TreeNode.i` and `TreeNode.parent`, from the one CSR that has both: `parent(v)`
    /// is `None` only for the layout's own root ([`State::parent`]'s doc).
    fn index_siblings(&mut self) {
        for p in 0..=self.h.node_count() {
            for (i, &c) in self.h.children(p).iter().enumerate() {
                self.st.sibling_index[c as usize] = i as u32;
                self.st.parent[c as usize] = Some(p);
            }
        }
    }

    fn parent_of(&self, v: u32) -> Option<u32> {
        self.st.parent[v as usize]
    }

    fn next_left(&self, v: u32) -> Option<u32> {
        self.h
            .children(v)
            .first()
            .copied()
            .or(self.st.thread[v as usize])
    }

    fn next_right(&self, v: u32) -> Option<u32> {
        self.h
            .children(v)
            .last()
            .copied()
            .or(self.st.thread[v as usize])
    }

    /// `defaultSeparation`: `1` for siblings, `2` across a subtree boundary.
    fn separation(&self, a: u32, b: u32) -> f64 {
        if self.parent_of(a) == self.parent_of(b) {
            1.0
        } else {
            2.0
        }
    }

    /// `executeShifts(v)`.
    fn execute_shifts(&mut self, v: u32) {
        let (mut shift, mut change) = (0.0, 0.0);
        for &w in self.h.children(v).iter().rev() {
            self.st.z[w as usize] += shift;
            self.st.m[w as usize] += shift;
            change += self.st.c[w as usize];
            shift += self.st.s[w as usize] + change;
        }
    }

    /// `firstWalk(v)`.
    fn first_walk(&mut self, v: u32) {
        let own_children = self.h.children(v);
        let parent = self.parent_of(v);
        let i = self.st.sibling_index[v as usize];
        let siblings = parent.map(|p| self.h.children(p));
        let w = (i > 0).then(|| siblings.expect("a child has siblings")[i as usize - 1]);
        if let (Some(&first), Some(&last)) = (own_children.first(), own_children.last()) {
            self.execute_shifts(v);
            let midpoint = (self.st.z[first as usize] + self.st.z[last as usize]) / 2.0;
            match w {
                Some(w) => {
                    self.st.z[v as usize] = self.st.z[w as usize] + self.separation(v, w);
                    self.st.m[v as usize] = self.st.z[v as usize] - midpoint;
                }
                None => self.st.z[v as usize] = midpoint,
            }
        } else if let Some(w) = w {
            self.st.z[v as usize] = self.st.z[w as usize] + self.separation(v, w);
        }
        if let Some(p) = parent {
            let default = self.st.big_a[p as usize].unwrap_or(siblings.expect("has a parent")[0]);
            let updated = self.apportion(v, w, default);
            self.st.big_a[p as usize] = Some(updated);
        }
    }

    /// `nextAncestor(vim, v, ancestor)`.
    fn next_ancestor(&self, vim: u32, v: u32, ancestor: u32) -> u32 {
        let candidate = self.st.anc[vim as usize];
        if self.parent_of(candidate) == self.parent_of(v) {
            candidate
        } else {
            ancestor
        }
    }

    /// `moveSubtree(wm, wp, shift)`, `wp` always the node `apportion` is placing.
    fn move_subtree(&mut self, wm: u32, wp: u32, shift: f64) {
        let index = |v: u32| f64::from(self.st.sibling_index[v as usize]);
        let change = shift / (index(wp) - index(wm));
        self.st.c[wp as usize] -= change;
        self.st.s[wp as usize] += shift;
        self.st.c[wm as usize] += change;
        self.st.z[wp as usize] += shift;
        self.st.m[wp as usize] += shift;
    }

    /// `secondWalk`, with `t.parent.m = -t.z` folded in for `root`.
    fn second_walk(&mut self, root: u32) {
        let root_mod = -self.st.z[root as usize];
        self.st.x[root as usize] = self.st.z[root as usize] + root_mod;
        self.st.m[root as usize] += root_mod;
        for &v in self.h.order().iter().skip(1) {
            let parent_mod = self.st.m[self.parent_of(v).expect("not the root") as usize];
            self.st.x[v as usize] = self.st.z[v as usize] + parent_mod;
            self.st.m[v as usize] += parent_mod;
        }
    }

    /// The fixed-size rescale: `left`/`right`/`bottom` by a preorder walk (ties keep the
    /// first-visited node, exactly as d3's strict `<`/`>` do), then `x`/`y` fitted to
    /// `size([1, 1])`.
    fn normalize(&mut self, root: u32) {
        let pre = preorder(self.h, root);
        let (mut left, mut right, mut bottom) = (root, root, root);
        for &v in &pre {
            if self.st.x[v as usize] < self.st.x[left as usize] {
                left = v;
            }
            if self.st.x[v as usize] > self.st.x[right as usize] {
                right = v;
            }
            if self.h.depth(v) > self.h.depth(bottom) {
                bottom = v;
            }
        }
        let s = if left == right {
            1.0
        } else {
            self.separation(left, right) / 2.0
        };
        let tx = s - self.st.x[left as usize];
        let kx = 1.0 / (self.st.x[right as usize] + s + tx);
        let ky = 1.0 / f64::from(self.h.depth(bottom).max(1));
        for &v in &pre {
            self.st.x[v as usize] = (self.st.x[v as usize] + tx) * kx;
            self.st.y[v as usize] = f64::from(self.h.depth(v)) * ky;
        }
    }
}

/// Post-order (children ascending, then the node), from `root`: the two-stack technique
/// `hierarchy/eachAfter.js` uses, so the sequence matches it exactly.
fn postorder(h: &Hierarchy, root: u32) -> Vec<u32> {
    let mut stack = vec![root];
    let mut visited = Vec::new();
    while let Some(v) = stack.pop() {
        visited.push(v);
        stack.extend(h.children(v));
    }
    visited.reverse();
    visited
}

/// Pre-order (the node, then children ascending), from `root`: `hierarchy/eachBefore.js`.
fn preorder(h: &Hierarchy, root: u32) -> Vec<u32> {
    let mut stack = vec![root];
    let mut order = Vec::new();
    while let Some(v) = stack.pop() {
        order.push(v);
        stack.extend(h.children(v).iter().rev());
    }
    order
}
