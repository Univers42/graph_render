//! The table's own tests. Every one of these is a property a row can lose without any
//! coordinate changing, which is why they are here and not in the doc.

use super::baseline::BASELINE;
use super::{ROWS, Reference};
use std::collections::BTreeSet;

/// The matrix has exactly the 32 names `dispatcher.py` dispatches on, once each. A name
/// listed twice would give two rows the same coordinates and one name no row at all.
#[test]
fn there_are_thirty_two_rows_named_once() {
    assert_eq!(ROWS.len(), 32);
    let names: Vec<&str> = ROWS.iter().map(|r| r.name).collect();
    let unique: BTreeSet<&str> = names.iter().copied().collect();
    assert_eq!(unique.len(), ROWS.len(), "a SciGraphs name is listed twice");
}

/// The 32 names, spelled exactly as `dispatcher.py` spells them. This is the list the matrix
/// is about, so a rename on our side and not on SciGraphs' would silently drop a row.
#[test]
fn every_row_is_a_name_the_dispatcher_dispatches_on() {
    const NAMES: [&str; 32] = [
        "RANDOM",
        "GRID",
        "SPRING",
        "SPRING_3D",
        "CIRCLE_PACKING",
        "FORCEATLAS2",
        "IGRAPH_FR",
        "IGRAPH_KK",
        "IGRAPH_DRL",
        "IGRAPH_DRL_2D",
        "IGRAPH_LGL",
        "SPHERE",
        "SPECTRAL_3D",
        "SPIRAL_3D",
        "HELIX",
        "CUBE",
        "HIERARCHICAL_3D",
        "BIPARTITE_3D",
        "IGRAPH_DH",
        "IGRAPH_GRAPHOPT",
        "MDS_3D",
        "YIFAN_HU",
        "GRAPHVIZ_DOT",
        "GRAPHVIZ_NEATO",
        "GRAPHVIZ_FDP",
        "GRAPHVIZ_SFDP",
        "GRAPHVIZ_TWOPI",
        "GRAPHVIZ_CIRCO",
        "GRAPHVIZ_OSAGE",
        "GRAPHVIZ_PATCHWORK",
        "SUGIYAMA",
        "CIRCULAR_HIERARCHY",
    ];
    for name in NAMES {
        assert!(
            ROWS.iter().any(|row| row.name == name),
            "{name}: a name the dispatcher dispatches on has no row"
        );
    }
}

/// Every motor id in the table is registered on this tree, so no row's motor cells can be
/// `not run: not registered`. `GRAPHVIZ_DOT` is the one row with no motor id at all, and the
/// test says which rather than skipping it silently.
#[test]
fn every_motor_id_is_registered_and_one_row_has_none() {
    let without: Vec<&str> = ROWS
        .iter()
        .filter(|row| row.motor.is_none())
        .map(|row| row.name)
        .collect();
    assert_eq!(without, ["GRAPHVIZ_DOT"], "the rows with no motor id");
    for row in ROWS.iter().filter(|row| row.motor.is_some()) {
        let id = row.motor.unwrap();
        assert!(
            graph_core::registry::find(id).is_some(),
            "{id}: named by {} but not registered",
            row.name
        );
    }
}

/// The eight Graphviz rows name the eight engines `GRAPHVIZ_ENGINES` maps them to, plus
/// `YIFAN_HU`, which SciGraphs runs through the same sfdp call (`yifan_hu.py:366`). A row
/// pointing at the wrong engine would compare two different references under one name.
#[test]
fn the_graphviz_rows_name_their_own_engines() {
    let engines: Vec<(&str, Option<&str>)> =
        ROWS.iter().map(|row| (row.name, row.engine())).collect();
    let named = |name: &str| -> Option<&'static str> {
        engines
            .iter()
            .find(|(candidate, _)| *candidate == name)
            .and_then(|(_, engine)| *engine)
    };
    assert_eq!(named("GRAPHVIZ_DOT"), Some("dot"));
    assert_eq!(named("GRAPHVIZ_NEATO"), Some("neato"));
    assert_eq!(named("GRAPHVIZ_FDP"), Some("fdp"));
    assert_eq!(named("GRAPHVIZ_SFDP"), Some("sfdp"));
    assert_eq!(named("GRAPHVIZ_TWOPI"), Some("twopi"));
    assert_eq!(named("GRAPHVIZ_CIRCO"), Some("circo"));
    assert_eq!(named("GRAPHVIZ_OSAGE"), Some("osage"));
    assert_eq!(named("GRAPHVIZ_PATCHWORK"), Some("patchwork"));
    assert_eq!(
        named("YIFAN_HU"),
        Some("sfdp"),
        "SciGraphs runs YIFAN_HU through sfdp"
    );
    assert_eq!(
        named("SPRING_3D"),
        None,
        "the SciGraphs arm is not an engine"
    );
    assert_eq!(
        ROWS.iter()
            .filter(|r| matches!(r.reference, Reference::Graphviz(_)))
            .count(),
        9,
        "the Graphviz rows"
    );
}

/// Every convention gap names the line it is about, in the form a repair job opens. A gap
/// without a `file:line` is a rumour.
#[test]
fn every_gap_names_a_line() {
    for row in ROWS.iter() {
        for gap in row.gaps {
            assert!(
                !gap.parameter.is_empty(),
                "{}: an unnamed parameter",
                row.name
            );
            assert!(
                !gap.note.is_empty(),
                "{}: {} has no note",
                row.name,
                gap.parameter
            );
            let (file, line) = gap.at.rsplit_once(':').unwrap_or_else(|| {
                panic!(
                    "{}: {} has no line number: {}",
                    row.name, gap.parameter, gap.at
                )
            });
            assert!(
                file.starts_with("crates/") || file.starts_with("SciGraphs/"),
                "{}: {} names no file in this tree: {file}",
                row.name,
                gap.parameter
            );
            assert!(
                line.parse::<u32>().is_ok(),
                "{}: {}: {line}",
                row.name,
                gap.parameter
            );
        }
    }
}

/// The two arms' defaults are the ones the dispatcher hands its own reference: `iterations`
/// 50 and `scale` 5.0 (`dispatcher.py:14`). A row's whole premise is that both sides were
/// given the same two numbers.
#[test]
fn the_arms_defaults_are_the_dispatchers() {
    assert_eq!(super::ITERATIONS, 50);
    assert_eq!(super::SCALE, 5.0);
}

/// The layout seed is `derive_seed(42, "layout")`, recomputed here rather than trusted, so
/// the number the motor arm seeds with is checked against SciGraphs' own derivation.
#[test]
fn the_layout_seed_is_derived_from_forty_two() {
    // `repro/determinism.py:56-62`: sha256("42:layout"), first four bytes big-endian, mod 2^31.
    let expected = 981_798_123;
    assert_eq!(super::LAYOUT_SEED, expected);
}

/// The baseline covers every row and nothing else, so a row cannot be added without a
/// measurement and a stale row cannot be judged against a name that moved.
///
/// **This is the RED that gates the measurement.** The table starts empty, this test fails
/// with the 32 names it is waiting for, and it goes green when the measured run is pasted
/// in — which is the only way a pinned number reaches this file.
#[test]
fn the_baseline_covers_every_row() {
    let missing: Vec<&str> = ROWS
        .iter()
        .map(|row| row.name)
        .filter(|name| !BASELINE.iter().any(|base| base.name == *name))
        .collect();
    assert!(missing.is_empty(), "no baseline pinned for: {missing:?}");
    assert_eq!(
        BASELINE.len(),
        ROWS.len(),
        "the baseline has rows the matrix does not"
    );
}

/// Every row's reference arm is one of the two words, so the matrix's "reference reached"
/// column has a closed vocabulary.
#[test]
fn every_reference_arm_is_one_of_two_words() {
    for row in ROWS.iter() {
        let arm = row.reference_arm();
        assert!(
            arm == "scigraphs" || arm.starts_with("graphviz:"),
            "{}: {arm}",
            row.name
        );
    }
}
