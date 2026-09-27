//! The snapshot header and the column writer — the bytes the hash is taken over.
//!
//! Binary layout, little-endian, no implicit padding (Phase 0; `docs/contract/binary-layout.md`
//! becomes authoritative in Phase 2):
//!
//! | offset | size | field                                            |
//! |-------:|-----:|--------------------------------------------------|
//! |      0 |    4 | magic `b"GMSN"`                                  |
//! |      4 |    4 | format major, `u32`                              |
//! |      8 |    4 | format minor, `u32`                              |
//! |     12 |    1 | node geometry tag (`NodeGeometryKind`)           |
//! |     13 |    1 | edge geometry tag (`EdgeGeometryKind`)           |
//! |     14 |    1 | z channel: `0` absent; `1` reserved, refused     |
//! |     15 |    1 | padding, must be `0`                             |
//! |     16 |    4 | stage count, `u32`; reserved, must be `1` for now |
//! |     20 |    4 | node count, `u32`                                |
//! |     24 |    4 | edge count, `u32`                                |
//! |     28 |    … | columns, each `count × f32` little-endian        |
//!
//! Every integer on the wire is `u32` or a single tag byte — never `usize` (D6).

use crate::geometry::{EdgeGeometryKind, NodeGeometryKind, TagError};
use core::fmt;

/// The four bytes every snapshot starts with.
pub const MAGIC: [u8; 4] = *b"GMSN";
/// Length of the fixed header in bytes.
pub const HEADER_LEN: u32 = 28;

/// A snapshot format version. A reader refuses any major above the one it knows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    feature = "codegen",
    derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)
)]
pub struct FormatVersion {
    /// Incremented on any change an older reader would misread.
    pub major: u32,
    /// Incremented on additive changes an older reader can safely ignore.
    pub minor: u32,
}

/// The version this crate writes and the highest major it reads. `0.x` is pre-release.
pub const CURRENT_VERSION: FormatVersion = FormatVersion { major: 0, minor: 1 };

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
    pub stage_count: u32,
    /// Number of nodes, and the length of every node column.
    pub node_count: u32,
    /// Number of edges.
    pub edge_count: u32,
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
    /// Written by a newer major version than this reader knows.
    UnsupportedMajor {
        /// Major version found in the payload.
        found: u32,
        /// Highest major version this reader knows.
        known: u32,
    },
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
            Self::UnsupportedMajor { found, known } => {
                write!(f, "format major {found} > known {known}")
            }
            Self::Geometry(err) => write!(f, "{err}"),
            Self::ReservedZChannel(v) => write!(f, "z channel {v} is reserved and not implemented"),
            Self::NonZeroPadding(v) => write!(f, "header padding byte is {v}, must be 0"),
            Self::ReservedStageCount(n) => write!(f, "stage count {n} is reserved; only 1 is read"),
        }
    }
}

/// A column held a NaN or an infinity, which may not be hashed (D9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NonFinite {
    /// Index of the first non-finite value in the column.
    pub index: u32,
}

impl fmt::Display for NonFinite {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "non-finite value at column index {}", self.index)
    }
}

impl SnapshotHeader {
    /// Appends the 28 header bytes.
    pub fn encode(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&MAGIC);
        out.extend_from_slice(&self.version.major.to_le_bytes());
        out.extend_from_slice(&self.version.minor.to_le_bytes());
        out.extend_from_slice(&[self.node_kind.tag(), self.edge_kind.tag(), 0, 0]);
        out.extend_from_slice(&self.stage_count.to_le_bytes());
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
        if version.major > CURRENT_VERSION.major {
            return Err(ReadError::UnsupportedMajor {
                found: version.major,
                known: CURRENT_VERSION.major,
            });
        }
        check_reserved(head[14], head[15], le_u32(head, 16))?;
        Ok(Self {
            version,
            node_kind: NodeGeometryKind::from_tag(head[12]).map_err(ReadError::Geometry)?,
            edge_kind: EdgeGeometryKind::from_tag(head[13]).map_err(ReadError::Geometry)?,
            stage_count: le_u32(head, 16),
            node_count: le_u32(head, 20),
            edge_count: le_u32(head, 24),
        })
    }
}

fn check_reserved(z_channel: u8, padding: u8, stage_count: u32) -> Result<(), ReadError> {
    if z_channel != 0 {
        return Err(ReadError::ReservedZChannel(z_channel));
    }
    if padding != 0 {
        return Err(ReadError::NonZeroPadding(padding));
    }
    if stage_count != 1 {
        return Err(ReadError::ReservedStageCount(stage_count));
    }
    Ok(())
}

fn le_u32(head: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([head[at], head[at + 1], head[at + 2], head[at + 3]])
}

/// Appends one `f32` column little-endian, refusing NaN and infinities (D9).
///
/// Checks the whole column before writing any of it, so a refused column leaves
/// `out` untouched rather than half-written.
pub fn push_f32_column(column: &[f32], out: &mut Vec<u8>) -> Result<(), NonFinite> {
    if let Some(bad) = column.iter().position(|v| !v.is_finite()) {
        return Err(NonFinite {
            index: u32::try_from(bad).unwrap_or(u32::MAX),
        });
    }
    for value in column {
        out.extend_from_slice(&value.to_le_bytes());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::ReadError::*;
    use super::*;

    const HEADER: SnapshotHeader = SnapshotHeader {
        version: CURRENT_VERSION,
        node_kind: NodeGeometryKind::Circle,
        edge_kind: EdgeGeometryKind::Polyline,
        stage_count: 1,
        node_count: 7,
        edge_count: 3,
    };

    fn read(
        h: SnapshotHeader,
        patch: impl FnOnce(&mut Vec<u8>),
    ) -> Result<SnapshotHeader, ReadError> {
        let mut bytes = Vec::new();
        h.encode(&mut bytes);
        patch(&mut bytes);
        SnapshotHeader::decode(&bytes)
    }

    #[test]
    fn header_is_exactly_header_len_bytes_and_round_trips() {
        assert_eq!(
            read(HEADER, |b| assert_eq!(b.len(), HEADER_LEN as usize)),
            Ok(HEADER)
        );
    }

    #[test]
    fn reader_refuses_a_major_above_the_one_it_knows() {
        let mut newer = HEADER;
        newer.version.major = CURRENT_VERSION.major + 1;
        let known = CURRENT_VERSION.major;
        assert_eq!(
            read(newer, |_| ()),
            Err(UnsupportedMajor {
                found: known + 1,
                known
            })
        );
    }

    #[test]
    fn reader_accepts_a_newer_minor() {
        let mut newer = HEADER;
        newer.version.minor = CURRENT_VERSION.minor + 5;
        assert_eq!(read(newer, |_| ()), Ok(newer));
    }

    #[test]
    fn reader_refuses_reserved_fields_and_short_input() {
        let arc = Err(Geometry(TagError::Reserved(crate::geometry::ARC_TAG)));
        assert_eq!(read(HEADER, |b| b[13] = crate::geometry::ARC_TAG), arc);
        assert_eq!(read(HEADER, |b| b[14] = 1), Err(ReservedZChannel(1)));
        assert_eq!(read(HEADER, |b| b[15] = 9), Err(NonZeroPadding(9)));
        assert_eq!(read(HEADER, |b| b[16] = 2), Err(ReservedStageCount(2)));
        assert_eq!(
            read(HEADER, |b| b.truncate(5)),
            Err(Truncated {
                needed: 28,
                found: 5
            })
        );
        assert_eq!(read(HEADER, |b| b[0] = b'X'), Err(BadMagic));
    }

    #[test]
    fn column_writer_refuses_nan_and_infinity_and_writes_nothing() {
        let mut out = Vec::new();
        assert_eq!(
            push_f32_column(&[1.0, f32::NAN], &mut out),
            Err(NonFinite { index: 1 })
        );
        assert_eq!(
            push_f32_column(&[f32::NEG_INFINITY], &mut out),
            Err(NonFinite { index: 0 })
        );
        assert!(out.is_empty());
    }

    #[test]
    fn column_writer_emits_little_endian_bytes() {
        let mut out = Vec::new();
        assert_eq!(push_f32_column(&[1.0, -2.5], &mut out), Ok(()));
        assert_eq!(out, [0, 0, 0x80, 0x3f, 0, 0, 0x20, 0xc0]);
    }
}
