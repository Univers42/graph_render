//! Every way a columnar ingest document can be refused (`docs/contract/ingest-columns.md`).
//!
//! One variant per check, so a test can name the rule it broke and a host reading only the
//! ABI's single `ColumnsInvalid` code still has one thing to look up.

use core::fmt;

/// Why [`decode`](super::decode) refused a document. At the ABI every one of these is
/// `Code::ColumnsInvalid` (`docs/contract/wasm-abi.md`); the variants are for the tests and
/// for a host that wants to say which rule it broke.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnsError {
    /// The buffer is shorter than the fixed header.
    ShortBuffer {
        /// Bytes the buffer has.
        found: usize,
    },
    /// `header[0]` is not `0x31434D47`.
    BadMagic {
        /// The magic that was there.
        found: u32,
    },
    /// `header[1]` is not `1`. There is no compatibility story: the section table is the
    /// format, so a later version is a different document and this reader declines it.
    BadVersion {
        /// The version that was there.
        found: u32,
    },
    /// `header[6]` or `header[7]` is not `0`.
    NonZeroReserved {
        /// Which of the two reserved words.
        word: u32,
    },
    /// A count is `u32::MAX`, the value the optional columns use to mean "absent". A count
    /// that large is either that marker or a header nobody honest writes.
    CountTooLarge {
        /// Which header word carried it.
        word: &'static str,
    },
    /// The declared sections do not sum to the buffer length. Both directions are refused:
    /// a short buffer is truncated, and a long one has bytes this reader cannot account for
    /// and must not ignore.
    Length {
        /// What the header declared.
        expected: u64,
        /// What the buffer holds.
        found: usize,
    },
    /// A section size did not fit `u64` — impossible with `u32` counts, but checked rather
    /// than assumed, because a wrapped length is the one that reads out of bounds.
    SizeOverflow,
    /// `offsets[0]` is not `0`.
    OffsetOrigin {
        /// The first offset.
        found: u32,
    },
    /// `offsets[i + 1] < offsets[i]`.
    DecreasingOffset {
        /// The entry index.
        index: u32,
    },
    /// `offsets[string_count]` is not `blob_len`.
    OffsetEnd {
        /// The last offset.
        found: u32,
        /// `blob_len` from the header.
        blob_len: u32,
    },
    /// An offset is past the end of the blob.
    OffsetOutOfRange {
        /// The entry index.
        index: u32,
    },
    /// The blob is not UTF-8.
    Utf8 {
        /// Byte offset of the first bad sequence inside the blob.
        at: usize,
    },
    /// A string's byte range does not start and end on a character boundary. The blob is
    /// valid UTF-8 and the slice is still not a `str`: only `str::get` can say so.
    SplitCodePoint {
        /// The string index whose range does.
        index: u32,
    },
    /// A pad byte is not `0`.
    NonZeroPadding {
        /// Byte offset of the offending pad byte.
        at: usize,
    },
    /// A string index is not below `string_count`.
    StringIndex {
        /// The column the index was read from.
        column: &'static str,
        /// The row it was read at.
        row: u32,
    },
    /// A required column holds `u32::MAX`, the absent marker. Only `database_id`, `group`,
    /// `icon` and `record_id` may hold it.
    RequiredAbsent {
        /// The column that held it.
        column: &'static str,
        /// The row it was read at.
        row: u32,
    },
    /// An edge names a node row at or past `node_count`.
    EndpointRow {
        /// The edge row.
        row: u32,
        /// The column that held the row number.
        column: &'static str,
    },
    /// A boolean cell is neither `0` nor `1`.
    NotBoolean {
        /// The column.
        column: &'static str,
        /// The row.
        row: u32,
        /// What was there.
        found: u32,
    },
    /// A float is NaN or infinite. `-0.0` and subnormals are finite and are accepted —
    /// `ingest::read_records` accepts them too, and the differential compares them.
    NotFinite {
        /// The column.
        column: &'static str,
        /// The row.
        row: u32,
    },
}

impl fmt::Display for ColumnsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ShortBuffer { found } => {
                write!(f, "buffer of {found} bytes is shorter than the header")
            }
            Self::BadMagic { found } => write!(f, "magic {found:#x} is not 0x31434d47"),
            Self::BadVersion { found } => write!(f, "version {found} is not 1"),
            Self::NonZeroReserved { word } => write!(f, "reserved header word {word} is not 0"),
            Self::CountTooLarge { word } => write!(f, "{word} is u32::MAX, the absent marker"),
            Self::Length { expected, found } => {
                write!(f, "sections sum to {expected} bytes, buffer holds {found}")
            }
            Self::SizeOverflow => write!(f, "a section size does not fit u64"),
            Self::OffsetOrigin { found } => write!(f, "offsets[0] is {found}, not 0"),
            Self::DecreasingOffset { index } => write!(f, "offsets[{index}] decreases"),
            Self::OffsetEnd { found, blob_len } => {
                write!(f, "last offset {found} is not blob_len {blob_len}")
            }
            Self::OffsetOutOfRange { index } => write!(f, "offsets[{index}] is past the blob"),
            Self::Utf8 { at } => write!(f, "blob is not UTF-8 at byte {at}"),
            Self::SplitCodePoint { index } => write!(f, "string {index} splits a code point"),
            Self::NonZeroPadding { at } => write!(f, "pad byte at {at} is not 0"),
            Self::StringIndex { column, row } => write!(f, "{column}[{row}] names no string"),
            Self::RequiredAbsent { column, row } => {
                write!(f, "{column}[{row}] is u32::MAX in a required column")
            }
            Self::EndpointRow { row, column } => {
                write!(f, "{column}[{row}] names a row past the nodes")
            }
            Self::NotBoolean { column, row, found } => {
                write!(f, "{column}[{row}] is {found}, not 0 or 1")
            }
            Self::NotFinite { column, row } => write!(f, "{column}[{row}] is not finite"),
        }
    }
}
