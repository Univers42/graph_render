//! The fixture table's own invariants: that every line is one case naming one reference
//! function, that the camera really is the rectangle it claims to be, that the comparison
//! is not vacuous, and that the two things the reference cannot decide stay undecided.
//! `harness/oracle-scale.py` is what checks the two arms against each other; these are the
//! checks that keep it pointed at the right thing.

use super::cases::{path, square};
use super::*;
use std::collections::BTreeSet;

/// Every case line parses, names a case of its own, and names a reference function the
/// ceilings know — so a line that fell out of the table cannot be compared against
/// nothing while the gate still reads as a pass.
#[test]
fn every_case_is_one_line_naming_a_reference_the_ceilings_know() {
    let mut names: Vec<String> = Vec::new();
    for case in 0..CASES {
        let line = line(case, None).unwrap_or_else(|e| panic!("case {case}: {e}"));
        assert!(line["case"].is_string(), "case {case} has no name");
        names.push(line["case"].as_str().expect("name").to_string());
        let reference = line["reference"].as_str().expect("reference");
        let judged = SCALE
            .ceilings
            .iter()
            .any(|&(_, key, _)| key == reference.rsplit('.').next().unwrap_or(""));
        assert!(
            judged,
            "case {case} names {reference}, which no ceiling judges"
        );
        assert!(line["input"].is_object(), "{}: no input", line["case"]);
        assert!(line["ours"].is_object(), "{}: no answer", line["case"]);
    }
    let mut sorted = names.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(
        sorted.len(),
        names.len(),
        "two cases share a name: {names:?}"
    );
    assert!(
        line(CASES, None).is_err(),
        "a case past the table must be refused"
    );
}

/// The camera is the rectangle: its corners land on the corners of the NDC box, and a
/// node's disc crosses the edge exactly when the motor's own rectangle test says it does.
/// Without this the cull comparison could pass on a matrix that culls everything or
/// nothing.
#[test]
fn the_camera_is_the_rectangle_and_its_radius_crosses_the_edge() {
    let view = square(0.0, 8.0, 0.5);
    let matrix = flat(&ortho(view));
    // The reference reads `radii` in clip space: `clip_radii`, not the world radius.
    let r = view.radius * 2.0 / (view.x1 - view.x0);
    let at = |x: f64, y: f64| ref_kept(&matrix, x, y, r);
    assert!(at(4.0, 4.0), "the middle is inside");
    assert!(at(7.75, 4.0), "a disc straddling the right edge is kept");
    assert!(!at(8.75, 4.0), "a disc past the right edge is culled");
    assert!(at(4.0, -0.25), "a disc straddling the left edge is kept");
    assert!(!at(4.0, -0.75), "a disc past the left edge is culled");
    assert!(at(4.0, 8.25), "a disc straddling the top edge is kept");
    assert!(!at(4.0, 8.75), "a disc past the top edge is culled");
    assert!(ref_kept(&matrix, 0.0, 0.0, 0.0) && ref_kept(&matrix, 8.0, 8.0, 0.0));
}

/// The clip-space radius the fixture states is the world radius scaled the way the
/// reference reads it, and the two shapes it cannot express are refused rather than
/// compared against something the fixture never said.
#[test]
fn the_radius_is_the_world_radius_scaled_and_only_a_square_rectangle_carries_it() {
    let view = square(0.0, 8.0, 0.5);
    assert_eq!(clip_radii(view, 8).expect("square"), vec![0.125; 8]);
    let wide = Viewport::from_size(0.0, 0.0, 8.0, 4.0, 0.5);
    assert!(
        clip_radii(wide, 8).is_err(),
        "one radius cannot be both axes' margin"
    );
    let fat = square(0.0, 8.0, 4.5);
    assert!(
        clip_radii(fat, 8).is_err(),
        "a radius over half the side culls on z alone"
    );
}

/// A node sitting exactly on a decision boundary is refused, because the reference's
/// `abs(w) + 1e-9` would decide it rather than the rectangle: a case that passes or fails
/// on 4e-9 of slack is not evidence of anything. The boundary is the side grown by the
/// radius, so it is `8 + 0.5` and not `8`.
#[test]
fn a_case_whose_node_is_on_the_boundary_is_refused() {
    let on = Fixture {
        topology: path().expect("path").topology,
        x: vec![2.0, 3.0, 4.0, 5.0, 6.0, 8.5],
        y: vec![4.0; 6],
    };
    assert!(
        !decidable(square(0.0, 8.0, 0.5), &on),
        "8.5 is 8 + 0.5, the right boundary"
    );
    assert!(
        cull("edge", &on, square(0.0, 8.0, 0.5)).is_err(),
        "refused, not compared"
    );
    let inside = Fixture {
        topology: path().expect("path").topology,
        x: vec![2.0, 3.0, 4.0, 5.0, 6.0, 7.0],
        y: vec![4.0; 6],
    };
    assert!(decidable(square(0.0, 8.0, 0.5), &inside));
    assert!(cull("edge", &inside, square(0.0, 8.0, 0.5)).is_ok());
}

/// The comparison is not vacuous: the cull fixture hides half its nodes, the budget
/// fixture separates a budget of 0 from 1 from n, and the coarse fixture's two communities
/// really do produce one link between them.
#[test]
fn the_cases_separate_the_answers_they_compare() {
    assert_eq!(
        line(0, None).expect("0")["ours"]["mask"],
        json!([1, 1, 1, 1, 1, 1])
    );
    assert_eq!(
        line(1, None).expect("1")["ours"]["mask"],
        json!([1, 0, 0, 0, 0, 0])
    );
    assert_eq!(
        line(3, None).expect("n")["ours"]["mask"],
        json!([1, 1, 1, 1, 1, 1])
    );
    assert_eq!(
        line(5, None).expect("half")["ours"]["mask"],
        json!([1, 1, 1, 1, 0, 0, 0, 0]),
        "half culled"
    );
    let two = line(9, None).expect("two communities");
    assert_eq!(
        two["ours"]["links"],
        json!([[0, 3]]),
        "one super-edge between the reps"
    );
    assert_eq!(
        two["map"]["community"],
        json!([[0, 0], [1, 3]]),
        "ascending by community id"
    );
    assert_eq!(
        line(10, None).expect("self loop")["ours"]["links"],
        two["ours"]["links"],
        "a self-loop on a collapsed member is not a link"
    );
}

/// The star and the path are single communities under louvain on some partitions and
/// several on others, and the reference refuses a coarse level in the first case
/// (`simplify.py:170-171`). Either way the two arms must be handed the same partition, so
/// what is compared is the collapse: the fixture's `labels` are the motor's own.
#[test]
fn the_coarse_fixture_carries_the_motor_own_partition_and_its_map_agrees_with_it() {
    for case in 7..=10 {
        let line = line(case, None).unwrap_or_else(|e| panic!("case {case}: {e}"));
        let labels = numbers(&line["input"]["labels"]);
        let map = pairs_of(&line["map"]["community"]);
        let distinct = labels.iter().collect::<BTreeSet<_>>().len();
        assert_eq!(map.len(), distinct, "case {case}: one entry per community");
        let mut ids: Vec<u32> = map.iter().map(|&(id, _)| id).collect();
        let ascending = ids.clone();
        ids.sort_unstable();
        assert_eq!(
            ids, ascending,
            "case {case}: the map is ascending by id, as np.unique is"
        );
        for (id, rep) in map {
            for (node, &community) in labels.iter().enumerate() {
                if community == id {
                    assert!(
                        node as u32 >= rep,
                        "case {case}: node {node} -> representative {rep}"
                    );
                }
            }
        }
    }
}

/// The tie fixture is the one case the reference cannot answer, so it has to keep the shape
/// that makes it one: the budget's cut strictly inside a class of equal degrees. Degrees
/// `[1, 2, 3, 1, 2, 3]` at a budget of 3 selects one of the two degree-2 nodes, and which
/// one is `np.argsort`'s business, not the rule's.
#[test]
fn the_tie_case_cuts_inside_a_class_of_equal_degrees() {
    let line = line(11, None).expect("the tie case");
    assert_eq!(line["case"], "lod.budget.tie");
    let key = numbers(&line["input"]["order_key"]);
    assert_eq!(
        key,
        vec![1, 2, 3, 1, 2, 3],
        "two classes tied across the cut"
    );
    let mask = numbers(&line["ours"]["mask"]);
    assert_eq!(
        mask.iter().filter(|&&m| m == 1).count(),
        3,
        "the budget is 3"
    );
    let (kept, dropped): (Vec<u32>, Vec<u32>) = split_on(&key, &mask);
    assert!(
        kept.iter().min() <= dropped.iter().max(),
        "one degree is both kept and dropped, which is what makes the case undecidable"
    );
}

// --- helpers

/// A JSON array of numbers as `u32`.
fn numbers(value: &Value) -> Vec<u32> {
    value
        .as_array()
        .expect("an array")
        .iter()
        .map(|v| v.as_f64().expect("a number") as u32)
        .collect()
}

/// A JSON array of `[a, b]` pairs as `u32` tuples.
fn pairs_of(value: &Value) -> Vec<(u32, u32)> {
    value
        .as_array()
        .expect("an array of pairs")
        .iter()
        .map(|p| {
            let p = p.as_array().expect("a pair");
            (
                numbers(&Value::Array(vec![p[0].clone()]))[0],
                numbers(&Value::Array(vec![p[1].clone()]))[0],
            )
        })
        .collect()
}

/// The keys the mask keeps and the keys it drops.
fn split_on(key: &[u32], mask: &[u32]) -> (Vec<u32>, Vec<u32>) {
    let split = |want: u32| -> Vec<u32> {
        key.iter()
            .zip(mask)
            .filter(|&(_, &m)| m == want)
            .map(|(&k, _)| k)
            .collect()
    };
    (split(1), split(0))
}

// --- the reference arithmetic, re-run natively so a matrix change is caught here and not
// --- only in the container. numpy is not a graph-cli dependency, so the one function the
// --- camera test needs is written out longhand: it is `lod.py:35-47` with `w = 1`.

/// `ortho(view)` as a flat row-major array of `f64`.
fn flat(matrix: &[Vec<f64>]) -> Vec<f64> {
    matrix.iter().flat_map(|row| row.iter().copied()).collect()
}

/// `lod.frustum_cull_spheres` for one node against [`flat`]'s matrix, `z = 0`: each axis
/// keeps when `clip/aw ± r/aw` brackets the NDC box, and the near plane keeps the rest.
fn ref_kept(matrix: &[f64], x: f64, y: f64, radius: f64) -> bool {
    let clip = |row: usize, p: [f64; 3]| {
        matrix[row * 4] * p[0]
            + matrix[row * 4 + 1] * p[1]
            + matrix[row * 4 + 2] * p[2]
            + matrix[row * 4 + 3]
    };
    let point = [x, y, 0.0];
    let aw = 1.0 + 1e-9;
    let keep = (0..3).all(|axis| {
        let ndc = clip(axis, point) / aw;
        (ndc + radius / aw >= -1.0) && (ndc - radius / aw <= 1.0)
    });
    keep && clip(2, point) + radius >= -aw
}
