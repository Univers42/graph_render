//! The generator's tests: the oracle's node count and its 6-node model, the stream's
//! boundaries (`<` against a draw, F-46), and the two guards on `pick` and on the
//! count itself (F-44, F-45).

use super::*;

#[test]
fn count_floors_clamps_and_reads_non_finite_as_two() {
    let cases = [
        (6.0, 6),
        (2.9, 2),
        (f64::NAN, 2),
        (f64::INFINITY, 2),
        (f64::NEG_INFINITY, 2),
        (-4.0, 2),
        (1e9, 100_000),
        (100_000.5, 100_000),
        (99_999.99, 99_999),
    ];
    for (n, want) in cases {
        assert_eq!(synthetic_count(n), want, "{n}");
    }
}

/// Why `<` and `<=` are one mutant here (`.cargo/mutants.toml`): the largest model
/// draws at most 2 per node, 7 per attachment step and 3 per extra edge — under
/// 1 000 000 — and none of the first 1 000 000 draws is exactly one half.
///
/// Ponytail: an empirical bound, not a proof. `1_000_000` is 2.2× what the largest
/// accepted model consumes (about 450 000 draws at `MAX_SYNTHETIC_NODES`), so the
/// margin is measured, not derived. Direction: one more draw per node, or a
/// [`MAX_BENCH_NODES`](crate::registry::MAX_BENCH_NODES) fixture, and the guarantee
/// stops being tested at the size that matters. Escape hatch:
/// `no_draw_lands_on_a_branch_boundary` below, which names every constant a `<` is
/// written against instead of the one this sweep happened to choose.
#[test]
fn no_draw_a_synthetic_model_can_reach_is_exactly_one_half() {
    let mut rnd = Mulberry32::new(SEED);
    assert!((0..1_000_000).all(|_| rnd.next_f64() != 0.5));
}

/// The three constants the generator tests a draw against with `<` (`synthetic_node`'s
/// `roll < 0.6` / `roll < 0.8`, `synthetic_edges`' `< 0.5`). A `<=` mutant is the same
/// function while no draw lands exactly on one of them; the sweep above measures that
/// for `0.5` alone, so this one covers the other two and all three are pinned at once.
#[test]
fn no_draw_lands_on_a_branch_boundary() {
    let mut rnd = Mulberry32::new(SEED);
    assert!((0..1_000_000).all(|_| {
        let roll = rnd.next_f64();
        roll != 0.5 && roll != 0.6 && roll != 0.8
    }));
}

/// `pick(len)` returns an index *inside* `len` for every table the generator reads and
/// every draw: `floor(rnd() * len)` is `< len` exactly while `rnd() < 1`.
#[test]
fn every_pick_of_every_reachable_table_is_inside_it() {
    let mut rnd = Mulberry32::new(SEED);
    for len in [1, 2, 4, 5, 12, crate::edgekind::EdgeKind::ALL.len()] {
        assert!((0..1000).all(|_| rnd.pick(len) < len), "len {len}");
    }
}

/// F-44: `pick(0)` has no index to return. The assert fires before the multiply, so
/// nothing is drawn and the stream is left where it was.
#[test]
#[cfg(debug_assertions)]
#[should_panic(expected = "pick from a table of at least one entry")]
fn picking_from_an_empty_table_is_refused() {
    Mulberry32::new(SEED).pick(0);
}

/// F-45: the oracle's `2..=100_000` clamp is **not** re-applied in
/// `synthetic_records`, because `seeded_model` (public) is handed counts up to 10× the
/// cap by the bench campaign and every one of those fixtures is hashed. One node past
/// the cap must come back as one node past it.
#[test]
fn a_count_past_the_oracles_cap_is_still_built_at_the_count_asked_for() {
    let past = MAX_SYNTHETIC_NODES + 1;
    let (nodes, edges) = synthetic_records(past);
    assert_eq!(
        nodes.len(),
        past as usize,
        "not clamped to the oracle's cap"
    );
    assert!(edges.len() > 100_000, "{} edges", edges.len());
    assert!(edges.iter().all(|e| e.id.starts_with("bench-e-")));
}

/// `buildSyntheticModel(6)` printed by the oracle under node:22-slim.
#[test]
fn six_nodes_match_the_oracle() {
    let t = build_synthetic_model(6.0).expect("fits");
    let icons: Vec<_> = (0..6).map(|i| t.node(i).icon).collect();
    let want = [
        "\u{1F33F}",
        "icon:map",
        "\u{1F4C8}",
        "icon:rocket",
        "\u{1F5FA}\u{FE0F}",
        "\u{1F5FA}\u{FE0F}",
    ];
    assert_eq!(icons, want.map(Some));
    let first = t.node(0);
    assert_eq!(
        (first.id, first.kind, first.has_note),
        ("bench:db-0:0", NodeKind::Note, true)
    );
    assert_eq!(
        (first.group, first.weight),
        (Some("Active"), 0.908_497_499_664_568_9)
    );
    let ends: Vec<_> = (0..t.edge_count())
        .map(|e| (t.edge(e).source, t.edge(e).target))
        .collect();
    assert_eq!(ends.len(), 9);
    assert_eq!(ends[8], ("bench:db-5:5", "bench:db-1:1"));
    let e0 = t.edge(0);
    assert_eq!(
        (e0.id, e0.strength, e0.directed),
        ("bench-e-0", 0.534_562_692_884_355_9, true)
    );
}

#[test]
fn stats_match_the_oracle_at_40_nodes_and_at_the_cap() {
    let forty = build_synthetic_model(40.0).expect("fits");
    assert_eq!(forty.stats().edges, 57);
    let note_links = (0..57).filter(|&e| forty.edge(e).kind == EdgeKind::NoteLink);
    assert_eq!(note_links.count(), 2);
    let (nodes, edges) = synthetic_records(MAX_SYNTHETIC_NODES);
    let notes = nodes.iter().filter(|n| n.kind == NodeKind::Note).count();
    assert_eq!((nodes.len(), edges.len(), notes), (100_000, 154_978, 4348));
}
