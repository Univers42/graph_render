//! The façade against the paths the gates already trust: the hash gate's native arm
//! (`graph_core::run_with`), the handle table `gm_post_run` writes through, and the
//! contract's own derivation. Byte equality, not agreement in shape.

use super::*;
use crate::contract::tests::{EXPECTED, member};
use crate::handle::{Handle, Handles};
use crate::seed_ingest;
use graph_core::{REFERENCE_DEGREE, gate_node_count, run_with, seeded_model};

/// Small enough to run every layout in a debug build, and three sizes apart: 2, 9 and 42
/// nodes (`gate_node_count`).
const SEEDS: [u32; 3] = [0, 7, 40];

fn built(seed: u32) -> Topology {
    let text = seed_ingest::for_seed(seed).expect("the seed model is finite");
    build(text.as_bytes(), Source::Ingest).expect("the seed's ingest builds")
}

/// What the hash gate's native arm hashes for `layout` at `seed`, as `run` reports it.
fn gate_bytes(seed: u32, layout: &graph_core::registry::Capability) -> Result<Vec<u8>, Code> {
    let (nodes, edges) = seeded_model(seed, gate_node_count(seed), REFERENCE_DEGREE);
    run_with(&nodes, &edges, layout.id, layout.run)
        .map(|run| run.snapshot.to_bytes())
        .map_err(|_| Code::LayoutFailed)
}

#[test]
fn every_layout_gives_the_hash_gates_bytes() {
    for seed in SEEDS {
        let topology = built(seed);
        for layout in &LAYOUTS {
            assert_eq!(
                run(&topology, layout.id, None),
                gate_bytes(seed, layout),
                "{} at seed {seed}",
                layout.id
            );
        }
    }
}

/// A handle laid out by `layout` the way `gm_run` leaves one: the geometry and its
/// snapshot when both succeed, neither when either fails.
fn laid_out(handles: &mut Handles, seed: u32, layout: &graph_core::registry::Capability) -> u32 {
    let topology = built(seed);
    let kept = (layout.run)(&topology).ok().and_then(|geometry| {
        let snapshot = graph_core::layout::snapshot(&topology, geometry.clone()).ok()?;
        Some((geometry, snapshot))
    });
    let (geometry, snapshot) = kept.unzip();
    let handle = Handle {
        topology,
        snapshot,
        geometry,
    };
    handles.insert(handle).expect("the table has room")
}

#[test]
fn a_post_pass_gives_the_bytes_gm_post_run_leaves_on_the_handle() {
    let mut handles = Handles::new();
    for layout in &LAYOUTS {
        let handle = laid_out(&mut handles, 7, layout);
        let topology = built(7);
        for (index, id) in post_ids().enumerate() {
            let index = u32::try_from(index).expect("eight posts");
            let exported = crate::stage_exports::post_run(&mut handles, handle, index)
                .map(Snapshot::to_bytes)
                .map_err(|code| match code {
                    Code::NoGeometryYet => Code::LayoutFailed,
                    other => other,
                });
            let ran = run(&topology, layout.id, Some(id));
            assert_eq!(ran, exported, "{} then {id}", layout.id);
        }
    }
}

#[test]
fn the_contract_source_gives_its_derivations_bytes() {
    let document = member(EXPECTED, "ingest");
    let (_, derived) = crate::contract::derive(document.as_bytes()).expect("the fixture derives");
    let topology = build(document.as_bytes(), Source::Contract).expect("the fixture builds");
    let layout = registry::find("layout.grid").expect("registered");
    let geometry = (layout.run)(&derived).expect("grid lays out the fixture");
    let expected = graph_core::layout::snapshot(&derived, geometry).expect("finite");
    assert_eq!(run(&topology, layout.id, None), Ok(expected.to_bytes()));
}

#[test]
fn each_source_refuses_the_others_document_under_its_own_code() {
    let contract = member(EXPECTED, "ingest");
    let provisional = seed_ingest::for_seed(7).expect("finite");
    let refused = |bytes: &[u8], source| build(bytes, source).err();
    assert_eq!(
        refused(contract.as_bytes(), Source::Ingest),
        Some(Code::IngestInvalid)
    );
    assert_eq!(
        refused(provisional.as_bytes(), Source::Contract),
        Some(Code::ContractInvalid)
    );
    assert_eq!(refused(b"\xff", Source::Ingest), Some(Code::IngestInvalid));
    assert_eq!(
        refused(b"\xff", Source::Contract),
        Some(Code::ContractInvalid)
    );
}

#[test]
fn an_unknown_id_is_refused_before_anything_runs() {
    let topology = built(7);
    let grid = "layout.grid";
    assert_eq!(
        run(&topology, "layout.none", None),
        Err(Code::UnknownLayoutId)
    );
    assert_eq!(
        run(&topology, "layout.none", Some("post.none")),
        Err(Code::UnknownLayoutId)
    );
    assert_eq!(
        run(&topology, grid, Some("post.none")),
        Err(Code::IndexOutOfRange)
    );
    assert_eq!(run(&topology, grid, Some("")), Err(Code::IndexOutOfRange));
}

#[test]
fn the_id_lists_are_the_registries_in_their_order() {
    let layouts: Vec<&str> = LAYOUTS.iter().map(|layout| layout.id).collect();
    assert_eq!(layout_ids().collect::<Vec<_>>(), layouts);
    let posts: Vec<&str> = (0..crate::post::count())
        .filter_map(crate::post::id_at)
        .collect();
    assert_eq!(post_ids().collect::<Vec<_>>(), posts);
    assert_eq!(posts.len(), crate::post::CAPABILITIES.len());
}
