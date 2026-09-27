//! The geometry vocabulary (`prompt.md` §4): a closed, versioned set of node and edge
//! kinds. The discriminant is **one byte per snapshot, never one per element**, so the
//! attribute columns stay pure SoA and the transport face stays zero-copy.
//!
//! `Ribbon` (Sankey) and `Arc` (chord) have their tag numbers allocated here and are
//! deliberately **not** enum variants: a reserved kind must not be constructible,
//! only recognisable. Adding one later is additive — reserve the tag, not the
//! implementation.

use core::fmt;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_node_kind_round_trips_through_its_tag() {
        for kind in [
            NodeGeometryKind::Point,
            NodeGeometryKind::Circle,
            NodeGeometryKind::Box,
        ] {
            assert_eq!(NodeGeometryKind::from_tag(kind.tag()), Ok(kind));
        }
    }

    #[test]
    fn every_edge_kind_round_trips_through_its_tag() {
        for kind in [
            EdgeGeometryKind::Line,
            EdgeGeometryKind::Polyline,
            EdgeGeometryKind::Curve,
        ] {
            assert_eq!(EdgeGeometryKind::from_tag(kind.tag()), Ok(kind));
        }
    }

    #[test]
    fn reserved_edge_tags_are_refused_as_reserved_not_unknown() {
        assert_eq!(
            EdgeGeometryKind::from_tag(RIBBON_TAG),
            Err(TagError::Reserved(3))
        );
        assert_eq!(
            EdgeGeometryKind::from_tag(ARC_TAG),
            Err(TagError::Reserved(4))
        );
        assert_eq!(EdgeGeometryKind::from_tag(5), Err(TagError::Unknown(5)));
        assert_eq!(NodeGeometryKind::from_tag(3), Err(TagError::Unknown(3)));
    }
}
