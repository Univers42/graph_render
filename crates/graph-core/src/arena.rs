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
    fn slot(self) -> usize {
        (self.0.get() - 1) as usize
    }
}

/// FNV-1a, 64-bit. A fixed hash, so a map's internal layout — not only its iteration
/// order — is the same on every run and every target (D4). Not collision-resistant;
/// nothing here is adversarial.
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

/// Interned strings: one buffer, `u32` spans, and a lookup keyed by content.
#[derive(Debug, Default, Clone)]
pub struct StringArena {
    text: String,
    spans: Vec<(u32, u32)>,
    lookup: IndexMap<Interned, (), FixedState>,
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
        let (text, spans) = (&self.text, &self.spans);
        let entry = self
            .lookup
            .raw_entry_mut_v1()
            .from_hash(hash, |&k| resolve(text, spans, k) == value);
        let slot = match entry {
            RawEntryMut::Occupied(found) => return Ok(*found.key()),
            RawEntryMut::Vacant(slot) => slot,
        };
        let overflow = CapacityError {
            what: "string arena",
        };
        let start = u32::try_from(self.text.len()).map_err(|_| overflow)?;
        let len = u32::try_from(value.len()).map_err(|_| overflow)?;
        start.checked_add(len).ok_or(overflow)?;
        let next = u32::try_from(self.spans.len() + 1).map_err(|_| overflow)?;
        let handle = Interned(NonZeroU32::new(next).ok_or(overflow)?);
        self.text.push_str(value);
        self.spans.push((start, len));
        slot.insert_hashed_nocheck(hash, handle, ());
        Ok(handle)
    }

    /// The handle for `value` if it was interned; never stores anything.
    pub fn find(&self, value: &str) -> Option<Interned> {
        let hash = FixedState::default().hash_one(value);
        self.lookup
            .raw_entry_v1()
            .from_hash(hash, |&k| self.get(k) == value)
            .map(|(&k, ())| k)
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

fn resolve<'a>(text: &'a str, spans: &[(u32, u32)], handle: Interned) -> &'a str {
    let (start, len) = spans[handle.slot()];
    &text[start as usize..(start + len) as usize]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interning_collapses_repeats_and_keeps_first_seen_order() {
        let mut arena = StringArena::default();
        let a = arena.intern("group-a").expect("fits");
        let b = arena.intern("").expect("fits");
        assert_eq!(arena.intern("group-a"), Ok(a));
        assert_ne!(a, b);
        assert_eq!((arena.get(a), arena.get(b)), ("group-a", ""));
        assert_eq!((arena.len(), arena.byte_len()), (2, 7));
        assert!(!arena.is_empty() && StringArena::default().is_empty());
        assert_eq!(size_of::<Option<Interned>>(), 4);
    }

    #[test]
    fn find_never_stores_and_tells_near_misses_apart() {
        let mut arena = StringArena::default();
        let note = arena.intern("note:1").expect("fits");
        assert_eq!(arena.find("note:1"), Some(note));
        assert_eq!(arena.find("NOTE:1"), None);
        assert_eq!(arena.find("note:"), None);
        assert_eq!(arena.len(), 1);
    }

    #[test]
    fn with_capacity_holds_exactly_what_a_default_arena_holds() {
        let values = ["group-a", "", "group-b", "group-a", "label:1"];
        let bytes: usize = values.iter().map(|v| v.len()).sum();
        let mut sized = StringArena::with_capacity(values.len(), bytes);
        let mut grown = StringArena::default();
        assert!(sized.is_empty() && sized.find("group-a").is_none());
        let handles: Vec<_> = values
            .iter()
            .map(|v| {
                let (a, b) = (sized.intern(v), grown.intern(v));
                assert_eq!(a, b, "same handle for {v:?}");
                a
            })
            .collect();
        for (handle, value) in handles.iter().zip(&values) {
            assert_eq!(sized.get(handle.expect("fits")), *value);
        }
        assert_eq!(sized.len(), 4);
        assert_eq!(sized.len(), grown.len());
        assert_eq!(sized.byte_len(), grown.byte_len());
        assert_eq!(sized.find("group-a"), Some(handles[0].expect("fits")));
        assert_eq!(sized.find("nope"), None);
    }

    #[test]
    fn a_capacity_error_names_what_overflowed() {
        let err = CapacityError {
            what: "string arena",
        };
        let text = "string arena exceeds the u32 index space";
        assert_eq!(err.to_string(), text);
        assert_eq!(crate::StageError::Capacity(err).to_string(), text);
    }

    #[test]
    fn fnv1a_matches_the_published_64_bit_vectors() {
        let hash = |bytes: &[u8]| {
            let mut h = Fnv1a::default();
            h.write(bytes);
            h.finish()
        };
        assert_eq!(hash(b""), 0xCBF2_9CE4_8422_2325);
        assert_eq!(hash(b"a"), 0xAF63_DC4C_8601_EC8C);
        assert_eq!(hash(b"foobar"), 0x8594_4171_F739_67E8);
    }
}
