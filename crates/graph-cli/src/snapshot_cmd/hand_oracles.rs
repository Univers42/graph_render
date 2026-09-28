//! The hand oracles `roundtrip` checks a snapshot against, one per layout that is gated
//! on `roundtrip` rather than a JS differential (`crate::capabilities::registry`):
//! [`grid`] (existing), [`circular`] and [`packing`] (Phase 3). Each restates its
//! layout's own stated convention independently of `graph-core`'s implementation and
//! compares bit for bit — the same shape as `grid`'s, just over a different formula.

use graph_contract::binary::Snapshot;
use graph_contract::geometry::{EdgeGeometry, NodeGeometry};
use graph_contract::notes::NoteCode;
use graph_core::layout::hierarchy::Hierarchy;
use graph_core::{REFERENCE_DEGREE, index_model, seeded_model};

/// The grid's conventions restated in f64, independently of graph-core's integer
/// `dimensions`: `cols = ceil(sqrt(n))`, `rows = ceil(n / cols)`, node `i` in cell
/// `(i mod cols, floor(i / cols))`, the lattice centred on the origin at unit spacing,
/// `Point` nodes and `Line` edges. Compared bit for bit.
pub fn grid(snapshot: &Snapshot) -> Result<(), String> {
    let p = snapshot.parts();
    let (NodeGeometry::Point { x, y }, EdgeGeometry::Line) = (&p.nodes, &p.edges) else {
        return Err("not Point nodes with Line edges".into());
    };
    let n = x.len() as f64;
    let cols = n.sqrt().ceil();
    let rows = (n / cols).ceil();
    for (i, (&gx, &gy)) in x.iter().zip(y).enumerate() {
        let (col, row) = (i as f64 % cols, (i as f64 / cols).floor());
        let want = (
            (col - (cols - 1.0) / 2.0) as f32,
            (row - (rows - 1.0) / 2.0) as f32,
        );
        if (gx.to_bits(), gy.to_bits()) != (want.0.to_bits(), want.1.to_bits()) {
            return Err(format!(
                "node {i} at ({gx}, {gy}), the conventions put it at {want:?}"
            ));
        }
    }
    Ok(())
}

/// `layout.circular.radial`'s convention restated independently: node `v`'s ring is
/// [`Hierarchy::depth`] (the shared, already-tested substrate — this restates the
/// layout's own arithmetic on top of it, not the repair itself); its angular slot is
/// ascending dense index within that ring; `radius = ring * 1.0`,
/// `angle = slot * 2*pi/count`, libm `sin`/`cos`. Rebuilds the seed's own model rather
/// than reading it back off the snapshot, which carries positions, not ring membership.
pub fn circular(seed: u32, nodes: u32, snapshot: &Snapshot) -> Result<(), String> {
    let p = snapshot.parts();
    let (NodeGeometry::Point { x, y }, EdgeGeometry::Line) = (&p.nodes, &p.edges) else {
        return Err("not Point nodes with Line edges".into());
    };
    let (records, edges) = seeded_model(seed, nodes, REFERENCE_DEGREE);
    let topology = index_model(&records, &edges).map_err(|e| format!("reindexing: {e}"))?;
    let hierarchy = Hierarchy::of(&topology).map_err(|e| format!("hierarchy: {e}"))?;
    let rings: Vec<u32> = (0..topology.node_count())
        .map(|v| hierarchy.depth(v))
        .collect();
    let width = rings.iter().max().map_or(0, |&d| d as usize + 1);
    let mut counts = vec![0u32; width];
    for &r in &rings {
        counts[r as usize] += 1;
    }
    let mut seen = vec![0u32; width];
    for (v, (&gx, &gy)) in x.iter().zip(y).enumerate() {
        let ring = rings[v];
        let slot = seen[ring as usize];
        seen[ring as usize] += 1;
        let count = counts[ring as usize];
        let radius = f64::from(ring);
        let angle = f64::from(slot) * (2.0 * std::f64::consts::PI) / f64::from(count);
        let want = (
            (radius * libm::cos(angle)) as f32,
            (radius * libm::sin(angle)) as f32,
        );
        if (gx.to_bits(), gy.to_bits()) != (want.0.to_bits(), want.1.to_bits()) {
            return Err(format!(
                "node {v} (ring {ring} slot {slot}/{count}) at ({gx}, {gy}), the convention puts it at {want:?}"
            ));
        }
    }
    Ok(())
}

/// `layout.packing.circle`'s promise restated: every circle finite with a positive
/// radius, always; full edge tangency, within the crate's own stated 1e-3 (f32) bound,
/// whenever note code 3 is absent — its absence is the only trustworthy sign the packing
/// is exact (module doc), so that is exactly what this hand oracle can honestly hold it
/// to. It does not re-verify a fallback packing's tangency: none is promised.
pub fn packing(snapshot: &Snapshot) -> Result<(), String> {
    let p = snapshot.parts();
    let (NodeGeometry::Circle { x, y, r }, EdgeGeometry::Line) = (&p.nodes, &p.edges) else {
        return Err("not Circle nodes with Line edges".into());
    };
    for (i, ((&cx, &cy), &cr)) in x.iter().zip(y).zip(r).enumerate() {
        if !(cx.is_finite() && cy.is_finite() && cr.is_finite() && cr > 0.0) {
            return Err(format!(
                "circle {i} at ({cx}, {cy}) r={cr}: not finite and positive"
            ));
        }
    }
    if p.notes
        .code
        .contains(&(NoteCode::PackingApproximate as u32))
    {
        return Ok(()); // the fallback: no tangency promised, per the module's own doc.
    }
    const TANGENCY: f32 = 1e-3;
    for (e, (&u, &v)) in p.source.iter().zip(&p.target).enumerate() {
        if u == v {
            continue;
        }
        let (u, v) = (u as usize, v as usize);
        let dist = ((x[u] - x[v]).powi(2) + (y[u] - y[v]).powi(2)).sqrt();
        let want = r[u] + r[v];
        if (dist - want).abs() > TANGENCY * want.max(1.0) {
            return Err(format!(
                "edge {e} ({u}, {v}): centres {dist} apart, radii sum to {want}"
            ));
        }
    }
    Ok(())
}
