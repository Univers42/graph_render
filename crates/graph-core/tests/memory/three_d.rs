//! The 3D arms of [`super`]'s measurements, and the numbers `registry/three_d.rs`'s
//! `BASIC_3D_CEILING` doc now rests on. Run with the command in that file's header:
//!
//! ```sh
//! docker run --rm -v "$PWD:/w" ge-rust \
//!   cargo test --release -p graph-core --test memory -- --ignored --nocapture
//! ```
//!
//! One arm per 3D layout family, same allocator, same [`super::print_measurement`], same
//! three node counts, so every figure below is comparable with the 2D rows in the parent.
//! Every row under `registry/three_d.rs`'s `BASIC_3D_CEILING` is covered by one of the
//! three arms (`layout.force.spring3d` is not: it takes `SPRING_CEILING`). The closed forms
//! are swept as four rows rather than one so the "they share an allocation shape" claim
//! can be checked instead of asserted; [`basic_3d_closed_form_pipeline_memory_per_node`]
//! records which rows agreed.

use super::{Run, print_measurement};

/// `layout.hierarchical3d`, the 3D row that reads the graph and builds the level
/// structure, swept to 100 000 nodes like its 2D hierarchy siblings.
#[test]
#[ignore = "a measurement, not a check: run alone with --release -- --ignored --nocapture"]
fn hierarchical_3d_pipeline_memory_per_node() {
    for n in [1_000_u32, 10_000, 100_000] {
        print_measurement(
            graph_core::layout::hierarchical_3d::ID,
            graph_core::layout::hierarchical_3d::run,
            n,
        );
    }
}

/// `layout.bipartite_3d`, the third graph-reading 3D row: two node sets on parallel
/// planes, one ring each. Swept beside `layout.hierarchical3d` because both read the graph
/// and are the only two rows under `registry/three_d.rs`'s ceiling that do.
#[test]
#[ignore = "a measurement, not a check: run alone with --release -- --ignored --nocapture"]
fn bipartite_3d_pipeline_memory_per_node() {
    for n in [1_000_u32, 10_000, 100_000] {
        print_measurement(
            graph_core::layout::basic_3d::bipartite_3d::ID,
            graph_core::layout::basic_3d::bipartite_3d,
            n,
        );
    }
}

/// The four graph-free closed forms: `layout.basic3d.sphere`, `helix`, `cube` and
/// `spiral`. Each reads the node count and no edge, so one arm standing for all four is a
/// claim about their allocation shape — and it is measured here rather than assumed: at
/// 10 000 and at 100 000 nodes all four peak at the same byte (8 199 698 and 91 930 690),
/// 0 % apart rather than merely inside 5 %. At 1 000 nodes three of the four are 887 903 B
/// and `spiral` is 1 315 703 B, so the shared shape is stated at the two larger sizes and
/// the smallest size is where it does not hold. Blocks pasted at
/// `docs/measurements/fix-memory-3d.md`.
#[test]
#[ignore = "a measurement, not a check: run alone with --release -- --ignored --nocapture"]
fn basic_3d_closed_form_pipeline_memory_per_node() {
    use graph_core::layout::basic_3d;
    let layouts: [(&str, Run); 4] = [
        (basic_3d::sphere::ID, basic_3d::sphere),
        (basic_3d::helix::ID, basic_3d::helix),
        (basic_3d::cube::ID, basic_3d::cube),
        (basic_3d::spiral::ID, basic_3d::spiral),
    ];
    for (id, run) in layouts {
        for n in [1_000_u32, 10_000, 100_000] {
            print_measurement(id, run, n);
        }
    }
}
