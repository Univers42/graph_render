//! Split from `quadtree.rs` for the house line cap (the same split `hierarchy.rs`/
//! `hierarchy/tests.rs` already uses).

/// A square (or, before the second `cover`, a unit box) in tree-building coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Bounds {
    pub(crate) x0: f64,
    pub(crate) y0: f64,
    pub(crate) x1: f64,
    pub(crate) y1: f64,
}

impl Default for Bounds {
    fn default() -> Self {
        Self {
            x0: f64::NAN,
            y0: f64::NAN,
            x1: f64::NAN,
            y1: f64::NAN,
        }
    }
}

impl Bounds {
    /// Narrows to the quadrant containing `(x, y)`; returns its slot, `bottom<<1|right`
    /// (`add.js:31-32`).
    pub(super) fn narrow(&mut self, x: f64, y: f64) -> usize {
        let xm = (self.x0 + self.x1) / 2.0;
        let ym = (self.y0 + self.y1) / 2.0;
        let right = x >= xm;
        if right {
            self.x0 = xm
        } else {
            self.x1 = xm
        }
        let bottom = y >= ym;
        if bottom {
            self.y0 = ym
        } else {
            self.y1 = ym
        }
        ((bottom as usize) << 1) | (right as usize)
    }

    /// One split iteration: narrows toward `(x, y)`, reports both points' slot, `(i, j)`
    /// (`add.js:42-46`).
    pub(super) fn split_step(&mut self, x: f64, y: f64, xp: f64, yp: f64) -> (usize, usize) {
        let xm = (self.x0 + self.x1) / 2.0;
        let ym = (self.y0 + self.y1) / 2.0;
        let i = self.narrow(x, y);
        let j = (((yp >= ym) as usize) << 1) | ((xp >= xm) as usize);
        (i, j)
    }

    pub(super) fn quadrant(self, slot: usize) -> Self {
        let (xm, ym) = ((self.x0 + self.x1) / 2.0, (self.y0 + self.y1) / 2.0);
        match slot {
            0 => Self {
                x1: xm,
                y1: ym,
                ..self
            },
            1 => Self {
                x0: xm,
                y1: ym,
                ..self
            },
            2 => Self {
                x1: xm,
                y0: ym,
                ..self
            },
            _ => Self {
                x0: xm,
                y0: ym,
                ..self
            },
        }
    }
}
