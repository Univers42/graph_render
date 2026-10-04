//! The arena's own tests: interning order and near misses, the reservation, and the
//! two guards `Fnv1a` (F-35) and `handle_at` (F-36) carry.

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
    assert!(sized.text.capacity() >= bytes);
    assert!(sized.spans.capacity() >= values.len());
    assert!(sized.lookup.capacity() >= values.len());
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
    let mut int = Fnv1a::default();
    int.write_u32(0x0102_0304);
    assert_eq!(
        int.finish(),
        hash(&[4, 3, 2, 1]),
        "an integer hashes little-endian"
    );
}

#[test]
fn the_handle_past_usize_max_is_the_capacity_error() {
    let refused = Err(CapacityError {
        what: "string arena",
    });
    assert_eq!(handle_at(usize::MAX), refused);
    assert_eq!(handle_at(u32::MAX as usize), refused);
}
