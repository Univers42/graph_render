//! Which arm of the spectral family a call draws: `layout.spectral`/`layout.mds.pivot`
//! solve two coordinates per node, and `layout.spectral3d`/`layout.mds.pivot3d` solve the
//! three the reference's own `_spectral_layout_3d`/`_mds_layout_3d` solve.
//!
//! **One enum, not a `dims: usize`, because the two arms also disagree about a short
//! block.** `_spectral_component_coordinates:158-159` fills a dimension the solve did not
//! reach by repeating the last column it did; `_pivot_mds_component_coordinates:214` writes
//! only what it solved and leaves the rest at the origin. Carrying that as a fifth
//! parameter would put five parameters on every scatter call for a difference two call
//! sites between them already know.

use super::{DIMS, DIMS_3D};

/// The arm. `Copy` so a loop can hold it across calls without borrowing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Width {
    /// `layout.spectral`: two coordinates, spectral's repeat-the-last-column rule.
    Spectral2d,
    /// `layout.spectral3d`: three coordinates, the same rule.
    Spectral3d,
    /// `layout.mds.pivot`: two coordinates, write-only-what-solved.
    PivotMds2d,
    /// `layout.mds.pivot3d`: three coordinates, write-only-what-solved.
    PivotMds3d,
}

impl Width {
    /// Coordinates per node.
    pub(crate) fn dims(self) -> usize {
        if self.in_space() { DIMS_3D } else { DIMS }
    }

    /// Whether this arm draws a volume. The three-dimensional arm also ends with
    /// `_rescale_positions`, which the two-dimensional one never runs — that is what keeps
    /// the 2D ids' bytes today's.
    pub(crate) fn in_space(self) -> bool {
        matches!(self, Width::Spectral3d | Width::PivotMds3d)
    }

    /// Columns to write for a solve that produced `solved`: every dimension under spectral's
    /// rule, `min(solved, dims)` under pivot MDS's.
    pub(crate) fn columns(self, solved: usize) -> usize {
        match self {
            Width::Spectral2d | Width::Spectral3d => self.dims(),
            Width::PivotMds2d | Width::PivotMds3d => solved.min(self.dims()),
        }
    }
}