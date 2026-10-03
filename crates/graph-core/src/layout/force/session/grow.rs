//! Growing a running session **in place** onto the topology it was built over, after
//! [`Topology::extend`] appended a batch to it: O(batch) where a [`carry`](ForceSession::carry)
//! rebuilds the whole session.
//!
//! The result is the bytes `carry` gives for the same two topologies, field for field
//! (`session/tests/grow.rs`), because an extended topology keeps every row and every edge
//! index it had: a node of the old topology is the same row of the new one, so the carry's
//! id map is the identity on `0..rows` and every row past it is new. What a carry rebuilds,
//! this appends:
//!
//! | Carry | Grow |
//! |---|---|
//! | `simple_graph(to)` | the new raw edges absorbed, in edge order, first pair wins |
//! | `link::geometry` over every edge | [`edge_geometry`] over the edges whose degrees moved |
//! | `Placement` over every row | [`beside_carried`] over the new rows, else the spiral |
//! | fresh zeroed `vx`, `vy`, `px`, `py` and free pins | the same, pushed for new rows |
//! | `Mesh::new(rows)` | `Mesh::new(rows)` |

use super::carry::beside_carried;
use super::{ForceSession, SessionError};
use crate::arena::CapacityError;
use crate::csr::AppendCsr;
use crate::index::Topology;
use crate::layout::force::SimpleGraph;
use crate::layout::force::barnes_hut::link::edge_geometry;
use crate::layout::force::barnes_hut::sim::{Sim, spiral_point};
use crate::layout::force::particle_mesh::Mesh;

impl ForceSession {
    /// This session grown onto `topology`, which must be the topology it is over with
    /// zero or more batches appended by [`Topology::extend`]: rows `rows..` and raw edges
    /// `absorbed..` are taken in, and nothing before them is read again.
    ///
    /// Equal, bit for bit, to `self.carry(previous, topology)`: carried rows keep their
    /// position, velocity and pins; new rows start at rest where a carry places them; the
    /// run keeps its heat, tick count and parameters; a particle-mesh session gets a fresh
    /// mesh, as a carried one does.
    ///
    /// Refused, with nothing changed, by [`SessionError::ColumnLength`] (column `grow`) when
    /// `topology` has fewer nodes than the session's rows or fewer edges than it absorbed,
    /// and by [`SessionError::Capacity`] when the new edges could overflow the adjacency.
    /// A topology that is not an extension of this session's is not detected.
    pub fn grow(&mut self, topology: &Topology) -> Result<(), SessionError> {
        self.check_growth(topology)?;
        let (old_rows, rows) = (self.rows(), topology.node_count());
        let sim = &mut self.sim;
        for _ in old_rows..rows {
            sim.graph.rows.push_row().map_err(SessionError::Capacity)?;
        }
        let touched =
            absorb(&mut sim.graph, topology, self.absorbed).map_err(SessionError::Capacity)?;
        relink(sim, &touched);
        place(sim, old_rows, rows);
        for scratch in [&mut sim.px, &mut sim.py] {
            scratch.clear();
            scratch.resize(rows as usize, 0.0);
        }
        if self.mesh.is_some() {
            self.mesh = Some(Mesh::new(rows));
        }
        self.absorbed = topology.edge_count();
        Ok(())
    }

    /// Every refusal, before anything is written. Past it no append can fail: the simple
    /// graph's rows gain at most two values per new raw edge.
    fn check_growth(&self, topology: &Topology) -> Result<(), SessionError> {
        let (rows, nodes, edges) = (self.rows(), topology.node_count(), topology.edge_count());
        let short = |got: u32, held: u32| SessionError::ColumnLength {
            column: "grow",
            got: u64::from(got),
            nodes: held,
        };
        if nodes < rows {
            return Err(short(nodes, rows));
        }
        if edges < self.absorbed {
            return Err(short(edges, self.absorbed));
        }
        let live = self.sim.graph.rows.len() as u64 + 2 * u64::from(edges - self.absorbed);
        if live > AppendCsr::SAFE_LIVE {
            return Err(SessionError::Capacity(CapacityError {
                what: "force simple graph",
            }));
        }
        Ok(())
    }
}

/// Raw edges `from..` of `topology` taken into `graph` in edge order, and the endpoints of
/// every simple edge they added: the nodes whose degree moved, ascending, each once.
fn absorb(
    graph: &mut SimpleGraph,
    topology: &Topology,
    from: u32,
) -> Result<Vec<u32>, CapacityError> {
    let raw = topology.edges();
    let mut touched = Vec::new();
    for e in from as usize..topology.edge_count() as usize {
        if let Some(simple) = graph.absorb(raw.source[e], raw.target[e], raw.strength[e])? {
            touched.extend([graph.lo[simple as usize], graph.hi[simple as usize]]);
        }
    }
    touched.sort_unstable();
    touched.dedup();
    Ok(touched)
}

/// The link geometry of every edge at a node in `touched`: the new edges, and every old
/// edge whose bias read a degree that moved. No other edge's inputs changed.
///
/// **Caveat:** the cost is the degree of each touched node, not the batch: one edge added
/// to a hub of degree `d` recomputes `d` edges. The values are the ones a full
/// `link::geometry` gives; only the work is degree-proportional.
fn relink(sim: &mut Sim, touched: &[u32]) {
    let m = sim.graph.lo.len();
    for column in [
        &mut sim.link_distance,
        &mut sim.link_strength,
        &mut sim.link_bias,
    ] {
        column.resize(m, 0.0);
    }
    for &v in touched {
        for &e in sim.graph.rows.row(v) {
            let (at, (d, s, b)) = (
                e as usize,
                edge_geometry(&sim.graph, &sim.params, e as usize),
            );
            (
                sim.link_distance[at],
                sim.link_strength[at],
                sim.link_bias[at],
            ) = (d, s, b);
        }
    }
}

/// Rows `old_rows..rows`, ascending: beside their carried neighbours (rows below
/// `old_rows`, whose positions grow never writes) or on the seed spiral, at rest and free.
fn place(sim: &mut Sim, old_rows: u32, rows: u32) {
    for (fresh, row) in (0_u32..).zip(old_rows..rows) {
        let (x, y) = (&sim.x, &sim.y);
        let carried_at = |v: u32| (v < old_rows).then(|| (x[v as usize], y[v as usize]));
        let at = beside_carried(&sim.graph, row, fresh, carried_at);
        let (px, py) = at.unwrap_or_else(|| spiral_point(row));
        sim.x.push(px);
        sim.y.push(py);
    }
    let n = rows as usize;
    sim.vx.resize(n, 0.0);
    sim.vy.resize(n, 0.0);
    sim.fx.resize(n, None);
    sim.fy.resize(n, None);
}
