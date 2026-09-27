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
//! |     14 |    1 | z channel: `0` absent; `1` reserved, refused      |
//! |     15 |    1 | padding, must be `0`                              |
//! |     16 |    4 | stage count, `u32`; reserved, must be `1` for now |
//! |     20 |    4 | node count, `u32`                                 |
//! |     24 |    4 | edge count, `u32`                                 |
//!
//! Every integer on the wire is `u32` or a single tag byte — never `usize` (D6).

use crate::geometry::{EdgeGeometryKind, NodeGeometryKind, TagError};
use crate::version::{FormatVersion, NewerMajor, check_readable};
use core::fmt;

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
    /// The reserved z channel is set; z is allocated but not implemented.
    ReservedZChannel(u8),
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
            Self::ReservedZChannel(v) => write!(f, "z channel {v} is reserved and not implemented"),
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
        out.extend_from_slice(&[self.node_kind.tag(), self.edge_kind.tag(), 0, 0]);
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
        check_reserved(head[14], head[15])?;
        Ok(Self {
            version,
            node_kind: NodeGeometryKind::from_tag(head[12]).map_err(ReadError::Geometry)?,
            edge_kind: EdgeGeometryKind::from_tag(head[13]).map_err(ReadError::Geometry)?,
            stage_count: StageCount::try_from(le_u32(head, 16))?,
            node_count: le_u32(head, 20),
            edge_count: le_u32(head, 24),
        })
    }
}

fn check_reserved(z_channel: u8, padding: u8) -> Result<(), ReadError> {
    if z_channel != 0 {
        return Err(ReadError::ReservedZChannel(z_channel));
    }
    if padding != 0 {
        return Err(ReadError::NonZeroPadding(padding));
    }
    Ok(())
}

fn le_u32(head: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([head[at], head[at + 1], head[at + 2], head[at + 3]])
}

/// Why a snapshot was refused, by either face or at construction. Every variant names
/// the column (`node.id`, `edge.source`, `node.x`, `edge.pts`, …) and, where there is one,
/// the position that broke the rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotError {
    /// The fixed header was refused.
    Header(ReadError),
    /// The payload ends inside a column the header's counts call for.
    Truncated {
        /// The column that was cut short.
        column: &'static str,
    },
    /// Bytes remain after the last column.
    TrailingBytes {
        /// How many.
        count: u64,
    },
    /// A column's length is not the one the counts require.
    Length {
        /// The column.
        column: &'static str,
        /// Values it must hold.
        expected: u64,
        /// Values it holds.
        found: u64,
    },
    /// NaN or ±∞, which may not be hashed (D9).
    NonFinite {
        /// The column.
        column: &'static str,
        /// The first bad position.
        index: u32,
    },
    /// A negative radius, width or height.
    Negative {
        /// The column.
        column: &'static str,
        /// The first bad position.
        index: u32,
    },
    /// Offsets that do not start at 0, decrease, or run past their data.
    Offsets {
        /// The column.
        column: &'static str,
        /// The first bad offset.
        index: u32,
    },
    /// An id whose bytes are not UTF-8.
    Utf8 {
        /// The column.
        column: &'static str,
        /// The id's position.
        index: u32,
    },
    /// A padding byte that is not 0.
    Padding {
        /// The column the padding closes.
        column: &'static str,
    },
    /// An id equal to an earlier one in the same table.
    DuplicateId {
        /// The column.
        column: &'static str,
        /// The later of the two positions.
        index: u32,
    },
    /// An edge endpoint that is not a node of this snapshot.
    Endpoint {
        /// `edge.source` or `edge.target`.
        column: &'static str,
        /// The edge.
        index: u32,
    },
    /// A curve of degree 0.
    CurveDegree,
    /// More ids, id bytes or points than a `u32` counts.
    Capacity {
        /// The column.
        column: &'static str,
    },
}

impl fmt::Display for SnapshotError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Header(err) => err.fmt(f),
            Self::Truncated { column } => write!(f, "{column}: the snapshot ends inside it"),
            Self::TrailingBytes { count } => write!(f, "{count} bytes after the last column"),
            Self::Length {
                column,
                expected,
                found,
            } => write!(f, "{column}: {found} values, need {expected}"),
            Self::NonFinite { column, index } => write!(f, "{column}[{index}]: NaN or infinite"),
            Self::Negative { column, index } => write!(f, "{column}[{index}]: negative"),
            Self::Offsets { column, index } => {
                write!(
                    f,
                    "{column}[{index}]: offsets must start at 0, never decrease and end at the data's end"
                )
            }
            Self::Utf8 { column, index } => write!(f, "{column}[{index}]: not UTF-8"),
            Self::Padding { column } => write!(f, "{column}: padding bytes must be 0"),
            Self::DuplicateId { column, index } => {
                write!(f, "{column}[{index}]: repeats an earlier id")
            }
            Self::Endpoint { column, index } => write!(f, "{column}[{index}]: not a node"),
            Self::CurveDegree => write!(f, "edge.degree: a curve needs degree 1 or more"),
            Self::Capacity { column } => write!(f, "{column}: more than a u32 can count"),
        }
    }
}

#[cfg(test)]
mod tests;
