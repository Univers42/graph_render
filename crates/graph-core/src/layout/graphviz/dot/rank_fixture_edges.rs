//! The edges of the twenty fixture seeds `rank_tests.rs` and `order_tests.rs` pin, as
//! (source, target), so that neither test needs a file under `target/`. Written from
//! `graph-cli emit-graphviz-fixtures --engine twopi --seeds 20`, the generator that also
//! feeds the oracle; the first twenty seeds of the 1000-seed probe are the same graphs.
//!
//! The table is in two files because it does not fit in one: this one holds seeds 0 to 12 and
//! [`late`] holds 13 to 19, and [`all`] hands back the twenty in order. A fixture table is
//! data, so it is data all the way down — nothing here is derived, and nothing here is
//! hand-edited to make a test pass.

mod late;

pub use late::LATE_FIXTURE_EDGES;

/// Seeds 0 to 12: each seed's edges, in the fixture's own order.
pub const FIXTURE_EDGES: &[(u32, &[(u32, u32)])] = &[
    (0, &[(1, 0), (1, 0)]),
    (1, &[(1, 0), (2, 0), (2, 0)]),
    (2, &[(1, 0), (2, 0), (2, 0), (3, 2), (3, 0)]),
    (3, &[(1, 0), (1, 0), (2, 0), (3, 0), (3, 0), (4, 0), (4, 1)]),
    (
        4,
        &[
            (1, 0),
            (1, 0),
            (2, 1),
            (2, 0),
            (3, 0),
            (4, 0),
            (4, 0),
            (5, 1),
            (5, 1),
        ],
    ),
    (
        5,
        &[
            (1, 0),
            (1, 0),
            (2, 0),
            (2, 0),
            (3, 0),
            (3, 0),
            (4, 0),
            (5, 0),
            (5, 1),
            (6, 2),
            (6, 0),
        ],
    ),
    (
        6,
        &[
            (1, 0),
            (2, 0),
            (2, 0),
            (3, 0),
            (4, 0),
            (4, 0),
            (5, 1),
            (6, 1),
            (6, 2),
            (7, 0),
            (7, 0),
        ],
    ),
    (
        7,
        &[
            (1, 0),
            (2, 0),
            (2, 0),
            (3, 0),
            (3, 0),
            (4, 0),
            (5, 0),
            (5, 1),
            (6, 2),
            (6, 0),
            (7, 0),
            (7, 0),
            (8, 0),
        ],
    ),
    (
        8,
        &[
            (1, 0),
            (1, 0),
            (2, 0),
            (3, 0),
            (3, 0),
            (4, 1),
            (4, 1),
            (5, 0),
            (6, 0),
            (7, 0),
            (8, 1),
            (8, 0),
            (9, 1),
            (9, 5),
        ],
    ),
    (
        9,
        &[
            (1, 0),
            (1, 0),
            (2, 0),
            (3, 0),
            (3, 0),
            (4, 1),
            (5, 1),
            (5, 2),
            (6, 0),
            (6, 0),
            (7, 1),
            (7, 0),
            (8, 1),
            (8, 4),
            (9, 1),
            (10, 0),
        ],
    ),
    (
        10,
        &[
            (1, 0),
            (1, 0),
            (2, 0),
            (2, 0),
            (3, 0),
            (4, 0),
            (4, 0),
            (5, 2),
            (5, 0),
            (6, 0),
            (6, 0),
            (7, 0),
            (8, 0),
            (9, 3),
            (9, 1),
            (10, 1),
            (10, 0),
            (11, 1),
        ],
    ),
    (
        11,
        &[
            (1, 0),
            (1, 0),
            (2, 0),
            (2, 0),
            (3, 1),
            (3, 1),
            (4, 0),
            (5, 0),
            (6, 0),
            (7, 1),
            (7, 0),
            (8, 1),
            (8, 4),
            (9, 1),
            (10, 0),
            (11, 1),
            (11, 1),
            (12, 8),
        ],
    ),
    (
        12,
        &[
            (1, 0),
            (1, 0),
            (2, 0),
            (2, 0),
            (3, 0),
            (3, 0),
            (4, 1),
            (4, 0),
            (5, 0),
            (5, 0),
            (6, 0),
            (7, 0),
            (8, 3),
            (8, 1),
            (9, 1),
            (9, 0),
            (10, 1),
            (11, 4),
            (11, 0),
            (12, 4),
            (13, 11),
        ],
    ),
];

/// Every fixture seed's edges, seeds 0 to 19, in order — the whole table, which is what the
/// two pin tests iterate.
pub fn all() -> Vec<(u32, &'static [(u32, u32)])> {
    FIXTURE_EDGES
        .iter()
        .chain(LATE_FIXTURE_EDGES)
        .copied()
        .collect()
}
