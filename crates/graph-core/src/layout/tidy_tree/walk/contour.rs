//! `apportion` and the contour walk it runs, split out of `walk.rs` for the house line
//! limit. Everything here is a second `impl<'h> Walk<'h>` block: same type, same
//! privacy — [`super::Walk`]'s fields and `walk.rs`'s other private methods
//! (`next_left`/`next_right`/`separation`/`move_subtree`/`next_ancestor`) are visible
//! here because this module nests under `walk`, not because anything was widened.

use super::Walk;

/// `apportion`'s four contour cursors and their running mod sums (`vi±`/`vo±` and
/// `si±`/`so±` in `tree.js`'s comments): `i`nside/`o`utside, `-`/`+` for the left (`w`'s)
/// and right (`v`'s) subtree.
struct Contour {
    vim: Option<u32>,
    vip: Option<u32>,
    vom: u32,
    vop: u32,
    sip: f64,
    sop: f64,
    sim: f64,
    som: f64,
}

impl<'h> Walk<'h> {
    /// `apportion(v, w, ancestor)`: the contour walk that threads and shifts subtrees so
    /// `v` clears its left sibling `w`, one step per [`Self::step_contour`] then
    /// [`Self::finish_contour`] — split out to keep each under the house line limit.
    pub(super) fn apportion(&mut self, v: u32, w: Option<u32>, ancestor: u32) -> u32 {
        let Some(w) = w else { return ancestor };
        let parent = self.parent_of(v).expect("w is Some only for a non-root");
        let vom = self.h.children(parent)[0];
        let mut c = Contour {
            vim: Some(w),
            vip: Some(v),
            vom,
            vop: v,
            sip: self.st.m[v as usize],
            sop: self.st.m[v as usize],
            sim: self.st.m[w as usize],
            som: self.st.m[vom as usize],
        };
        while self.step_contour(v, ancestor, &mut c) {}
        self.finish_contour(v, &c, ancestor)
    }

    /// One pass of the contour walk: advances `vim`/`vip`, and — while both are still on
    /// the tree — shifts the inner subtree clear of the outer one and folds both
    /// contours' mods into `c`'s running sums. `false` once either side runs out.
    fn step_contour(&mut self, v: u32, ancestor: u32, c: &mut Contour) -> bool {
        c.vim = c.vim.and_then(|x| self.next_right(x));
        c.vip = c.vip.and_then(|x| self.next_left(x));
        let (Some(a), Some(b)) = (c.vim, c.vip) else {
            return false;
        };
        let reach = "the outside contour reaches as far";
        c.vom = self.next_left(c.vom).expect(reach);
        c.vop = self.next_right(c.vop).expect(reach);
        self.st.anc[c.vop as usize] = v;
        let shift =
            self.st.z[a as usize] + c.sim - self.st.z[b as usize] - c.sip + self.separation(a, b);
        if shift > 0.0 {
            let wm = self.next_ancestor(a, v, ancestor);
            self.move_subtree(wm, v, shift);
            c.sip += shift;
            c.sop += shift;
        }
        c.sim += self.st.m[a as usize];
        c.sip += self.st.m[b as usize];
        c.som += self.st.m[c.vom as usize];
        c.sop += self.st.m[c.vop as usize];
        true
    }

    /// After the walk: thread whichever side ran out first onto the side still going, so
    /// a later `apportion` can resume the traversal through it — `tree.js`'s two trailing
    /// `if`s.
    fn finish_contour(&mut self, v: u32, c: &Contour, mut ancestor: u32) -> u32 {
        if let Some(a) = c.vim
            && self.next_right(c.vop).is_none()
        {
            self.st.thread[c.vop as usize] = Some(a);
            self.st.m[c.vop as usize] += c.sim - c.sop;
        }
        if let Some(b) = c.vip
            && self.next_left(c.vom).is_none()
        {
            self.st.thread[c.vom as usize] = Some(b);
            self.st.m[c.vom as usize] += c.sip - c.som;
            ancestor = v;
        }
        ancestor
    }
}
