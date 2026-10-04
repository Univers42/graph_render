//! Split from `octree.rs` for the house line cap (the split `quadtree.rs`/`bounds.rs`
//! already uses). The only differences from `quadtree/bounds.rs` are the third axis and
//! the three-bit slot.

/// A cube (or, before the second `cover`, a unit box) in tree-building coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Bounds3 {
    pub(crate) x0: f64,
    pub(crate) y0: f64,
    pub(crate) z0: f64,
    pub(crate) x1: f64,
    pub(crate) y1: f64,
    pub(crate) z1: f64,
}

impl Default for Bounds3 {
    fn default() -> Self {
        Self {
            x0: f64::NAN,
            y0: f64::NAN,
            z0: f64::NAN,
            x1: f64::NAN,
            y1: f64::NAN,
            z1: f64::NAN,
        }
    }
}

impl Bounds3 {
    /// The cube's edge. It is `NaN` on a cube whose bounds went non-finite, which every
    /// `>` / `<` against it reads as "no longer making progress" — the bail both `cover`'s
    /// growth and `insert_leaf`'s split step share.
    pub(super) fn span(self) -> f64 {
        self.x1 - self.x0
    }

    /// Narrows to the octant containing `(x, y, z)`; returns its slot, `z<<2 | y<<1 | x`,
    /// where each bit is "at or above this axis' midpoint".
    ///
    /// The three halves are written out rather than folded into a loop so the slot's bit
    /// order is one line a reader can check against [`Self::octant`], which consumes it.
    pub(super) fn narrow(&mut self, x: f64, y: f64, z: f64) -> usize {
        let (xm, ym, zm) = self.mid();
        if x >= xm {
            self.x0 = xm;
        } else {
            self.x1 = xm;
        }
        if y >= ym {
            self.y0 = ym;
        } else {
            self.y1 = ym;
        }
        if z >= zm {
            self.z0 = zm;
        } else {
            self.z1 = zm;
        }
        (((z >= zm) as usize) << 2) | (((y >= ym) as usize) << 1) | ((x >= xm) as usize)
    }

    /// One split iteration: narrows toward `(x, y, z)`, reports both points' slot,
    /// `(i, j)`.
    pub(super) fn split_step(&mut self, p: (f64, f64, f64), q: (f64, f64, f64)) -> (usize, usize) {
        let (xm, ym, zm) = self.mid();
        let i = self.narrow(p.0, p.1, p.2);
        let j =
            (((q.2 >= zm) as usize) << 2) | (((q.1 >= ym) as usize) << 1) | ((q.0 >= xm) as usize);
        (i, j)
    }

    pub(super) fn octant(self, slot: usize) -> Self {
        let (xm, ym, zm) = self.mid();
        let mut next = self;
        if slot & 1 != 0 {
            next.x0 = xm;
        } else {
            next.x1 = xm;
        }
        if slot & 2 != 0 {
            next.y0 = ym;
        } else {
            next.y1 = ym;
        }
        if slot & 4 != 0 {
            next.z0 = zm;
        } else {
            next.z1 = zm;
        }
        next
    }

    /// The three midpoints, in the order every method here names them.
    fn mid(self) -> (f64, f64, f64) {
        (
            (self.x0 + self.x1) / 2.0,
            (self.y0 + self.y1) / 2.0,
            (self.z0 + self.z1) / 2.0,
        )
    }
}
