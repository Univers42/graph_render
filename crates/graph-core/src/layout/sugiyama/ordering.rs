//! Crossing reduction: median sweeps plus adjacent transposition (Gansner, Koutsofios, North & Vo, "A Technique for Drawing Directed Graphs", 1993, as `dot` implements it), seeded by a breadth-first initial order. Bilayer crossings are counted exactly, by the accumulator tree of Barth, Jünger & Mutzel (2002). Reference: `hierarchical.py:416-563`.
//!
//! Ponytail: median+transpose is a local search, not a minimum-crossing solver (every crossing count itself is exact). Failing input: a layer whose optimum needs a non-adjacent swap this pass never tries. Direction: more crossings than optimal, never a wrong edge. Escape hatch: the margin in `docs/measurements/phase05-crossings.md`.
use super::layering::Layering;
use super::weighted_median;
use crate::stage::StageError;
#[cfg(test)]
use std::cell::Cell;

#[cfg(test)]
mod tests;
mod transpose;

// How many weighted medians this thread has computed. Test-only, per thread, so the cost
// finding (the review's L-02) has a RED that counts work instead of timing it: no clock, no
// wall time, nothing on any output path.
#[cfg(test)]
thread_local! {
    static MEDIANS: Cell<u64> = const { Cell::new(0) };
}

/// Weighted medians computed so far on this thread.
#[cfg(test)]
pub(super) fn median_evaluations() -> u64 {
    MEDIANS.with(Cell::get)
}

/// `up`/`down`, bundled so a sweep helper takes one parameter, not two (≤4 per house style),
/// and read by the child [`transpose`] module.
struct Adjacency<'a> {
    up: &'a [Vec<u32>],
    down: &'a [Vec<u32>],
}
/// `(iterations, transpose_rounds)` for a run, bundled for the same reason.
struct Throttle {
    iterations: u32,
    transpose_rounds: u32,
}

impl Throttle {
    /// The throttle for `total` ordering-graph vertices (`hierarchical.py:670-672`).
    fn for_total(total: u32) -> Self {
        let iterations = if total <= 50_000 { 8 } else { 4 };
        let transpose_rounds = match total {
            0..=5_000 => 4,
            5_001..=30_000 => 2,
            30_001..=150_000 => 1,
            _ => 0,
        };
        Self {
            iterations,
            transpose_rounds,
        }
    }
}

/// Each layer's left-to-right vertex order.
pub(crate) struct Ordering {
    /// `layers[l]`: layer `l`'s vertices, left to right.
    pub(crate) layers: Vec<Vec<u32>>,
    /// Best crossing count the sweep saw: [`super::crossings_for`]'s measurement hook, unused by production code (`coords`/`routing` only need `layers`).
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) crossings: u64,
}

impl Ordering {
    /// Orders every layer of `layering` by the throttled median+transpose sweep, or refuses a
    /// `num_layers` that does not cover every vertex's own layer: that used to index past
    /// the end of `order` and leave the vertex out of every row (the review's L-13).
    pub(crate) fn build(layering: &Layering, num_layers: u32) -> Result<Self, StageError> {
        if !layer_covered(layering, num_layers) {
            return Err(StageError::Param {
                name: "num_layers",
                rule: "at or above every vertex's own layer index",
            });
        }
        let adjacency = Adjacency {
            up: &layering.up,
            down: &layering.down,
        };
        let mut layers = init_order(num_layers as usize, &layering.layer_of, &adjacency);
        let mut position = vec![0u32; layering.layer_of.len()];
        reindex(&layers, &mut position);
        let throttle = Throttle::for_total(layering.layer_of.len() as u32);
        let crossings = order_layers(&mut layers, &adjacency, &mut position, throttle);
        Ok(Self { layers, crossings })
    }
}

/// Whether `num_layers` covers every vertex's own layer. `layered` derives it from
/// `max() + 1`, so this is the guard on [`Ordering::build`]'s own seam, not a failure a
/// public input can reach.
fn layer_covered(layering: &Layering, num_layers: u32) -> bool {
    layering.layer_of.iter().all(|&l| l < num_layers)
}

/// Seeds each layer with a breadth-first walk (`up`/`down` treated as undirected) from every unvisited vertex in dense-index order, so joined vertices start out near each other.
fn init_order(num_layers: usize, layer_of: &[u32], adjacency: &Adjacency) -> Vec<Vec<u32>> {
    let mut order = vec![Vec::new(); num_layers];
    let n = layer_of.len();
    let mut seen = vec![false; n];
    let mut queue = Vec::with_capacity(n);
    let mut head = 0usize;
    for start in 0..n as u32 {
        if seen[start as usize] {
            continue;
        }
        seen[start as usize] = true;
        order[layer_of[start as usize] as usize].push(start);
        queue.push(start);
        while head < queue.len() {
            let v = queue[head];
            head += 1;
            let neighbours = adjacency.down[v as usize]
                .iter()
                .chain(&adjacency.up[v as usize]);
            for &w in neighbours {
                if !seen[w as usize] {
                    seen[w as usize] = true;
                    order[layer_of[w as usize] as usize].push(w);
                    queue.push(w);
                }
            }
        }
    }
    order
}

fn median_position(neighbours: &[u32], position: &[u32]) -> f64 {
    #[cfg(test)]
    MEDIANS.with(|c| c.set(c.get() + 1));
    weighted_median(neighbours.iter().map(|&v| position[v as usize]).collect())
}

/// Reorders `row` by median value; a neighbourless vertex stays put, ties break on slot.
///
/// **Each movable vertex's median is computed once**, into the key the sort then reads (the
/// review's L-02): the comparator used to collect and sort the neighbour positions again on
/// every comparison, `O(W log W)` medians and allocations for a width-`W` row where `O(W)`
/// is enough. The order is unchanged — a vertex's key is `(median, slot)` either way, and
/// the slots the ranked vertices land in are the same ascending set.
/// **The vertices it moved are the only ones whose key changed**, so the row is reindexed
/// from the result rather than from the key.
fn sort_by_median(row: &mut [u32], adjacency: &[Vec<u32>], position: &mut [u32]) {
    let mut slots = Vec::new();
    let mut keyed = Vec::new();
    for (slot, &v) in row.iter().enumerate() {
        if !adjacency[v as usize].is_empty() {
            keyed.push((median_position(&adjacency[v as usize], position), slot, v));
            slots.push(slot);
        }
    }
    if keyed.len() < 2 {
        return;
    }
    keyed.sort_by(|a, b| {
        a.0.partial_cmp(&b.0)
            .expect("never NaN")
            .then(a.1.cmp(&b.1))
    });
    for (&slot, &(_, _, v)) in slots.iter().zip(&keyed) {
        row[slot] = v;
    }
    for (i, &v) in row.iter().enumerate() {
        position[v as usize] = i as u32;
    }
}

/// Crossings between two adjacent layers of width `width`, given each edge's `(upper, lower)` position pair sorted ascending. Exact, not an estimate.
fn bilayer_crossings(pairs: &[(u32, u32)], width: u32) -> u64 {
    let mut first: u32 = 1;
    while first < width {
        first *= 2;
    }
    let mut tree = vec![0u64; (2 * first - 1) as usize];
    let offset = first - 1;
    let mut count = 0u64;
    for &(_, lower) in pairs {
        let mut node = lower + offset;
        tree[node as usize] += 1;
        while node > 0 {
            if node % 2 == 1 {
                count += tree[(node + 1) as usize];
            }
            node = (node - 1) / 2;
            tree[node as usize] += 1;
        }
    }
    count
}

fn total_crossings(layers: &[Vec<u32>], down: &[Vec<u32>], position: &[u32]) -> u64 {
    let mut total = 0u64;
    for level in 0..layers.len().saturating_sub(1) {
        let mut pairs: Vec<(u32, u32)> = Vec::new();
        for (i, &v) in layers[level].iter().enumerate() {
            for &w in &down[v as usize] {
                pairs.push((i as u32, position[w as usize]));
            }
        }
        pairs.sort_unstable();
        total += bilayer_crossings(&pairs, layers[level + 1].len() as u32);
    }
    total
}

/// Alternating median sweeps with a transpose step, keeping the best order seen; returns that order's crossing count.
fn order_layers(
    layers: &mut Vec<Vec<u32>>,
    adjacency: &Adjacency,
    position: &mut [u32],
    throttle: Throttle,
) -> u64 {
    let (up, down) = (adjacency.up, adjacency.down);
    let mut best = layers.clone();
    let mut best_count = total_crossings(layers, down, position);
    for sweep in 0..throttle.iterations {
        if sweep % 2 == 1 {
            for level in (0..layers.len().saturating_sub(1)).rev() {
                sort_by_median(&mut layers[level], down, position);
            }
        } else {
            for row in layers.iter_mut().skip(1) {
                sort_by_median(row, up, position);
            }
        }
        transpose::transpose(layers, adjacency, position, throttle.transpose_rounds); // no-op at 0
        let count = total_crossings(layers, down, position);
        if count < best_count {
            best_count = count;
            best = layers.clone();
        }
        if best_count == 0 {
            break;
        }
    }
    *layers = best;
    reindex(layers, position);
    best_count
}

fn reindex(layers: &[Vec<u32>], position: &mut [u32]) {
    for row in layers {
        for (i, &v) in row.iter().enumerate() {
            position[v as usize] = i as u32;
        }
    }
}
