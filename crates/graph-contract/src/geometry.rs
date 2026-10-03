//! The geometry vocabulary (`prompt.md` §4): a closed, versioned set of node and edge
//! kinds. The discriminant is **one byte per snapshot, never one per element**, so the
//! attribute columns stay pure SoA and the transport face stays zero-copy.
//!
//! `Ribbon` (Sankey) and `Arc` (chord) have their tag numbers allocated here and are
//! deliberately **not** enum variants: a reserved kind must not be constructible,
//! only recognisable. Adding one later is additive — reserve the tag, not the
//! implementation.

use crate::snapshot::SnapshotError;
use core::fmt;

mod columns;

/// How every node in one snapshot is shaped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    feature = "codegen",
    derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)
)]
#[repr(u8)]
pub enum NodeGeometryKind {
    /// Columns `x[]`, `y[]`: force, spectral, MDS, circular, grid, geographic.
    Point = 0,
    /// Columns `x[]`, `y[]`, `r[]`: circle packing.
    Circle = 1,
    /// Columns `x[]`, `y[]`, `w[]`, `h[]`: treemap, matrix cell, layered node.
    Box = 2,
}

/// How every edge in one snapshot is shaped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    feature = "codegen",
    derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)
)]
#[repr(u8)]
pub enum EdgeGeometryKind {
    /// No columns: endpoints derive from node geometry, so straight edges cost zero bytes.
    Line = 0,
    /// CSR-shaped `offsets[]` (m+1) and `pts[]`: layered bends, orthogonal routes, bundles.
    Polyline = 1,
    /// `offsets[]`, `pts[]` and a degree tag: bezier and quadratic styles, FDEB output.
    Curve = 2,
}

/// Tag allocated to Sankey ribbons. Reserved: recognised, never produced.
pub const RIBBON_TAG: u8 = 3;
/// Tag allocated to chord arcs. Reserved: recognised, never produced.
pub const ARC_TAG: u8 = 4;

/// Why a geometry tag byte was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagError {
    /// The tag belongs to a reserved kind that this reader does not implement.
    Reserved(u8),
    /// The tag is not allocated to any kind.
    Unknown(u8),
}

impl fmt::Display for TagError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reserved(tag) => write!(f, "geometry tag {tag} is reserved and not implemented"),
            Self::Unknown(tag) => write!(f, "geometry tag {tag} is not allocated"),
        }
    }
}

impl NodeGeometryKind {
    /// The byte this kind is written as.
    pub const fn tag(self) -> u8 {
        self as u8
    }

    /// Reads a node-kind byte. No node tag is reserved yet, so anything else is unknown.
    pub const fn from_tag(tag: u8) -> Result<Self, TagError> {
        match tag {
            0 => Ok(Self::Point),
            1 => Ok(Self::Circle),
            2 => Ok(Self::Box),
            other => Err(TagError::Unknown(other)),
        }
    }
}

impl EdgeGeometryKind {
    /// The byte this kind is written as.
    pub const fn tag(self) -> u8 {
        self as u8
    }

    /// Reads an edge-kind byte, telling a reserved tag apart from an unallocated one.
    pub const fn from_tag(tag: u8) -> Result<Self, TagError> {
        match tag {
            0 => Ok(Self::Line),
            1 => Ok(Self::Polyline),
            2 => Ok(Self::Curve),
            RIBBON_TAG | ARC_TAG => Err(TagError::Reserved(tag)),
            other => Err(TagError::Unknown(other)),
        }
    }
}

/// Every node's geometry in one snapshot, as columns. The variant is the snapshot's one
/// node discriminant; each column holds one `f32` per node, in node order.
#[derive(Debug, Clone, PartialEq)]
pub enum NodeGeometry {
    /// Centres.
    Point {
        /// Horizontal centre.
        x: Vec<f32>,
        /// Vertical centre.
        y: Vec<f32>,
    },
    /// Centres and radii.
    Circle {
        /// Horizontal centre.
        x: Vec<f32>,
        /// Vertical centre.
        y: Vec<f32>,
        /// Radius, never negative.
        r: Vec<f32>,
    },
    /// Boxes.
    Box {
        /// Horizontal centre.
        x: Vec<f32>,
        /// Vertical centre.
        y: Vec<f32>,
        /// Width, never negative.
        w: Vec<f32>,
        /// Height, never negative.
        h: Vec<f32>,
    },
}

/// Every edge's geometry in one snapshot. The variant is the snapshot's one edge
/// discriminant.
#[derive(Debug, Clone, PartialEq)]
pub enum EdgeGeometry {
    /// Straight from source to target: nothing stored, endpoints come from node geometry.
    Line,
    /// Straight segments through each edge's interior points.
    Polyline(Paths),
    /// A curve of one degree for the whole snapshot through each edge's control points.
    Curve {
        /// The curve degree, at least 1 (2 quadratic, 3 cubic).
        degree: u32,
        /// Each edge's control points.
        paths: Paths,
    },
}

/// The points of every edge, CSR-shaped like the adjacency: edge `e` owns points
/// `offsets[e]..offsets[e + 1]`, and point `p` is `(pts[2p], pts[2p + 1])`. A row holds
/// the **interior** points, source to target; an empty row is drawn straight.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Paths {
    /// `m + 1` point offsets, from 0, never decreasing.
    pub offsets: Vec<u32>,
    /// `2 × offsets[m]` coordinates, `x` then `y` for each point.
    pub pts: Vec<f32>,
}

impl NodeGeometry {
    /// The discriminant written in the header.
    pub const fn kind(&self) -> NodeGeometryKind {
        match self {
            Self::Point { .. } => NodeGeometryKind::Point,
            Self::Circle { .. } => NodeGeometryKind::Circle,
            Self::Box { .. } => NodeGeometryKind::Box,
        }
    }

    /// Every column with its wire name, in wire order. A 2D snapshot's columns; a 3D
    /// one is [`columns_dim`](Self::columns_dim) with its z.
    pub fn columns(&self) -> Vec<(&'static str, &[f32])> {
        match self {
            Self::Point { x, y } => vec![("x", x), ("y", y)],
            Self::Circle { x, y, r } => vec![("x", x), ("y", y), ("r", r)],
            Self::Box { x, y, w, h } => vec![("x", x), ("y", y), ("w", w), ("h", h)],
        }
    }

    /// Every column in wire order, with the z column spliced in right after `y` when the
    /// snapshot is 3D. Coordinates stay contiguous (`x, y, z`) and the sizes shift one
    /// word along, which is why a reader takes column positions from `dim` in the header
    /// rather than from fixed offsets (`docs/contract/binary-layout.md`).
    pub fn columns_dim<'a>(&'a self, z: Option<&'a [f32]>) -> Vec<(&'static str, &'a [f32])> {
        columns::columns_dim(self, z)
    }

    /// `Ok` when every column — the z column included, when `z` is given — has `n` finite
    /// values and no size is negative. A z is a coordinate, so it may be negative; only
    /// `r`, `w` and `h` are sizes.
    pub fn check(&self, n: u32, z: Option<&[f32]>) -> Result<(), SnapshotError> {
        columns::check(self, n, z)
    }
}

impl EdgeGeometry {
    /// The discriminant written in the header.
    pub const fn kind(&self) -> EdgeGeometryKind {
        match self {
            Self::Line => EdgeGeometryKind::Line,
            Self::Polyline(_) => EdgeGeometryKind::Polyline,
            Self::Curve { .. } => EdgeGeometryKind::Curve,
        }
    }

    /// `Ok` when the paths fit `m` edges and a curve's degree is at least 1.
    pub fn check(&self, m: u32) -> Result<(), SnapshotError> {
        match self {
            Self::Line => Ok(()),
            Self::Polyline(paths) => paths.check(m),
            Self::Curve { degree: 0, .. } => Err(SnapshotError::CurveDegree),
            Self::Curve { paths, .. } => paths.check(m),
        }
    }
}

impl Paths {
    /// `Ok` when there are `m + 1` offsets from 0, never decreasing, and exactly the
    /// finite coordinates the last one calls for.
    pub fn check(&self, m: u32) -> Result<(), SnapshotError> {
        check_len("edge.offsets", u64::from(m) + 1, self.offsets.len())?;
        if self.offsets[0] != 0 {
            return Err(SnapshotError::Offsets {
                column: "edge.offsets",
                index: 0,
            });
        }
        if let Some(bad) = self.offsets.windows(2).position(|w| w[1] < w[0]) {
            return Err(SnapshotError::Offsets {
                column: "edge.offsets",
                index: index_u32(bad + 1),
            });
        }
        let points = self.offsets[self.offsets.len() - 1];
        check_len("edge.pts", 2 * u64::from(points), self.pts.len())?;
        check_finite("edge.pts", &self.pts)
    }
}

/// A wire column name as the refusals name it. Every arm is spelled out, and a name this
/// reader does not know is `None` rather than the last arm's: reporting it under `node.h`
/// is the silent-mislabel bug the 3D `z` arm exists to prevent.
fn node_column(name: &str) -> Option<&'static str> {
    Some(match name {
        "x" => "node.x",
        "y" => "node.y",
        "z" => "node.z",
        "r" => "node.r",
        "w" => "node.w",
        "h" => "node.h",
        _ => return None,
    })
}

/// `Ok` when `found` is `expected`.
pub(crate) fn check_len(
    column: &'static str,
    expected: u64,
    found: usize,
) -> Result<(), SnapshotError> {
    let found = u64::try_from(found).unwrap_or(u64::MAX);
    if found != expected {
        return Err(SnapshotError::Length {
            column,
            expected,
            found,
        });
    }
    Ok(())
}

/// `Ok` when no value is NaN or ±∞ (D9).
pub(crate) fn check_finite(column: &'static str, values: &[f32]) -> Result<(), SnapshotError> {
    match values.iter().position(|v| !v.is_finite()) {
        Some(bad) => Err(SnapshotError::NonFinite {
            column,
            index: index_u32(bad),
        }),
        None => Ok(()),
    }
}

/// A position as it is reported; positions past `u32::MAX` cannot be built.
pub(crate) fn index_u32(index: usize) -> u32 {
    u32::try_from(index).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests;
