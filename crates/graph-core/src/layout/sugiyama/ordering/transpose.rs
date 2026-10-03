//! Adjacent transposition: swap neighbours while that strictly lowers their crossings, up to
//! `rounds` full passes (`dot`'s own transpose step).
//!
//! **One buffer per vertex, reused.** The crossing count of an adjacent pair needs each
//! vertex's neighbour positions sorted, and a swap moves both of them — so [`Sorted`] keeps
//! one buffer per vertex per direction and refills it only after a swap has invalidated it.
//! Before (the review's L-03) [`pair_crossings`] collected and sorted two `Vec`s per call
//! and was called four times per pair: eight buffers filled per examined pair, ~500k of them
//! for the 64k pairs of a 2000-vertex two-layer sweep.
//!
//! Ponytail: invalidation is per pass, not per swap: one swap drops every buffer, so a pass
//! that swaps on every pair refills as often as the old code did. Failing input: a dense
//! layer where every adjacent swap is an improvement. Direction: time only, never a
//! crossing count — the buffer content is exactly what `pair_crossings` recomputed.
//! Escape hatch: the fill count is the `transpose_fills_a_neighbour_buffer_once_per_pair`
//! test's bound.

#[cfg(test)]
use std::cell::Cell;

use super::Adjacency;

// Counts this thread's buffer fills and adjacent pairs examined, so the cost finding has a
// RED that counts work instead of timing it.
#[cfg(test)]
thread_local! {
    static FILLS: Cell<u64> = const { Cell::new(0) };
    static PAIRS: Cell<u64> = const { Cell::new(0) };
}

/// `[buffer fills, adjacent pairs examined]` so far on this thread.
#[cfg(test)]
pub(super) fn work() -> [u64; 2] {
    [FILLS.with(Cell::get), PAIRS.with(Cell::get)]
}

/// One vertex's neighbour positions, sorted ascending, and whether it is still current.
struct Sorted<'a> {
    adjacency: &'a [Vec<u32>],
    buffer: Vec<Vec<u32>>,
    stamp: Vec<u32>,
    generation: u32,
}

impl<'a> Sorted<'a> {
    /// `n` empty buffers, none current.
    fn new(adjacency: &'a [Vec<u32>]) -> Self {
        let n = adjacency.len();
        Self {
            adjacency,
            buffer: vec![Vec::new(); n],
            stamp: vec![0; n],
            generation: 1,
        }
    }

    /// Refills `v`'s buffer from `position`, unless it is already current. The buffer is
    /// taken and given back, so a refilled vertex reuses its own allocation.
    fn fill(&mut self, v: u32, position: &[u32]) {
        let i = v as usize;
        if self.stamp[i] == self.generation {
            return;
        }
        let mut buffer = core::mem::take(&mut self.buffer[i]);
        buffer.clear();
        buffer.extend(self.adjacency[i].iter().map(|&w| position[w as usize]));
        buffer.sort_unstable();
        self.buffer[i] = buffer;
        self.stamp[i] = self.generation;
        #[cfg(test)]
        FILLS.with(|c| c.set(c.get() + 1));
    }

    /// Crossings between `v`'s and `w`'s edges with `v` left of `w`.
    fn pair(&mut self, v: u32, w: u32, position: &[u32]) -> u32 {
        self.fill(v, position);
        self.fill(w, position);
        crossings(&self.buffer[v as usize], &self.buffer[w as usize])
    }

    /// Drops every buffer: a swap moved two vertices, so no cached position list is current
    /// any more. Wrapping is unreachable — one pass would have to swap 4 billion times.
    fn invalidate(&mut self) {
        self.generation = self.generation.wrapping_add(1);
    }
}

/// Crossings between two sorted position lists: for each of `a`'s positions, how many of
/// `b`'s lie strictly left of it.
fn crossings(a: &[u32], b: &[u32]) -> u32 {
    let mut count = 0u32;
    let mut j = 0usize;
    for &x in a {
        while j < b.len() && b[j] < x {
            j += 1;
        }
        count += j as u32;
    }
    count
}

/// Swaps adjacent pairs in every row while that strictly reduces their crossings, up to
/// `rounds` full passes, stopping early once a pass makes no swap.
pub(super) fn transpose(
    layers: &mut [Vec<u32>],
    adjacency: &Adjacency,
    position: &mut [u32],
    rounds: u32,
) {
    for _ in 0..rounds {
        let mut improved = false;
        for row in layers.iter_mut() {
            improved |= transpose_row(row, adjacency, position);
        }
        if !improved {
            break;
        }
    }
}

/// One row's adjacent pairs, in ascending slot order, so the pass reads each buffer the
/// same way whichever neighbour it holds.
fn transpose_row(row: &mut [u32], adjacency: &Adjacency, position: &mut [u32]) -> bool {
    let mut up = Sorted::new(adjacency.up);
    let mut down = Sorted::new(adjacency.down);
    let mut improved = false;
    for i in 0..row.len().saturating_sub(1) {
        #[cfg(test)]
        PAIRS.with(|c| c.set(c.get() + 1));
        let (v, w) = (row[i], row[i + 1]);
        let before = up.pair(v, w, position) + down.pair(v, w, position);
        if before == 0 {
            continue;
        }
        // The `after` count reuses the buffers `before` filled: nothing moved in between.
        let after = up.pair(w, v, position) + down.pair(w, v, position);
        if after < before {
            improved = true;
            row[i] = w;
            row[i + 1] = v;
            position[v as usize] = i as u32 + 1;
            position[w as usize] = i as u32;
            up.invalidate();
            down.invalidate();
        }
    }
    improved
}
