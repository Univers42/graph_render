use super::*;
use crate::rng::Mt19937;

/// Every row of `grown` equals the same row of a `Csr` built from `pairs` in one pass.
fn assert_rows_match(grown: &AppendCsr, rows: u32, pairs: &[(u32, u32)]) {
    let frozen = Csr::from_pairs(rows, pairs.iter().copied()).expect("fits");
    assert_eq!((grown.rows(), grown.len()), (frozen.rows(), frozen.len()));
    assert_eq!(grown.is_empty(), frozen.is_empty());
    for r in 0..rows {
        assert_eq!(grown.row(r), frozen.row(r), "row {r}");
    }
}

/// `count` pairs over `rows` rows, drawn from `seed`.
fn random_pairs(seed: u32, rows: u32, count: usize) -> Vec<(u32, u32)> {
    let mut rng = Mt19937::new(seed);
    (0..count)
        .map(|_| (rng.next_u32() % rows, rng.next_u32()))
        .collect()
}

#[test]
fn a_frozen_build_is_the_csr_byte_for_byte() {
    let pairs = random_pairs(7, 50, 400);
    let grown = AppendCsr::from_pairs(50, pairs.iter().copied()).expect("fits");
    let frozen = Csr::from_pairs(50, pairs.iter().copied()).expect("fits");
    assert_eq!(grown.values, frozen.values);
    assert!(grown.spans.iter().all(|s| s.len == s.cap));
    assert_rows_match(&grown, 50, &pairs);
    assert_eq!(grown.byte_len(), 50 * 12 + 400 * 4);
}

#[test]
fn random_interleaved_appends_match_a_one_pass_build() {
    for seed in 0..8 {
        let pairs = random_pairs(seed, 37, 600);
        let (built, streamed) = pairs.split_at(150);
        let mut grown = AppendCsr::from_pairs(37, built.iter().copied()).expect("fits");
        for &(row, value) in streamed {
            grown.append(row, value).expect("fits");
        }
        assert_rows_match(&grown, 37, &pairs);
    }
}

#[test]
fn pushed_rows_grow_like_rows_built_up_front() {
    let pairs = random_pairs(3, 20, 300);
    let mut grown = AppendCsr::default();
    for _ in 0..20 {
        grown.push_row().expect("fits");
    }
    for &(row, value) in &pairs {
        grown.append(row, value).expect("fits");
    }
    assert_rows_match(&grown, 20, &pairs);
}

#[test]
fn a_full_row_inside_the_buffer_moves_to_the_tail() {
    let mut grown = AppendCsr::from_pairs(3, [(0, 1), (1, 2), (2, 3)].into_iter()).expect("fits");
    grown.append(0, 9).expect("fits");
    let span = grown.spans[0];
    assert_eq!((span.start, span.len, span.cap), (3, 2, 4));
    assert_eq!(grown.row(0), [1, 9]);
    grown.append(0, 8).expect("fits");
    assert_eq!(grown.spans[0].start, 3, "room left: written in place");
    grown.append(2, 4).expect("fits");
    assert_eq!(grown.row(2), [3, 4]);
    assert_rows_match(&grown, 3, &[(0, 1), (1, 2), (2, 3), (0, 9), (0, 8), (2, 4)]);
}

#[test]
fn the_tail_row_pushes_without_moving() {
    let mut grown = AppendCsr::from_pairs(2, [(0, 1), (1, 2)].into_iter()).expect("fits");
    grown.append(1, 5).expect("fits");
    let span = grown.spans[1];
    assert_eq!((span.start, span.len, span.cap), (1, 2, 2));
    assert_eq!(grown.values, [1, 2, 5]);
}

#[test]
fn dead_slots_past_the_live_count_compact_to_the_csr_layout() {
    let built: Vec<(u32, u32)> = (0..10).map(|r| (r, r)).collect();
    let mut grown = AppendCsr::from_pairs(10, built.iter().copied()).expect("fits");
    let mut pairs = built.clone();
    let mut compacted = false;
    for r in 0..9 {
        grown.append(r, 100 + r).expect("fits");
        pairs.push((r, 100 + r));
        compacted |= grown.spans.iter().all(|s| s.len == s.cap);
        assert!(
            grown.values.len() <= 2 * grown.len(),
            "dead never outlives live"
        );
    }
    assert!(compacted, "nine moves of cap 4 must trip the compaction");
    assert_rows_match(&grown, 10, &pairs);
    let frozen = Csr::from_pairs(10, pairs.iter().copied()).expect("fits");
    let mut flat = grown.clone();
    flat.compact();
    assert_eq!(flat.values, frozen.values);
}

#[test]
fn an_empty_adjacency_is_well_formed() {
    let none = AppendCsr::default();
    assert_eq!((none.rows(), none.len(), none.byte_len()), (0, 0, 0));
    assert!(none.is_empty());
    let bare = AppendCsr::from_pairs(2, std::iter::empty()).expect("fits");
    assert_eq!(
        (bare.rows(), bare.row(0), bare.row(1)),
        (2, &[][..], &[][..])
    );
}

#[test]
fn growth_past_u32_is_a_capacity_error() {
    assert_eq!(grown_cap(0), Ok(4));
    assert_eq!(grown_cap(3), Ok(6));
    assert_eq!(grown_cap(u32::MAX / 2 + 1), Err(OVERFLOW));
    let mut full = AppendCsr::from_pairs(1, [(0, 1)].into_iter()).expect("fits");
    full.live = u32::MAX;
    assert_eq!(full.append(0, 2), Err(OVERFLOW));
    assert_eq!(full.row(0), [1], "a refusal changes nothing");
}

#[test]
#[should_panic(expected = "index out of bounds")]
fn a_row_out_of_range_panics() {
    let mut grown = AppendCsr::from_pairs(1, std::iter::empty()).expect("fits");
    let _ = grown.append(1, 0);
}
