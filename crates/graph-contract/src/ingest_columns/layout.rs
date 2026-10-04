//! The section table: where each column sits, and the walk that refuses a document whose
//! bytes do not add up.
//!
//! Sizes are computed in checked `u64` from the `u32` header words and are compared to the
//! buffer length **before** a single one of them is used as a slice bound. Nothing here
//! allocates, so "no allocation is sized from a header field" is not a promise about this
//! function — it is simply that there is none.

use super::Columns;
use super::error::ColumnsError;

/// `0x31434D47`, the four bytes `"GMC1"` read as a little-endian `u32`.
pub(super) const MAGIC: u32 = 0x3143_4D47;
/// The only version this reader speaks.
pub(super) const VERSION: u32 = 1;
/// `header` is eight `u32` words.
pub(super) const HEADER_BYTES: u64 = 32;
/// The `u32` string index that means "this optional field is absent".
pub(super) const ABSENT: u32 = u32::MAX;

/// The four `u64` words the exact-size check needs: node rows, edge rows, string entries,
/// blob bytes. All four come from the `u32` header, widened — never narrowed back.
#[derive(Debug, Clone, Copy)]
pub(super) struct Shape {
    /// Node rows.
    pub nodes: u64,
    /// Edge rows.
    pub edges: u64,
    /// String entries, plus one for the closing offset.
    pub strings: u64,
    /// Blob bytes.
    pub blob: u64,
}

impl Shape {
    /// The exact total the header declares, and the zero pad that precedes the columns.
    /// Section order: offsets, blob, pad, then `node weight`, `node version`,
    /// `edge strength`, then the eight node `u32` columns and the eight edge ones.
    fn declared(&self) -> Result<(u64, u64), ColumnsError> {
        let Self {
            nodes,
            edges,
            strings,
            blob,
        } = *self;
        let table = HEADER_BYTES
            .checked_add(strings.checked_add(1).ok_or(ColumnsError::SizeOverflow)? * 4)
            .and_then(|n| n.checked_add(blob))
            .ok_or(ColumnsError::SizeOverflow)?;
        // The pad is whole so the first column starts on an 8-byte boundary from the buffer
        // start: the f64 columns are then 8-aligned too, which reading them as `&[f64]`
        // would need `unsafe` to assert. Nothing here casts a pointer; every float is read
        // back from its eight little-endian bytes.
        let pad = (8 - table % 8) % 8;
        let wide = nodes.checked_mul(16).ok_or(ColumnsError::SizeOverflow)?;
        let strength = edges.checked_mul(8).ok_or(ColumnsError::SizeOverflow)?;
        let narrow = nodes.checked_add(edges).ok_or(ColumnsError::SizeOverflow)?;
        let u32s = narrow.checked_mul(32).ok_or(ColumnsError::SizeOverflow)?;
        table
            .checked_add(pad)
            .and_then(|n| n.checked_add(wide))
            .and_then(|n| n.checked_add(strength))
            .and_then(|n| n.checked_add(u32s))
            .map(|total| (total, pad))
            .ok_or(ColumnsError::SizeOverflow)
    }
}

/// Every section's byte range, borrowed from the one buffer: the offsets, the UTF-8 blob,
/// and the nineteen columns in contract order.
pub(super) struct Layout<'a> {
    /// The `string_count + 1` offsets, as raw little-endian bytes.
    pub offsets: &'a [u8],
    /// The blob, UTF-8 checked.
    pub blob: &'a str,
    /// The columns.
    pub columns: Columns<'a>,
}

/// Every section's byte range, and the refusal if they do not tile the buffer exactly.
pub(super) fn layout<'a>(bytes: &'a [u8], shape: Shape) -> Result<Layout<'a>, ColumnsError> {
    let (expected, pad) = shape.declared()?;
    if expected != bytes.len() as u64 {
        return Err(ColumnsError::Length {
            expected,
            found: bytes.len(),
        });
    }
    let offsets = take(bytes, HEADER_BYTES, (shape.strings + 1) * 4)?;
    let blob_at = HEADER_BYTES + (shape.strings + 1) * 4;
    let blob_bytes = take(bytes, blob_at, shape.blob)?;
    let columns_at = blob_at + shape.blob + pad;
    let pad_at = (blob_at + shape.blob) as usize;
    check_padding(&bytes[pad_at..columns_at as usize], pad_at)?;
    let blob = core::str::from_utf8(blob_bytes).map_err(|e| ColumnsError::Utf8 {
        at: e.valid_up_to(),
    })?;
    Ok(Layout {
        offsets,
        blob,
        columns: columns(bytes, columns_at, shape),
    })
}

/// The nineteen columns, in the order `docs/contract/ingest-columns.md` lists them.
///
/// The field-initializer order of a struct literal is the written order, so the `at` cursor
/// inside the macro walks the sections in exactly that order.
fn columns<'a>(bytes: &'a [u8], at: u64, shape: Shape) -> Columns<'a> {
    macro_rules! split {
        ($($name:ident = $len:expr),+ $(,)?) => {{
            let mut at = at;
            let columns = Columns {
                $( $name: { let s = take(bytes, at, $len).expect("the exact-size check proved this"); at += $len; s }, )+
            };
            (columns, at)
        }};
    }
    let node4 = shape.nodes * 4;
    let edge4 = shape.edges * 4;
    let (columns, end) = split! {
        weight = shape.nodes * 8,
        version = shape.nodes * 8,
        strength = shape.edges * 8,
        id = node4,
        kind = node4,
        database = node4,
        source = node4,
        label = node4,
        group = node4,
        icon = node4,
        has_note = node4,
        edge_id = edge4,
        edge_source = edge4,
        edge_target = edge4,
        edge_kind = edge4,
        edge_label = edge4,
        record_id = edge4,
        directed = edge4,
        child_first = edge4,
    };
    // `end` is the end of the buffer, which the exact-size check already proved equals the
    // declared total; reading it again would only re-derive what `declared()` asserted.
    debug_assert_eq!(end as usize, bytes.len());
    columns
}

/// `bytes[from .. from + len]`, both ends computed in checked `usize`. The exact-size check
/// has already proved this range is inside the buffer, so a miss here means that check and
/// this arithmetic disagree — reported as a length mismatch, never a panic.
fn take(bytes: &[u8], at: u64, len: u64) -> Result<&[u8], ColumnsError> {
    let from = usize::try_from(at).map_err(|_| ColumnsError::SizeOverflow)?;
    let to = from
        .checked_add(usize::try_from(len).map_err(|_| ColumnsError::SizeOverflow)?)
        .ok_or(ColumnsError::SizeOverflow)?;
    bytes.get(from..to).ok_or(ColumnsError::Length {
        expected: to as u64,
        found: bytes.len(),
    })
}

/// Every pad byte is zero.
fn check_padding(pad: &[u8], at: usize) -> Result<(), ColumnsError> {
    match pad.iter().position(|&b| b != 0) {
        Some(i) => Err(ColumnsError::NonZeroPadding { at: at + i }),
        None => Ok(()),
    }
}

/// The eight header words, read little-endian, each one refused before the next is trusted.
pub(super) fn header(bytes: &[u8]) -> Result<Shape, ColumnsError> {
    let head: &[u8; 32] = bytes
        .get(..32)
        .and_then(|b| <&[u8; 32]>::try_from(b).ok())
        .ok_or(ColumnsError::ShortBuffer { found: bytes.len() })?;
    let word = |at: usize| u32::from_le_bytes(head[at..at + 4].try_into().expect("4 bytes"));
    if word(0) != MAGIC {
        return Err(ColumnsError::BadMagic { found: word(0) });
    }
    if word(4) != VERSION {
        return Err(ColumnsError::BadVersion { found: word(4) });
    }
    if word(24) != 0 || word(28) != 0 {
        return Err(ColumnsError::NonZeroReserved {
            word: word(24) | word(28),
        });
    }
    let counts = [
        ("node_count", word(8)),
        ("edge_count", word(12)),
        ("string_count", word(16)),
    ];
    if let Some(&(word, _)) = counts.iter().find(|&&(_, count)| count == ABSENT) {
        return Err(ColumnsError::CountTooLarge { word });
    }
    Ok(Shape {
        nodes: u64::from(word(8)),
        edges: u64::from(word(12)),
        strings: u64::from(word(16)),
        blob: u64::from(word(20)),
    })
}
