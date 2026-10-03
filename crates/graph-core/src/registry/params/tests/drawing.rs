//! What a published parameter does to the drawing: one parameter moved changes the bytes,
//! a value out of range is refused by name, a layout that publishes nothing draws an empty
//! buffer and refuses the rest, and the schema encodes to the length it claims.
//! `docs/decisions/layout-params.md`.

use crate::registry::{Capability, LAYOUTS, find};
use crate::stage::{StageError, gate_node_count, run_with, seeded_model};
use crate::weights::REFERENCE_DEGREE;

/// The determinism claim, as bytes: a run at the published defaults is the run the hash
/// gate is pinned to, so `hashgate --seeds 8` cannot move because a schema was added.
#[test]
fn a_run_at_the_published_defaults_is_the_registered_run() {
    let (nodes, edges) = model();
    for layout in &LAYOUTS {
        let defaults = buffer(&layout.params().defaults());
        assert_eq!(
            defaults.len(),
            layout.params().buffer_len(),
            "{}: the default buffer's own length",
            layout.id
        );
        let at_defaults = run_with(&nodes, &edges, layout.id, |t| {
            layout.run_params(t, &defaults)
        })
        .expect("runs at its defaults");
        let registered = run_with(&nodes, &edges, layout.id, layout.run).expect("runs");
        assert_eq!(
            at_defaults.snapshot.to_bytes(),
            registered.snapshot.to_bytes(),
            "{}: the published defaults are not this layout's Default",
            layout.id
        );
    }
}

/// One published parameter, moved, changes the drawing — one row per layout that
/// publishes anything. Selected by *name*, not by index, so reordering a schema does not
/// silently move the value onto a different parameter.
const MOVES_THE_DRAWING: &[(&str, &str, f64)] = &[
    ("layout.grid", "spacing", 4.0),
    ("layout.dag.sugiyama", "layer_spacing", 3.0),
    ("layout.packing.circle", "scale", 64.0),
    ("layout.force.spring", "iterations", 3.0),
    ("layout.force.spring3d", "iterations", 3.0),
    ("layout.forceatlas2", "max_iter", 3.0),
    ("layout.forceatlas2.barnes_hut", "max_iter", 3.0),
    ("layout.force.fruchterman_reingold", "niter", 3.0),
    ("layout.force.kamada_kawai", "epsilon", 1.0),
    ("layout.force.graphopt", "niter", 3.0),
    ("layout.force.davidson_harel", "maxiter", 3.0),
    ("layout.force.lgl", "maxit", 3.0),
    ("layout.force.drl", "seed", 12_345.0),
];

#[test]
fn one_published_parameter_moves_the_drawing_on_every_layout_that_publishes_one() {
    let publishing: Vec<&Capability> = LAYOUTS
        .iter()
        .filter(|l| !l.params.specs.is_empty())
        .collect();
    assert_eq!(
        publishing.len(),
        MOVES_THE_DRAWING.len(),
        "every layout that publishes a parameter has a row here, and no row names a layout \\
         that publishes none"
    );
    let (nodes, edges) = model();
    for (id, name, value) in MOVES_THE_DRAWING {
        let layout = find(id).unwrap_or_else(|| panic!("{id} is registered"));
        let index = layout
            .params
            .specs
            .iter()
            .position(|spec| spec.name == *name)
            .unwrap_or_else(|| panic!("{id} publishes {name}"));
        let mut values = layout.params().defaults();
        values[index] = *value;
        assert!(
            layout.params().validate(&buffer(&values)).is_ok(),
            "{id}: {name}={value} is in range"
        );
        let moved = run_with(&nodes, &edges, id, |t| {
            layout.run_params(t, &buffer(&values))
        })
        .expect("runs");
        let registered = run_with(&nodes, &edges, id, layout.run).expect("runs");
        assert_ne!(
            moved.snapshot.to_bytes(),
            registered.snapshot.to_bytes(),
            "{id}: moving {name} changed nothing"
        );
    }
}

/// A value out of range is refused, and the refusal names the parameter. Never clamped:
/// a drawing that is not the one asked for is worse than no drawing.
#[test]
fn a_value_out_of_range_is_refused_and_never_clamped() {
    let (nodes, edges) = model();
    for (id, name, _) in MOVES_THE_DRAWING {
        let layout = find(id).unwrap();
        let index = layout
            .params
            .specs
            .iter()
            .position(|spec| spec.name == *name)
            .unwrap();
        let mut values = layout.params().defaults();
        values[index] = layout.params.specs[index].max + 1.0;
        let refused = run_with(&nodes, &edges, id, |t| {
            layout.run_params(t, &buffer(&values))
        });
        match refused {
            Err(StageError::Param { name: got, .. }) => assert_eq!(got, *name, "{id}"),
            other => panic!("{id}: {name} out of range was {other:?}, not a refusal"),
        }
        // And an in-range value is accepted, so the refusal above is about the value and
        // not about the buffer.
        values[index] = layout.params.specs[index].default;
        assert!(
            run_with(&nodes, &edges, id, |t| layout
                .run_params(t, &buffer(&values)))
            .is_ok()
        );
    }
}

/// A layout that publishes nothing: an empty buffer draws it, a non-empty one is refused
/// rather than silently dropped. This is the shape twenty-six of the thirty-nine rows have.
#[test]
fn a_layout_that_publishes_nothing_draws_an_empty_buffer_and_refuses_the_rest() {
    let (nodes, edges) = model();
    let quiet: Vec<&Capability> = LAYOUTS
        .iter()
        .filter(|l| l.params.specs.is_empty())
        .collect();
    assert_eq!(quiet.len(), LAYOUTS.len() - MOVES_THE_DRAWING.len());
    for layout in &quiet {
        assert_eq!(
            layout.params().encode().len(),
            4,
            "{}: an empty schema is a u32 count",
            layout.id
        );
        let drawn =
            run_with(&nodes, &edges, layout.id, |t| layout.run_params(t, &[])).expect("runs");
        let registered = run_with(&nodes, &edges, layout.id, layout.run).expect("runs");
        assert_eq!(
            drawn.snapshot.to_bytes(),
            registered.snapshot.to_bytes(),
            "{}",
            layout.id
        );
        let refused = run_with(&nodes, &edges, layout.id, |t| {
            layout.run_params(t, &[0u8; 8])
        });
        assert!(
            matches!(refused, Err(StageError::Param { .. })),
            "{}: a non-empty buffer was {:?}",
            layout.id,
            refused.err()
        );
    }
}

/// The schema reaches a caller as `ParamsView::encode`'s bytes, and the whole registry's
/// encodes without a length that disagrees with its own count.
#[test]
fn every_published_schema_encodes_to_the_length_it_claims() {
    for layout in &LAYOUTS {
        let bytes = layout.params().encode();
        let count = u32::from_le_bytes(bytes[..4].try_into().unwrap()) as usize;
        assert_eq!(count, layout.params.specs.len(), "{}", layout.id);
        assert!(
            bytes.len() > 4 || count == 0,
            "{}: a non-empty list has a body",
            layout.id
        );
        assert_eq!(layout.params().defaults().len(), count, "{}", layout.id);
    }
}

fn buffer(values: &[f64]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

fn model() -> (
    Vec<crate::records::NodeRecord>,
    Vec<crate::records::EdgeRecord>,
) {
    seeded_model(6, gate_node_count(6), REFERENCE_DEGREE)
}
