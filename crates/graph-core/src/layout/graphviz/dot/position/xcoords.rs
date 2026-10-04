//! `set_xcoords` (`position.c:571-586`): move the second simplex's answer out of `ND_rank`
//! and into `ND_coord`, and put the rank number back.
//!
//! While the x simplex runs, `ND_rank` **is** the x coordinate — an integer number of points to
//! the right of where the drawing starts. This pass is the swap back: `coord.x` takes the
//! value and `rank` takes the rank index again, so every later pass that asks a node which rank
//! it is on gets a rank and not a column.
//!
//! The walk is rank by rank and left to right within a rank, which is the order the rows were
//! built in, so the assignment is a single ordered pass and node `i`'s coordinate is decided
//! before node `i + 1`'s (D10, in the sense that holds here: a node's x is a function of the
//! whole auxiliary graph, which is why this module is a pass and not a gather).
//!
//! Ponytail: none — this is exact, and the only thing it does not carry is the slack nodes,
//! which are not in a rank row and are removed before anything reads a coordinate again.

use super::Rows;
use super::super::fast::Fast;

/// `set_xcoords`: `coord.x = rank`, then `rank = the rank index`.
pub fn run(g: &mut Fast, rows: &Rows) {
    for (r, row) in rows.iter().enumerate() {
        for &node in row {
            let record = &mut g.nodes[node as usize];
            record.coord.x = f64::from(record.rank);
            record.rank = i32::try_from(r).expect("a rank index fits i32");
        }
    }
}
