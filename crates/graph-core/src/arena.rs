//! The string arena: every string the topology holds, stored once.
//!
//! One `String` buffer plus `(offset, len)` spans, both `u32` (D6). Interning collapses
//! repeated values — the same group, kind or database name on ten thousand nodes costs
//! one copy. Handles are opaque [`Interned`] values and never cross the wire: the wire
//! carries the strings themselves.

use core::fmt;
use core::hash::{BuildHasher, BuildHasherDefault, Hasher};
use core::num::NonZeroU32;
use indexmap::IndexMap;
use indexmap::map::RawEntryApiV1;
use indexmap::map::raw_entry_v1::RawEntryMut;

/// Handle to one string in the [`StringArena`] that issued it.
///
/// Stored as `index + 1` in a `NonZeroU32`, so `Option<Interned>` — the nullable
/// columns — costs 4 bytes, not 8.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Interned(NonZeroU32);

impl Interned {
    /// The arena slot this handle names, `0` for the first string interned. Public to
    /// the crate so a build can table slot → dense index; the wire never sees it.
    pub(crate) fn slot(self) -> usize {
        (self.0.get() - 1) as usize
    }
}

/// FNV-1a, 64-bit. A fixed hash, so a map's internal layout — not only its iteration
/// order — is the same on every run and every target (D4). Not collision-resistant;
/// nothing here is adversarial.
///
/// The integer writer is little-endian, as core's default is not: `write_u32` serialises
/// with `to_ne_bytes`, so a `u32` key hashed on a big-endian target lands in a different
/// bucket than the same key on x86_64 or wasm32, and `neighborhood.rs` keys its tables on
/// `u32`. `to_le_bytes` is the same four bytes core already writes on every little-endian
/// target, so no table moves here.
pub struct Fnv1a(u64);

impl Default for Fnv1a {
    fn default() -> Self {
        Self(0xCBF2_9CE4_8422_2325)
    }
}

impl Hasher for Fnv1a {
    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.0 = (self.0 ^ u64::from(byte)).wrapping_mul(0x100_0000_01B3);
        }
    }

    fn write_u32(&mut self, value: u32) {
        self.write(&value.to_le_bytes());
    }

    fn finish(&self) -> u64 {
        self.0
    }
}

/// The hasher every map in graph-core uses.
pub type FixedState = BuildHasherDefault<Fnv1a>;

/// A limit of the `u32` index space was reached. Refused, never wrapped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapacityError {
    /// What overflowed.
    pub what: &'static str,
}

impl fmt::Display for CapacityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} exceeds the u32 index space", self.what)
    }
}

/// Where one interned string sits in the arena's buffer: the pair `spans` holds.
///
/// The lookup map keys on this, not on the handle, so a probe reads the string
/// straight out of the bucket it lands in — one array fewer to chase than a key
/// that would have to be resolved through `spans` first.
///
/// The derived `Hash` is never called: every lookup goes through the raw entry
/// API with the hash of the *string* it is looking for, so the table's layout
/// depends on the strings alone and not on the key type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Span {
    start: u32,
    len: u32,
}

impl Span {
    fn text<'a>(&self, text: &'a str) -> &'a str {
        &text[self.start as usize..(self.start + self.len) as usize]
    }
}

/// Interned strings: one buffer, `u32` spans, and a lookup that is hashed by
/// content and keyed by the span that content sits at.
#[derive(Debug, Default, Clone)]
pub struct StringArena {
    text: String,
    spans: Vec<Span>,
    lookup: IndexMap<Span, (), FixedState>,
}

impl StringArena {
    /// An arena with room for `strings` distinct values totalling `bytes`, so a
    /// caller that already knows the shape of its input pays no rehash.
    ///
    /// Both counts are hints, not a promise: interning past them grows the
    /// buffer exactly as an arena built by [`Default`] would. Reserving changes a
    /// table's internal layout and no hash and no iteration order (D4), so the
    /// two arenas stay byte-identical in what they hand out.
    ///
    /// **Caveat:** `strings` over-reserves by every value the input repeats — one
    /// source name on a million nodes still reserves a million slots — and
    /// `bytes` over-reserves the same way, so a caller passing raw field lengths
    /// for a low-cardinality column holds memory it will not use.
    pub fn with_capacity(strings: usize, bytes: usize) -> Self {
        Self {
            text: String::with_capacity(bytes),
            spans: Vec::with_capacity(strings),
            lookup: IndexMap::with_capacity_and_hasher(strings, FixedState::default()),
        }
    }

    /// Returns the handle for `value`, storing it on first sight.
    pub fn intern(&mut self, value: &str) -> Result<Interned, CapacityError> {
        let hash = FixedState::default().hash_one(value);
        let text = &self.text;
        let entry = self
            .lookup
            .raw_entry_mut_v1()
            .from_hash(hash, |span| span.text(text) == value);
        let slot = match entry {
            RawEntryMut::Occupied(found) => return handle_at(found.index()),
            RawEntryMut::Vacant(slot) => slot,
        };
        let overflow = CapacityError {
            what: "string arena",
        };
        let start = u32::try_from(self.text.len()).map_err(|_| overflow)?;
        let len = u32::try_from(value.len()).map_err(|_| overflow)?;
        start.checked_add(len).ok_or(overflow)?;
        // The map appends exactly where `spans` does, so the entry's index is the
        // slot its handle names — `find` reads the handle back out of the index.
        debug_assert_eq!(slot.index(), self.spans.len());
        let handle = handle_at(slot.index())?;
        let span = Span { start, len };
        self.text.push_str(value);
        self.spans.push(span);
        slot.insert_hashed_nocheck(hash, span, ());
        Ok(handle)
    }

    /// The handle for `value` if it was interned; never stores anything.
    pub fn find(&self, value: &str) -> Option<Interned> {
        let hash = FixedState::default().hash_one(value);
        let text = &self.text;
        let index = self
            .lookup
            .raw_entry_v1()
            .index_from_hash(hash, |span| span.text(text) == value)?;
        handle_at(index).ok()
    }

    /// The string behind `handle`. `handle` must come from this arena.
    pub fn get(&self, handle: Interned) -> &str {
        resolve(&self.text, &self.spans, handle)
    }

    /// Distinct strings held.
    pub fn len(&self) -> usize {
        self.spans.len()
    }

    /// True when nothing has been interned.
    pub fn is_empty(&self) -> bool {
        self.spans.is_empty()
    }

    /// Bytes of string data held — the data-dependent half of the memory budget (§5.1).
    pub fn byte_len(&self) -> usize {
        self.text.len()
    }
}

/// The handle for the string interned at map position `index`. Interning only ever
/// appends, so the map's insertion order and `spans` are the same order and a
/// position names its handle (D4).
///
/// `index + 1` is checked, not wrapped: `usize::MAX` is a position a `Vec` cannot hold, so
/// the add would overflow-panic in a debug build on wasm32 before the `try_from` could
/// refuse it.
fn handle_at(index: usize) -> Result<Interned, CapacityError> {
    let overflow = CapacityError {
        what: "string arena",
    };
    index
        .checked_add(1)
        .and_then(|slot| u32::try_from(slot).ok())
        .and_then(NonZeroU32::new)
        .map(Interned)
        .ok_or(overflow)
}

fn resolve<'a>(text: &'a str, spans: &[Span], handle: Interned) -> &'a str {
    spans[handle.slot()].text(text)
}

#[cfg(test)]
mod tests;
