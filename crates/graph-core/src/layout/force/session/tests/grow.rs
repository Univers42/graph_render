//! `grow_equals_carry`: a session grown in place after `Topology::extend` is, bit for bit,
//! the session `carry` builds from the same two topologies, on both engines, right after
//! the batch and again after the two have run on.

use crate::index::{Topology, index_model};
use crate::layout::force::barnes_hut::sim::Sim;
use crate::layout::force::session::{ForceSession, LiveParams, NodeRow, SessionError};
use crate::records::build::{edge, node};
use crate::records::{EdgeRecord, NodeRecord};
use core::ops::Range;

type Batch = (Vec<NodeRecord>, Vec<EdgeRecord>);

/// How long both sessions run after each batch before they are compared again.
const TICKS: u32 = 50;

fn link(id: u32, source: u32, target: u32, strength: f64) -> EdgeRecord {
    let (s, t) = (format!("n{source}"), format!("n{target}"));
    EdgeRecord {
        strength,
        ..edge(&format!("e{id}"), &s, &t)
    }
}

fn nodes(rows: Range<u32>) -> Vec<NodeRecord> {
    rows.map(|i| node(&format!("n{i}"), "")).collect()
}

/// A ring, then: a hub gaining 200 leaves; parallel edges (one reversed, strengths that
/// differ) and self-loops, on old and new nodes; an empty batch; a batch taking the node
/// count from 213 across 256 to 300, with new rows whose only neighbours are new.
fn stream() -> Vec<Batch> {
    let ring = (0..12).map(|i| link(i, i, (i + 1) % 12, 0.5)).collect();
    let hub = (0..200).map(|k| link(12 + k, 0, 12 + k, 0.3)).collect();
    let multi = vec![
        link(212, 1, 2, 0.9),
        link(213, 2, 1, 0.1),
        link(214, 5, 5, 0.4),
        link(215, 212, 212, 0.4),
        link(216, 212, 3, 0.8),
        link(217, 3, 212, 0.2),
        link(218, 212, 0, 0.6),
    ];
    let to = |k: u32| if k % 5 == 1 { k - 1 } else { (k * 37) % 213 };
    let wide = (213..300)
        .filter(|k| k % 5 != 0)
        .map(|k| link(6 + k, k, to(k), 0.5));
    vec![
        (nodes(0..12), ring),
        (nodes(12..212), hub),
        (nodes(212..213), multi),
        (Vec::new(), Vec::new()),
        (nodes(213..300), wide.collect()),
    ]
}

/// A session hot enough to be worth comparing, as `tests/carry.rs`'s `live`.
fn live(topology: &Topology, mesh: bool) -> ForceSession {
    let mut session = ForceSession::new(topology, LiveParams::default()).expect("in range");
    if mesh {
        session = session.with_particle_mesh();
    }
    session.step(3);
    session.reheat(1.0).expect("1.0 is in range");
    session.step(10);
    session
}

/// Every column the brief names, by bit pattern, and the simple graph they were read from.
fn assert_same(a: &ForceSession, b: &ForceSession, what: &str) {
    let (a, b) = (&a.sim, &b.sim);
    let bits = |c: &[f64]| c.iter().map(|v| v.to_bits()).collect::<Vec<_>>();
    let columns = [
        ("x", &a.x, &b.x),
        ("y", &a.y, &b.y),
        ("vx", &a.vx, &b.vx),
        ("vy", &a.vy, &b.vy),
        ("link_distance", &a.link_distance, &b.link_distance),
        ("link_strength", &a.link_strength, &b.link_strength),
        ("link_bias", &a.link_bias, &b.link_bias),
        ("simple strength", &a.graph.strength, &b.graph.strength),
    ];
    for (name, left, right) in columns {
        assert_eq!(bits(left), bits(right), "{what}: {name}");
    }
    let pins = |c: &[Option<f64>]| c.iter().map(|v| v.map(f64::to_bits)).collect::<Vec<_>>();
    assert_eq!(pins(&a.fx), pins(&b.fx), "{what}: fx");
    assert_eq!(pins(&a.fy), pins(&b.fy), "{what}: fy");
    let run = |s: &Sim| (s.alpha.to_bits(), s.alpha_target.to_bits(), s.tick_no);
    assert_eq!(run(a), run(b), "{what}: alpha, alpha_target, tick_no");
    assert_eq!(
        (&a.graph.lo, &a.graph.hi),
        (&b.graph.lo, &b.graph.hi),
        "{what}: edges"
    );
}

/// Grows one session through the stream, comparing it after each batch with the carry of
/// the session it was just before. Row 3 is pinned before the third batch.
fn grows_as_it_carries(mesh: bool) {
    let batches = stream();
    let mut topology = index_model(&batches[0].0, &batches[0].1).expect("fits");
    let mut grown = live(&topology, mesh);
    for (k, (nodes, edges)) in batches.iter().enumerate().skip(1) {
        if k == 2 {
            grown
                .pin(NodeRow::new(3), -250.0, 175.0)
                .expect("row 3 exists");
        }
        let previous = topology.clone();
        topology.extend(nodes, edges).expect("a strict batch");
        let mut carried = grown.carry(&previous, &topology).expect("over previous");
        grown.grow(&topology).expect("an extension");
        let what = format!("mesh {mesh}, batch {k}");
        assert_same(&grown, &carried, &what);
        grown.step(TICKS);
        carried.step(TICKS);
        assert_same(&grown, &carried, &format!("{what}, {TICKS} ticks on"));
    }
    assert_eq!(grown.xs().len(), 300);
}

#[test]
fn grow_equals_carry() {
    grows_as_it_carries(false);
    grows_as_it_carries(true);
}

#[test]
fn grow_refuses_fewer_nodes_or_edges_and_changes_nothing() {
    let batches = stream();
    let all_nodes: Vec<_> = batches[..2].iter().flat_map(|b| b.0.clone()).collect();
    let all_edges: Vec<_> = batches[..2].iter().flat_map(|b| b.1.clone()).collect();
    let all = index_model(&all_nodes, &all_edges).expect("fits");
    let (mut session, twin) = (live(&all, false), live(&all, false));
    let fewer_nodes = index_model(&batches[0].0, &batches[0].1).expect("fits");
    let fewer_edges = index_model(&all_nodes, &batches[0].1).expect("fits");
    for (smaller, got) in [(&fewer_nodes, 12), (&fewer_edges, 12)] {
        let refused = SessionError::ColumnLength {
            column: "grow",
            got,
            nodes: 212,
        };
        assert_eq!(session.grow(smaller), Err(refused));
        assert_same(&session, &twin, "after a refusal");
    }
    session
        .grow(&all)
        .expect("the same topology is an empty extension");
    assert_same(&session, &twin, "after an empty growth");
}
