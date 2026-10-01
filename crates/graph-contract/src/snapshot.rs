//! The snapshot's fixed header, and every reason a snapshot is refused.
//!
//! The header is the first 28 bytes of the binary face; the full byte layout, header
//! included, is `docs/contract/binary-layout.md`, which is authoritative. Little-endian,
//! no implicit padding:
//!
//! | offset | size | field                                             |
//! |-------:|-----:|---------------------------------------------------|
//! |      0 |    4 | magic `b"GMSN"`                                   |
//! |      4 |    4 | format major, `u32`                               |
//! |      8 |    4 | format minor, `u32`                               |
//! |     12 |    1 | node geometry tag (`NodeGeometryKind`)            |
//! |     13 |    1 | edge geometry tag (`EdgeGeometryKind`)            |
//! |     14 |    1 | `dim`: `0` 2D, no z column; `1` 3D (`Dim`)          |
//! |     15 |    1 | padding, must be `0`                              |
//! |     16 |    4 | stage count, `u32`; reserved, must be `1` for now |
//! |     20 |    4 | node count, `u32`                                 |
//! |     24 |    4 | edge count, `u32`                                 |
//!
//! Every integer on the wire is `u32` or a single tag byte — never `usize` (D6). The
//! header carries one [`Dim`] for the whole snapshot, so the node section's length is
//! readable from the header alone; see [`dim`] for the version rule that follows from it.

use crate::geometry::{EdgeGeometryKind, NodeGeometryKind, TagError};
use crate::version::{FormatVersion, NewerMajor, check_readable};
use core::fmt;

pub mod dim;

pub use dim::{DIM_SINCE_MINOR, Dim, carries_dim, label_for};

/// The four bytes every snapshot starts with.
pub const MAGIC: [u8; 4] = *b"GMSN";
/// Length of the fixed header in bytes.
pub const HEADER_LEN: u32 = 28;

/// The fixed-size head of every snapshot: one geometry discriminant for the whole payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    feature = "codegen",
    derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)
)]
pub struct SnapshotHeader {
    /// Format version the payload was written with.
    pub version: FormatVersion,
    /// Shape of every node in the snapshot.
    pub node_kind: NodeGeometryKind,
    /// Shape of every edge in the snapshot.
    pub edge_kind: EdgeGeometryKind,
    /// How many dimensions every node of the snapshot carries: `0` 2D with no z column,
    /// `1` 3D with one. Once for the whole payload, so the node section's length follows
    /// from this and the node kind alone. `2..=255` is reserved and refused.
    #[cfg_attr(feature = "codegen", schemars(schema_with = "dim::dim_schema"))]
    pub dim: Dim,
    /// Named geometries carried for one topology. Reserved: exactly `1` until implemented.
    #[cfg_attr(feature = "codegen", schemars(with = "u32", range(min = 1, max = 1)))]
    pub stage_count: StageCount,
    /// Number of nodes, and the length of every node column.
    pub node_count: u32,
    /// Number of edges.
    pub edge_count: u32,
}

/// The header's stage count. Reserved: [`StageCount::ONE`] is the only value that can be
/// built, so no writer can emit a header its own reader would refuse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    feature = "codegen",
    derive(serde::Serialize, serde::Deserialize),
    serde(try_from = "u32", into = "u32")
)]
pub struct StageCount(u32);

impl StageCount {
    /// One stage: the only count implemented.
    pub const ONE: Self = Self(1);

    /// The count as it goes on the wire.
    pub const fn get(self) -> u32 {
        self.0
    }
}

impl TryFrom<u32> for StageCount {
    type Error = ReadError;

    fn try_from(count: u32) -> Result<Self, ReadError> {
        match count {
            1 => Ok(Self::ONE),
            other => Err(ReadError::ReservedStageCount(other)),
        }
    }
}

impl From<StageCount> for u32 {
    fn from(count: StageCount) -> Self {
        count.0
    }
}

/// Why a snapshot header was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadError {
    /// Fewer bytes than the fixed header needs.
    Truncated {
        /// Bytes the header needs.
        needed: u32,
        /// Bytes that were present.
        found: u32,
    },
    /// The first four bytes are not `GMSN`.
    BadMagic,
    /// Written in a newer major version than this reader knows.
    UnsupportedMajor(NewerMajor),
    /// A geometry tag byte was reserved or unallocated.
    Geometry(TagError),
    /// The dim byte is neither 2D nor 3D; further dimensions are reserved.
    ReservedDim(u8),
    /// The padding byte is not zero.
    NonZeroPadding(u8),
    /// A stage count other than `1`; multi-stage snapshots are reserved.
    ReservedStageCount(u32),
}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated { needed, found } => {
                write!(f, "header needs {needed} bytes, got {found}")
            }
            Self::BadMagic => write!(f, "not a graph-motor snapshot: bad magic"),
            Self::UnsupportedMajor(newer) => newer.fmt(f),
            Self::Geometry(err) => write!(f, "{err}"),
            Self::ReservedDim(v) => write!(f, "dim {v} is reserved and not implemented"),
            Self::NonZeroPadding(v) => write!(f, "header padding byte is {v}, must be 0"),
            Self::ReservedStageCount(n) => write!(f, "stage count {n} is reserved; only 1 is read"),
        }
    }
}

impl SnapshotHeader {
    /// Appends the 28 header bytes.
    pub fn encode(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&MAGIC);
        out.extend_from_slice(&self.version.major.to_le_bytes());
        out.extend_from_slice(&self.version.minor.to_le_bytes());
        out.extend_from_slice(&[
            self.node_kind.tag(),
            self.edge_kind.tag(),
            self.dim.get(),
            0,
        ]);
        out.extend_from_slice(&self.stage_count.get().to_le_bytes());
        out.extend_from_slice(&self.node_count.to_le_bytes());
        out.extend_from_slice(&self.edge_count.to_le_bytes());
    }

    /// Reads a header, refusing a newer major and every reserved field that is set.
    pub fn decode(bytes: &[u8]) -> Result<Self, ReadError> {
        let head = bytes
            .get(..HEADER_LEN as usize)
            .ok_or(ReadError::Truncated {
                needed: HEADER_LEN,
                found: u32::try_from(bytes.len()).unwrap_or(u32::MAX),
            })?;
        if head[..4] != MAGIC {
            return Err(ReadError::BadMagic);
        }
        let version = FormatVersion {
            major: le_u32(head, 4),
            minor: le_u32(head, 8),
        };
        check_readable(version).map_err(ReadError::UnsupportedMajor)?;
        // dim, then padding, then the tags: a reader that cannot express the snapshot's
        // dimension must say so before it says anything about a geometry tag. Both are
        // read here, not in the struct literal below, because a literal evaluates its
        // fields in source order and the tags come first.
        let dim = Dim::try_from(head[14])?;
        check_padding(head[15])?;
        Ok(Self {
            version,
            node_kind: NodeGeometryKind::from_tag(head[12]).map_err(ReadError::Geometry)?,
            edge_kind: EdgeGeometryKind::from_tag(head[13]).map_err(ReadError::Geometry)?,
            dim,
            stage_count: StageCount::try_from(le_u32(head, 16))?,
            node_count: le_u32(head, 20),
            edge_count: le_u32(head, 24),
        })
    }
}

fn check_padding(padding: u8) -> Result<(), ReadError> {
    if padding != 0 {
        return Err(ReadError::NonZeroPadding(padding));
    }
    Ok(())
}

fn le_u32(head: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([head[at], head[at + 1], head[at + 2], head[at + 3]])
}

mod error;

pub use error::SnapshotError;

#[cfg(test)]
mod tests;
