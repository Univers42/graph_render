//! The gate model at one seed: `graph_core::seeded_model`, re-named dense so byte order is
//! list order, under the node-count cap the job states.

use super::{Fixture, GATE_CAP, names};
use graph_core::{REFERENCE_DEGREE, gate_node_count, seeded_model};

/// The gate's own model at one seed, re-named dense so byte order is list order.
pub(super) fn gate(seed: u32) -> Result<Fixture, String> {
    let count = gate_node_count(seed);
    if count > GATE_CAP {
        return Err(format!(
            "gate seed {seed} is {count} nodes, past the {GATE_CAP} cap"
        ));
    }
    let (mut nodes, mut edges) = seeded_model(seed, count, REFERENCE_DEGREE);
    let ids = names(nodes.len());
    // `seeded_model` names its nodes `n0..n{n}`, whose byte order is *not* its numeric order
    // past nine — so the ends are remapped through the original list rather than by string
    // surgery, and an end that is not a node is an error rather than a dangling edge.
    let original: Vec<String> = nodes.iter().map(|n| n.id.clone()).collect();
    for (record, id) in nodes.iter_mut().zip(&ids) {
        record.id = id.clone();
    }
    for (index, record) in edges.iter_mut().enumerate() {
        record.id = format!("e{index:05}");
        record.source = seat(&original, &ids, &record.source)?;
        record.target = seat(&original, &ids, &record.target)?;
    }
    Ok(Fixture {
        // Bounded by `GATE_SEEDS` and never by a caller's loop: 20 leaked names per emit.
        name: leak(format!("gate-{seed:02}")),
        about: leak(format!("the gate model at seed {seed}, {count} nodes")),
        nodes,
        edges,
    })
}

/// A `&'static str` out of a formatted value, because [`Fixture::name`] is `&'static str`
/// and a `Box::leak` is the honest way to say it: bounded by the seed list above.
fn leak(text: String) -> &'static str {
    Box::leak(text.into_boxed_str())
}

/// An edge end's new name, from the id the gate model gave it.
fn seat(original: &[String], ids: &[String], end: &str) -> Result<String, String> {
    original
        .iter()
        .position(|candidate| candidate == end)
        .map(|at| ids[at].clone())
        .ok_or_else(|| format!("edge end {end} is not a node of its fixture"))
}
