//! Carrying a running session onto a **new topology**: the P4 growth path, where the
//! graph gains nodes and edges while the layout is already live.
//!
//! # Why a carry and not a restart
//!
//! A layout under a live process is not a function of the current graph — it is the whole
//! trajectory of the process so far. Re-seeding on every emission would throw that away
//! and put the picture back on the golden spiral (`barnes_hut/seed.rs`) each time, which is
//! exactly the flicker the layout continuing instead of restarting exists to avoid. So a
//! carry moves the state across and keeps it: every surviving node keeps its position, its
//! velocity and its pins, and the run keeps its heat, its tick count and its parameters.
//!
//! # What crosses, and how
//!
//! **Only ids cross.** A dense row means nothing in a topology this session was not built
//! over, so [`Topology::node_index`] maps `to`'s rows back to `from`'s rows by id and nothing
//! else. The two id spaces are separate [`StringArena`](crate::arena::StringArena)s, and the
//! lookup is by string content, which is what makes it valid across them.
//!
//! | A node of `to` | Where it starts |
//! |---|---|
//! | its id is a row of `from` | exactly where it was: `x`, `y`, `vx`, `vy` and both pins |
//! | new, with a carried neighbour | the mean of its carried neighbours plus a phyllotaxis offset |
//! | new, with none | the golden-spiral point of its row in `to` |
//!
//! A new node starts **at rest**: `Sim::new` leaves every velocity at zero and a carry only
//! writes the columns of carried rows, so the columns a new node gets are the zeros it was
//! born with. Inventing a velocity for a node that has never moved would make the result
//! depend on the graph change rather than on the run.
//!
//! # The offset, and why it is this small
//!
//! [`OFFSET_RADIUS`] is one layout unit: the seed spiral's own first point is 12 units out
//! and `link_distance` is 60, so one unit lands a new node *inside* the cluster it belongs
//! to instead of displacing it, and two leaves of one hub land 2·sin(φ/2) = 1.86 units
//! apart at the golden angle — never coincident, which is the only thing the offset has to
//! guarantee.
//!
//! # Order
//!
//! Every walk is ascending and every reduction is over a fixed sequence: rows ascend, and a
//! row's neighbours are its simple graph's row, which `simple_graph` files in edge-index
//! order. No hash order reaches the output (D3), so the same two topologies carry to the
//! same bytes on every target.

use super::ForceSession;
use super::error::SessionError;
use crate::index::Topology;
use crate::layout::force::barnes_hut::sim::Sim;
use crate::layout::force::particle_mesh::Mesh;

/// The same constant `barnes_hut/seed.rs` spirals on, so the offset a new node gets is the
/// seed's own angle rather than a second one that happens to look similar.
const GOLDEN_ANGLE: f64 = 2.399963229728653;

/// How far from its neighbours' mean a new node with carried neighbours starts. See the
/// module's "The offset, and why it is this small".
const OFFSET_RADIUS: f64 = 1.0;

impl ForceSession {
    /// This session, carried from `from` — the topology it was built over — onto `to`.
    ///
    /// The returned session is a **new** one: `self` is untouched and both may be stepped.
    /// It starts with `self`'s `alpha`, `alpha_target`, `tick_no` and parameters, every
    /// node of `to` whose id is a row of `from` at its own position, velocity and pins, and
    /// every node of `to` that `from` did not have placed by the rule in the module's table.
    ///
    /// Refused — with nothing built — when `from` is not the topology this session is over,
    /// which is [`SessionError::ColumnLength`] against the session's own row count: the two
    /// id spaces cannot be compared at all when one of them has the wrong number of rows.
    /// `carrying onto a smaller topology is not refused`: dropping nodes is a shrink, and
    /// the survivors keep their bytes.
    pub fn carry(&self, from: &Topology, to: &Topology) -> Result<ForceSession, SessionError> {
        carried(self, from, to)
    }
}

/// The whole carry, after the refusal: build `to`'s session on the seed spiral, hand it
/// `self`'s run state, then one ascending pass over `to`'s rows places each of them.
fn carried(
    session: &ForceSession,
    from: &Topology,
    to: &Topology,
) -> Result<ForceSession, SessionError> {
    let rows = session.rows();
    let got = from.node_count();
    if got != rows {
        return Err(SessionError::ColumnLength {
            column: "from",
            got: u64::from(got),
            nodes: rows,
        });
    }
    let mut out = ForceSession::seeded(to, session.sim.params);
    let ForceSession { sim, deltas, mesh } = &mut out;
    // The carried session keeps its engine; the mesh's grids are sized to the new rows.
    *mesh = session.mesh.as_ref().map(|_| Mesh::new(sim.rows()));
    sim.alpha = session.sim.alpha;
    sim.alpha_target = session.sim.alpha_target;
    sim.tick_no = session.sim.tick_no;
    deltas.reserve(rows as usize);
    Placement {
        out: sim,
        old: &session.sim,
        to,
        carried: Placement::map(from, to),
    }
    .all();
    Ok(out)
}

/// One carry's placement state: where rows are being written, and the two topologies and
/// the run being read. A struct rather than five parameters, so the reads stay named.
struct Placement<'a> {
    out: &'a mut Sim,
    old: &'a Sim,
    to: &'a Topology,
    /// `to`'s row → the row of `from` naming the same node, or [`NEW`] for a node `from`
    /// did not have.
    ///
    /// Built once, ascending, before anything is placed. It is the whole id mapping of the
    /// carry, and it is an array rather than a lookup per question because the questions are
    /// not one per row: [`carried_mean`](Self::carried_mean) asks once per incident edge,
    /// which is an `O(m)` hash of an id string per batch instead of an `O(n)` array read.
    /// Sized once at the row count and never grown — no per-node allocation (D6).
    carried: Vec<u32>,
}

/// The `carried` entry for a node `from` did not have. A row number, so it can never be one.
const NEW: u32 = u32::MAX;

impl Placement<'_> {
    /// Builds the row map: one id lookup per row of `to`, ascending, against `from`.
    ///
    /// The id is read out of the column rather than through [`Topology::node`], which
    /// builds a [`NodeView`](crate::records::NodeView) of all ten fields to hand back one
    /// of them: at 1M rows that is ten reads a row this pass does not want.
    fn map(from: &Topology, to: &Topology) -> Vec<u32> {
        let strings = to.strings();
        let ids = &to.nodes().id;
        (0..ids.len())
            .map(|row| from.node_index(strings.get(ids[row])).unwrap_or(NEW))
            .collect()
    }

    /// Every row of `to`, ascending. `fresh` counts the new rows, and is the index the
    /// offset spirals on — a count over rows in order, so it does not depend on which rows
    /// happen to be new in which order.
    fn all(&mut self) {
        let mut fresh = 0_u32;
        for row in 0..self.to.node_count() {
            let old = self.carried[row as usize];
            if old == NEW {
                self.place_new(row, fresh);
                fresh += 1;
            } else {
                self.copy(old, row);
            }
        }
    }

    /// The row of `from` holding the node `to`'s row `row` names, or `None` when `from` has
    /// no such node. The only mapping between the two topologies.
    fn carried_row(&self, row: u32) -> Option<u32> {
        let old = self.carried[row as usize];
        (old != NEW).then_some(old)
    }

    /// The six columns a surviving node keeps, field for field: position, velocity and both
    /// pins. Nothing else in the old columns refers to a row of `from`.
    fn copy(&mut self, old: u32, row: u32) {
        let (row, old) = (row as usize, old as usize);
        self.out.x[row] = self.old.x[old];
        self.out.y[row] = self.old.y[old];
        self.out.vx[row] = self.old.vx[old];
        self.out.vy[row] = self.old.vy[old];
        self.out.fx[row] = self.old.fx[old];
        self.out.fy[row] = self.old.fy[old];
    }

    /// A new row: beside its carried neighbours when it has any, and where the seed already
    /// put it when it has none.
    fn place_new(&mut self, row: u32, fresh: u32) {
        let Some((mean_x, mean_y)) = self.carried_mean(row) else {
            // `Sim::new` seeded this row on the golden spiral, and that spiral point *is*
            // the golden-spiral position of row `row` in `to` — the same function, the same
            // row. Writing it again would only be a second spelling of it.
            return;
        };
        let angle = f64::from(fresh) * GOLDEN_ANGLE;
        let at = row as usize;
        self.out.x[at] = mean_x + OFFSET_RADIUS * libm::cos(angle);
        self.out.y[at] = mean_y + OFFSET_RADIUS * libm::sin(angle);
    }

    /// The mean position of `row`'s **carried** neighbours in `to`, or `None` when it has
    /// none. Read from `old` by `from`'s row, never from `out`: a carried neighbour further
    /// along `to` has not been copied yet when an earlier new row asks for it, so its
    /// position is only trustworthy in the columns the run already had.
    ///
    /// The walk is the new graph's own simple row — deduplicated, self-loops dropped, in
    /// edge-index order — so a parallel edge weighs once and the sum is over a fixed
    /// sequence.
    fn carried_mean(&self, row: u32) -> Option<(f64, f64)> {
        let (mut sum_x, mut sum_y, mut count) = (0.0_f64, 0.0_f64, 0_u32);
        for &e in self.out.graph.rows.row(row) {
            let Some(old) = self.carried_row(self.out.graph.other(e, row)) else {
                continue;
            };
            sum_x += self.old.x[old as usize];
            sum_y += self.old.y[old as usize];
            count += 1;
        }
        (count > 0).then(|| (sum_x / f64::from(count), sum_y / f64::from(count)))
    }
}
