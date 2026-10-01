//! How many dimensions a snapshot carries, and the one rule that labels it.
//!
//! `dim` is header byte 14, once per snapshot (`super::SnapshotHeader`), never once per
//! node: a z column that came and went per node would leave the node section's length
//! unreadable from the header alone. `0` is 2D and carries no z column; `1` is 3D and
//! does. Edge paths stay 2D either way.
//!
//! The version a snapshot is labelled with is the **lowest that can express it**, and
//! [`label_for`] is the only place that rule is written. A `dim = 0` snapshot keeps its
//! 0.3 label, so raising [`CURRENT_VERSION`](crate::version::CURRENT_VERSION) to 0.4
//! moves no 2D byte at all: byte 8 is the version and no 2D producer writes it any more.
//! Producers call [`label_for`], never `CURRENT_VERSION`
//! (`docs/decisions/contract-3d-verdict.md` condition 1).

use super::ReadError;
use crate::version::FormatVersion;

/// The first format minor whose header carries a `dim` other than 0, and so whose JSON
/// face spells `"dim"`.
pub const DIM_SINCE_MINOR: u32 = 4;

/// A snapshot's dimension, header byte 14: `0` two-dimensional, `1` three-dimensional.
/// `2..=255` is reserved and refused, so a reader never has to guess what a column it
/// does not know would have meant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(
    feature = "codegen",
    derive(serde::Serialize, serde::Deserialize),
    serde(try_from = "u8", into = "u8")
)]
pub struct Dim(u8);

impl Default for Dim {
    /// 2D: what a document that names no dimension is read as, the same way a document
    /// that names no notes is read as carrying none.
    fn default() -> Self {
        Self::D2
    }
}

impl Dim {
    /// 2D: `x` and `y` only, no z column.
    pub const D2: Self = Self(0);
    /// 3D: `x`, `y` and `z`.
    pub const D3: Self = Self(1);

    /// The value as it goes on the wire.
    pub const fn get(self) -> u8 {
        self.0
    }

    /// Whether a snapshot of this dimension carries a z column.
    pub const fn is_3d(self) -> bool {
        self.0 != Self::D2.0
    }
}

impl TryFrom<u8> for Dim {
    type Error = ReadError;

    fn try_from(value: u8) -> Result<Self, ReadError> {
        match value {
            0 => Ok(Self::D2),
            1 => Ok(Self::D3),
            other => Err(ReadError::ReservedDim(other)),
        }
    }
}

impl From<Dim> for u8 {
    fn from(dim: Dim) -> u8 {
        dim.0
    }
}

/// `true` when `version` is 0.4 or later: the versions whose header can carry a nonzero
/// `dim` and whose JSON face spells `"dim"`. The mirror of `notes::carries_notes`.
pub const fn carries_dim(version: FormatVersion) -> bool {
    version.major == 0 && version.minor >= DIM_SINCE_MINOR
}

/// The version a snapshot of `dim` is labelled with: the lowest that can express it. A 2D
/// snapshot is expressible in 0.3, so it is labelled 0.3 and its bytes — byte 8 included
/// — are byte-for-byte what they were before 3D existed. Only 3D needs 0.4.
pub const fn label_for(dim: Dim) -> FormatVersion {
    FormatVersion {
        major: 0,
        minor: match dim {
            Dim::D2 => 3,
            _ => DIM_SINCE_MINOR,
        },
    }
}

#[cfg(feature = "codegen")]
/// The JSON Schema of a `dim`: an integer, `0` or `1`, defaulting to `0` so a document
/// that omits it reads as 2D. JSON Schema cannot tie the header's `dim` to a z column
/// being present, so the rule is stated in prose and the reader enforces it.
pub fn dim_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
    schemars::json_schema!({
        "type": "integer",
        "format": "uint8",
        "default": 0,
        "enum": [0, 1]
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::version::{CURRENT_VERSION, UNVERSIONED};

    #[test]
    fn a_2d_snapshot_keeps_its_0_3_label_so_no_2d_byte_moves() {
        assert_eq!(label_for(Dim::D2).to_string(), "0.3");
        assert_eq!(label_for(Dim::D3).to_string(), "0.4");
        assert_eq!(
            (label_for(Dim::D2).minor, label_for(Dim::D3).minor),
            (3, CURRENT_VERSION.minor),
            "only 3D is labelled with what the crate writes; 2D stays below it"
        );
        assert_eq!(label_for(Dim::D2).minor, 3, "the label 0.3 predates dim");
        assert!(
            !carries_dim(label_for(Dim::D2)),
            "and 0.3 carries no dim at all"
        );
        assert!(carries_dim(label_for(Dim::D3)));
    }

    #[test]
    fn only_zero_and_one_are_dimensions_a_reader_understands() {
        for value in 0..=u8::MAX {
            match Dim::try_from(value) {
                Ok(dim) => {
                    assert!(value < 2, "{value} is read");
                    assert_eq!(u8::from(dim), value, "{value} round-trips");
                }
                Err(ReadError::ReservedDim(refused)) => assert_eq!(refused, value),
                Err(other) => panic!("{value} refused as {other:?}, not as a reserved dim"),
            }
        }
        assert!(!Dim::D2.is_3d());
        assert!(Dim::D3.is_3d());
    }

    #[test]
    fn dim_is_carried_from_the_minor_that_introduced_it() {
        let at = |minor| FormatVersion { major: 0, minor };
        assert!(!carries_dim(UNVERSIONED));
        assert!(!carries_dim(at(2)));
        assert!(
            !carries_dim(at(3)),
            "0.3 is 2D only: byte 14 is always 0 there"
        );
        assert!(carries_dim(at(4)));
        assert!(carries_dim(at(5)));
        assert!(!carries_dim(FormatVersion { major: 1, minor: 9 }));
    }
}
