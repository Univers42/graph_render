//! The motor arm's own tests: the byte formats, the one-bit break, and the four overrides.
//! Each is a thing that could be wrong with no coordinate visibly wrong.

use super::super::SCALE;
use super::super::fixtures::all;
use super::super::rows::ROWS;
use super::{BREAK_ENV, columns, flip_one_bit, raw_f32, raw_f64, row_line, run};
use graph_contract::binary::SnapshotParts;
use graph_core::layout::Geometry;
use graph_core::layout::circle_packing::{self, CirclePackingParams};
use graph_core::layout::forceatlas2::{Fa2Params, ForceAtlas2};
use graph_core::layout::graphviz::{self, sfdp};
use graph_core::{Stage, StageError, Topology, registry, run_with};

fn bipartite() -> super::super::fixtures::Fixture {
    all()
        .expect("the fixture set")
        .into_iter()
        .find(|f| f.name == "bipartite")
        .expect("the bipartite fixture")
}

/// The `f64` file is exactly three little-endian doubles per node, in fixture order, and the
/// `f32` file exactly half as many bytes. A header or a padding byte would put the two arms
/// a coordinate apart without either being wrong.
#[test]
fn the_raw_files_are_three_doubles_per_node() {
    let fixture = bipartite();
    let points: Vec<super::Ran> = vec![Ok(run("layout.grid", &fixture).expect("grid"))];
    let (bytes, count) = raw_f64(&points, false);
    assert_eq!(count, 3 * fixture.nodes.len() as u64);
    assert_eq!(bytes.len(), 8 * count as usize);
    assert_eq!(raw_f32(&bytes).len(), 4 * count as usize);
}

/// The break is one bit of one double, and it is bit 28, which is the last bit the `f32`
/// keeps — so a row the break touched is caught whether the judge reads the `f64` or the
/// `f32`. A break that survived only one of the two files would leave a hole in the gate.
#[test]
fn the_break_is_one_bit_that_survives_narrowing() {
    // 1.0 exactly: exponent zero, so one bit 29 is one `f32` ULP with nothing
    // else in the way.
    let original = 0x3f_f0_00_00_00_00_00_00u64.to_le_bytes();
    let mut bytes = original.to_vec();
    flip_one_bit(&mut bytes).expect("a word to break");
    assert_eq!(bytes.len(), original.len());
    assert_eq!(
        bytes
            .iter()
            .zip(original.iter())
            .filter(|(a, b)| a != b)
            .count(),
        1,
        "more than one byte moved"
    );
    let before = f64::from_le_bytes(original);
    let after = f64::from_le_bytes(bytes.clone().try_into().unwrap());
    assert_ne!(after as f32, before as f32, "the f32 did not move");
    // The `f32` file is the *narrowed* `f64` file, break included: one break that moved only
    // the `f64` would leave the judge a file that does not describe the row.
    assert_eq!(
        raw_f32(&bytes),
        narrow(after),
        "the f32 file is not the narrowed f64"
    );
    assert_eq!(
        raw_f32(original.as_ref()),
        narrow(before),
        "the f32 file is not narrowed"
    );
    assert_ne!(
        raw_f32(&bytes),
        raw_f32(original.as_ref()),
        "the f32 file did not move"
    );
    assert_eq!(
        (after - before).abs(),
        f32::from_bits(0x3f80_0001) as f64 - 1.0,
        "not one f32 ULP"
    );
}

/// The break names the row it broke and nothing else. A `row_line` that said `ok` while its
/// file was perturbed would make the negative control pass a healthy tree.
#[test]
fn a_broken_row_says_so_and_an_untouched_one_does_not() {
    let fixture = bipartite();
    let points: Vec<super::Ran> = vec![Ok(run("layout.grid", &fixture).expect("grid"))];
    let names = ["bipartite"];
    let broken = row_line(grid_row(), &points, &names, Some("sha".into()), 42, true);
    assert!(
        broken["motor"].as_str().unwrap().starts_with("broken:"),
        "{broken}"
    );
    assert_eq!(broken["broken"], serde_json::Value::Bool(true));
    let whole = row_line(grid_row(), &points, &names, Some("sha".into()), 42, false);
    assert_eq!(whole["motor"], "ok");
    assert_eq!(whole["broken"], serde_json::Value::Bool(false));
}

/// A row with no motor layout is reported as `not run`, with the reason — never as `ok` and
/// never as a zero coordinate count that a reader could mistake for a measurement.
#[test]
fn a_row_with_no_motor_layout_says_not_run() {
    let line = row_line(dot_row(), &[], &[], None, 0, false);
    assert!(
        line["motor"].as_str().unwrap().starts_with("not run:"),
        "{line}"
    );
    assert_eq!(line["coordinates"], serde_json::Value::from(0));
    assert_eq!(line["sha256"], serde_json::Value::Null);
}

/// Every gap the table declares reaches `motor.jsonl`, with its `file:line` intact. A gap
/// that stopped being reported would make a row's cause change with nobody reading it.
#[test]
fn every_declared_gap_reaches_the_row_line() {
    let line = row_line(dot_row(), &[], &[], None, 0, false);
    assert!(line["convention_gaps"].is_array());
    let dot = dot_row();
    assert_eq!(
        line["convention_gaps"].as_array().map(Vec::len),
        Some(dot.gaps.len()),
        "the gaps did not reach the line"
    );
}

/// **Override 1.** Circle packing's registered budget is 500 radius-solver sweeps; the arm
/// must run the 50 `apply_graph_layout` passes. If the override were dropped the coordinates
/// would still be a packing and nothing else would say so.
#[test]
fn circle_packing_runs_fifty_sweeps_not_the_registered_five_hundred() {
    assert_eq!(CirclePackingParams::default().iterations, 500);
    let fixture = bipartite();
    let at_fifty = parts_over(&fixture, circle_packing::ID, |t| {
        circle_packing::run_with(
            t,
            &CirclePackingParams {
                iterations: 50,
                scale: 5.0,
                ..CirclePackingParams::default()
            },
        )
    });
    let at_default = parts_over(&fixture, circle_packing::ID, |t| {
        circle_packing::run_with(t, &CirclePackingParams::default())
    });
    assert_ne!(
        shape(&at_fifty, &fixture),
        shape(&at_default, &fixture),
        "the override changed nothing: 50 and 500 are the same packing"
    );
}

/// **Override 2.** FA2's registered `max_iter` is 100 where the dispatcher passes 50.
#[test]
fn forceatlas2_runs_fifty_iterations_not_the_registered_hundred() {
    assert_eq!(Fa2Params::default().max_iter, 100);
    let fixture = bipartite();
    let at_fifty = parts_over(&fixture, ForceAtlas2::ID, |t| {
        ForceAtlas2::run(
            t,
            &Fa2Params {
                max_iter: 50,
                seed: super::super::LAYOUT_SEED,
                ..Fa2Params::default()
            },
        )
    });
    let at_default = parts_over(&fixture, ForceAtlas2::ID, |t| {
        ForceAtlas2::run(t, &Fa2Params::default())
    });
    assert_ne!(
        shape(&at_fifty, &fixture),
        shape(&at_default, &fixture),
        "the override changed nothing"
    );
}

/// **Override 3.** sfdp registers `run`, whose `DEFAULT_SEED` is 1; the arm calls
/// `run_seeded(get_layout_seed())`. The two must draw differently or the seed is decorative.
#[test]
fn sfdp_runs_at_the_layout_seed_not_its_registered_default() {
    assert_eq!(sfdp::DEFAULT_SEED, 1);
    let fixture = bipartite();
    let seeded = parts_over(&fixture, sfdp::ID, |t| {
        sfdp::run_seeded(t, super::super::LAYOUT_SEED)
    });
    let default = parts_over(&fixture, sfdp::ID, sfdp::run);
    assert_ne!(
        shape(&seeded, &fixture),
        shape(&default, &fixture),
        "the seed moved nothing"
    );
}

/// A 2D layout's z is `0.0` and is **reported**, not dropped: SciGraphs always writes three
/// coordinates, so the third column is a real difference on most rows and hiding it would
/// make a planar-against-planar row read as a 2D comparison.
#[test]
fn a_planar_layout_reports_a_zero_third_column() {
    let fixture = bipartite();
    let points = run("layout.grid", &fixture).expect("grid");
    assert!(points.iter().all(|p| p[2] == 0.0), "a 2D layout wrote a z");
    let solid = run("layout.basic3d.sphere", &fixture).expect("sphere");
    assert!(solid.iter().any(|p| p[2] != 0.0), "sphere wrote no z");
}

/// Every registered motor id produces three columns per node on at least one fixture, or says
/// why it does not. A layout that returns nothing on everything would otherwise be a row of
/// zeros.
#[test]
fn every_motor_id_produces_coordinates_somewhere() {
    let set = all().expect("the fixture set");
    let mut silent = Vec::new();
    for row in ROWS.iter().filter(|row| row.motor.is_some()) {
        let id = row.motor.unwrap();
        let ran = set.iter().filter_map(|f| run(id, f).ok()).count();
        if ran == 0 {
            silent.push(id);
        }
    }
    assert!(
        silent.is_empty(),
        "a motor layout produced no coordinates: {silent:?}"
    );
}

/// The break is read from the environment and only when it is set to a name.
#[test]
fn the_break_name_is_read_from_the_environment() {
    // The test process runs with no control set, so the honest run is the default.
    assert_eq!(super::break_one(), None);
    assert!(BREAK_ENV.starts_with("GM_MUTATE_"), "{BREAK_ENV}");
}

/// One fixture through the pipeline's own `run_with`, so the columns under test are the
/// snapshot's — the ones the comparison reads — and not the geometry the layout returned.
fn parts_over(
    fixture: &super::super::fixtures::Fixture,
    id: &'static str,
    layout: impl FnOnce(&Topology) -> Result<Geometry, StageError>,
) -> SnapshotParts {
    run_with(&fixture.nodes, &fixture.edges, id, layout)
        .map(|run| run.snapshot.into_parts())
        .map_err(|e: StageError| e.to_string())
        .expect("a layout over the bipartite fixture")
}

fn shape(parts: &SnapshotParts, fixture: &super::super::fixtures::Fixture) -> Vec<[f64; 3]> {
    columns(parts, fixture.nodes.len()).expect("three columns per node")
}

/// One `f64` narrowed to the four bytes [`raw_f32`] writes for it.
fn narrow(value: f64) -> Vec<u8> {
    (value as f32).to_le_bytes().to_vec()
}

fn grid_row() -> &'static super::super::Row {
    ROWS.iter().find(|r| r.name == "GRID").expect("GRID")
}

fn dot_row() -> &'static super::super::Row {
    ROWS.iter()
        .find(|r| r.name == "GRAPHVIZ_DOT")
        .expect("GRAPHVIZ_DOT")
}

/// The Graphviz module is reached through the layout tree, not through the registry: this
/// test holds that the seeded entry point the override names is the one that exists.
#[test]
fn the_seeded_sfdp_entry_point_is_the_one_the_override_names() {
    let _ = graphviz::sfdp::ID;
    let layout = registry::find(sfdp::ID).expect("sfdp is registered");
    assert_eq!(layout.id, sfdp::ID);
}

/// The sugiyama override calls the **scaled** entry point, not the registered `run`, and the
/// two differ in units: the registered layout draws X in the priority method's own units and
/// Y as `layer * LAYER_SPACING`, the scaled one on SciGraphs' `[-scale, scale]` axes. The
/// reference's `lo`/`hi` are over the dummy vertices too (`hierarchical.py:679-681`), which is
/// why this is a second entry point in graph-core and not a post pass here — so this test
/// holds the two apart, and that the registered default is still what the registry hands out.
#[test]
fn the_sugiyama_override_is_the_scaled_entry_point_and_leaves_the_registered_one_alone() {
    let fixture = bipartite();
    let scaled = run("layout.dag.sugiyama", &fixture).expect("sugiyama");
    assert!(
        scaled
            .iter()
            .all(|p| p[0].abs() <= SCALE && p[1].abs() <= SCALE),
        "the scaled arm left [-scale, scale]: {scaled:?}"
    );

    let registered = registry::find("layout.dag.sugiyama").expect("registered");
    let t = parts_over(&fixture, registered.id, |t| (registered.run)(t));
    let own = shape(&t, &fixture);
    assert_ne!(own, scaled, "the two entry points must not agree");
}
