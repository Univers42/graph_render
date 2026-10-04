//! The shape measure the collapse tests are written against, and the two graphs they run on.
//!
//! **Shape ratio** (`prompts/jobs/sg-sfdp-collapse.md`): the small eigenvalue over the large one
//! of the 2-D covariance of the points. 1 is a disc, 0 is a line. Graphviz 16.1.0's own `lesmis`
//! drawing reads 0.414 on the conformance arm; the motor read 0.006 before this repair.
//!
//! **Coincidence** is a second, separate measure, and it is the one that catches the failure
//! shapes the ratio misses: no two points within `1e-6` of the drawing's own extent. A drawing
//! can have a healthy ratio and still stack 40 pairs of nodes on one another.
//!
//! **Node order is part of the drawing, and this module picks one deliberately.** `lesmis`'s
//! node ids are the integers `0..76`, used here as dense indices in **numeric** order. The
//! conformance reader (`conformance/fixtures.rs`) numbers them in **byte** order, so
//! `"10"` sorts between `"1"` and `"2"`: the same graph, a different index order, and therefore
//! a different drawing. The numbers in `docs/measurements/sg-sfdp-collapse.md` are quoted from
//! both and say which is which.

use graph_contract::canonical_json::{Value, parse};

/// The gallery graph, 77 nodes and 254 edges (`fixtures/scigraphs/lesmis.json`, the same file
/// `conformance/fixtures/named.rs` reads for the `GRAPHVIZ_SFDP` row).
const LESMIS: &str = include_str!("../../../../../../fixtures/scigraphs/lesmis.json");

/// The lowest shape ratio that still separates "a layout" from "a line". Chosen well under
/// Graphviz's own 0.414 so the floor is a floor and not a moving target.
pub(super) const MIN_RATIO: f64 = 0.15;

/// `lesmis.json` as `(node count, edges)`. Its node ids are integers that are also their own
/// positions, which is what `conformance/fixtures/named.rs:28-46` checks, so an edge's ends are
/// used as dense indices unchanged.
pub(super) fn lesmis() -> (u32, Vec<(u32, u32)>) {
    let Value::Object(fields) = &parse(LESMIS).expect("lesmis json") else {
        panic!("lesmis is an object");
    };
    (node_count(fields), edges(fields))
}

fn node_count(fields: &[(String, Value)]) -> u32 {
    match field(fields, "nodes") {
        Value::Array(items) => items.len() as u32,
        other => panic!("lesmis nodes: {other:?}"),
    }
}

fn edges(fields: &[(String, Value)]) -> Vec<(u32, u32)> {
    let Value::Array(items) = field(fields, "edges") else {
        panic!("lesmis edges are not an array");
    };
    items.iter().map(ends).collect()
}

fn field<'a>(fields: &'a [(String, Value)], key: &str) -> &'a Value {
    fields
        .iter()
        .find(|(name, _)| name == key)
        .map(|(_, value)| value)
        .expect("lesmis names the field")
}

fn ends(item: &Value) -> (u32, u32) {
    let Value::Array(pair) = item else {
        panic!("edge is not a pair: {item:?}");
    };
    let mut ends = pair.iter().map(|v| match v {
        Value::Number(text) => text.parse::<u32>().expect("node id"),
        other => panic!("edge end: {other:?}"),
    });
    (ends.next().expect("from"), ends.next().expect("to"))
}

/// The `side × side` lattice's edges, in row-major order.
pub(super) fn grid_edges(side: u32) -> Vec<(u32, u32)> {
    let index = |r: u32, c: u32| r * side + c;
    let mut out = Vec::new();
    for r in 0..side {
        for c in 0..side {
            if c + 1 < side {
                out.push((index(r, c), index(r, c + 1)));
            }
            if r + 1 < side {
                out.push((index(r, c), index(r + 1, c)));
            }
        }
    }
    out
}

/// The shape ratio of `points`, small eigenvalue over large one.
pub(super) fn shape_ratio(points: &[(f32, f32)]) -> f64 {
    let n = points.len() as f64;
    let (mut mx, mut my) = (0.0f64, 0.0f64);
    for (x, y) in points {
        mx += f64::from(*x);
        my += f64::from(*y);
    }
    let (mx, my) = (mx / n, my / n);
    let (mut sxx, mut syy, mut sxy) = (0.0f64, 0.0f64, 0.0f64);
    for (x, y) in points {
        let (dx, dy) = (f64::from(*x) - mx, f64::from(*y) - my);
        sxx += dx * dx;
        syy += dy * dy;
        sxy += dx * dy;
    }
    let (sxx, syy, sxy) = (sxx / n, syy / n, sxy / n);
    let half = 0.5 * (sxx + syy);
    let disc = (0.5 * (sxx - syy) * (sxx - syy) + sxy * sxy).sqrt();
    (half - 0.5 * disc) / (half + 0.5 * disc)
}

/// The drawing's own extent, from which the coincidence threshold is `1e-6` of it.
pub(super) fn extent(points: &[(f32, f32)]) -> f64 {
    let (mut lo, mut hi) = (f64::MAX, f64::MIN);
    for (x, y) in points {
        lo = lo.min(f64::from(*x)).min(f64::from(*y));
        hi = hi.max(f64::from(*x)).max(f64::from(*y));
    }
    hi - lo
}

/// The index of the closest pair closer than `1e-6` of the extent, if there is one.
pub(super) fn closest_coincident(points: &[(f32, f32)]) -> Option<(usize, usize, f64)> {
    let near = 1e-6 * extent(points);
    let mut worst: Option<(usize, usize, f64)> = None;
    for i in 0..points.len() {
        for j in i + 1..points.len() {
            let (dx, dy) = (points[i].0 - points[j].0, points[i].1 - points[j].1);
            let apart = f64::from(dx.hypot(dy));
            if apart <= near && worst.is_none_or(|(_, _, seen)| apart < seen) {
                worst = Some((i, j, apart));
            }
        }
    }
    worst
}

/// The two properties every collapse test asserts: a shape ratio above [`MIN_RATIO`] and no two
/// points closer than `1e-6` of the drawing's extent.
pub(super) fn assert_spread(points: &[(f32, f32)]) {
    let ratio = shape_ratio(points);
    assert!(ratio > MIN_RATIO, "shape ratio {ratio}: a strip or a point");
    if let Some((i, j, apart)) = closest_coincident(points) {
        panic!(
            "nodes {i} and {j} are {apart} apart, under {near}",
            near = 1e-6 * extent(points)
        );
    }
}
