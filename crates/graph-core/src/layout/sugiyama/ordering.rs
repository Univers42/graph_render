//! Crossing reduction: median sweeps plus adjacent transposition (Gansner, Koutsofios, North & Vo, "A Technique for Drawing Directed Graphs", 1993, as `dot` implements it), seeded by a breadth-first initial order. Bilayer crossings are counted exactly, by the accumulator tree of Barth, Jünger & Mutzel (2002). Reference: `hierarchical.py:416-563`.
//!
//! Ponytail: median+transpose is a local search, not a minimum-crossing solver (every crossing count itself is exact). Failing input: a layer whose optimum needs a non-adjacent swap this pass never tries. Direction: more crossings than optimal, never a wrong edge. Escape hatch: the margin in `docs/measurements/phase05-crossings.md`.
use super::{layering::Layering, weighted_median};

/// `up`/`down`, bundled so a sweep helper takes one parameter, not two (≤4 per house style).
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
    /// Orders every layer of `layering` by the throttled median+transpose sweep.
    pub(crate) fn build(layering: &Layering, num_layers: u32) -> Self {
        let adjacency = Adjacency {
            up: &layering.up,
            down: &layering.down,
        };
        let mut layers = init_order(num_layers as usize, &layering.layer_of, &adjacency);
        let mut position = vec![0u32; layering.layer_of.len()];
        reindex(&layers, &mut position);
        let throttle = Throttle::for_total(layering.layer_of.len() as u32);
        let crossings = order_layers(&mut layers, &adjacency, &mut position, throttle);
        Self { layers, crossings }
    }
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
    weighted_median(neighbours.iter().map(|&v| position[v as usize]).collect())
}

/// Reorders `row` by median value; a neighbourless vertex stays put, ties break on slot.
fn sort_by_median(row: &mut [u32], adjacency: &[Vec<u32>], position: &mut [u32]) {
    let movable: Vec<(usize, u32)> = row
        .iter()
        .enumerate()
        .filter(|&(_, &v)| !adjacency[v as usize].is_empty())
        .map(|(i, &v)| (i, v))
        .collect();
    if movable.len() < 2 {
        return;
    }
    let mut keyed = movable.clone();
    keyed.sort_by(|&(i1, v1), &(i2, v2)| {
        let (m1, m2) = (
            median_position(&adjacency[v1 as usize], position),
            median_position(&adjacency[v2 as usize], position),
        );
        m1.partial_cmp(&m2).expect("never NaN").then(i1.cmp(&i2))
    });
    for (&(slot, _), &(_, v)) in movable.iter().zip(&keyed) {
        row[slot] = v;
    }
    for (i, &v) in row.iter().enumerate() {
        position[v as usize] = i as u32;
    }
}

/// Crossings between `v`'s and `w`'s `adjacency` edges if `v` is drawn left of `w`.
fn pair_crossings(v: u32, w: u32, adjacency: &[Vec<u32>], position: &[u32]) -> u32 {
    let mut a: Vec<u32> = adjacency[v as usize]
        .iter()
        .map(|&x| position[x as usize])
        .collect();
    let mut b: Vec<u32> = adjacency[w as usize]
        .iter()
        .map(|&x| position[x as usize])
        .collect();
    a.sort_unstable();
    b.sort_unstable();
    let mut count = 0u32;
    let mut j = 0usize;
    for &x in &a {
        while j < b.len() && b[j] < x {
            j += 1;
        }
        count += j as u32;
    }
    count
}

/// Swaps adjacent pairs while that strictly reduces their crossings, up to `rounds` full passes, stopping early once a pass makes no swap.
fn transpose(layers: &mut [Vec<u32>], adjacency: &Adjacency, position: &mut [u32], rounds: u32) {
    let (up, down) = (adjacency.up, adjacency.down);
    for _ in 0..rounds {
        let mut improved = false;
        for row in layers.iter_mut() {
            for i in 0..row.len().saturating_sub(1) {
                let (v, w) = (row[i], row[i + 1]);
                let before =
                    pair_crossings(v, w, up, position) + pair_crossings(v, w, down, position);
                if before == 0 {
                    continue;
                }
                let after =
                    pair_crossings(w, v, up, position) + pair_crossings(w, v, down, position);
                if after < before {
                    improved = true;
                    row[i] = w;
                    row[i + 1] = v;
                    position[v as usize] = i as u32 + 1;
                    position[w as usize] = i as u32;
                }
            }
        }
        if !improved {
            break;
        }
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
        transpose(layers, adjacency, position, throttle.transpose_rounds); // no-op at 0
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::index_model;
    use crate::layout::sugiyama::acyclic::{Acyclic, Arcs};
    use crate::layout::sugiyama::layering::{DUMMY_BUDGET, assign_layers};
    use crate::records::build::{edge, node};

    /// `(ordering, num_layers)` for `nodes`/`edges`.
    fn ordering(nodes: &[&str], edges: &[(&str, &str, &str)]) -> (Ordering, u32) {
        let n: Vec<_> = nodes.iter().map(|id| node(id, "")).collect();
        let e: Vec<_> = edges.iter().map(|&(id, s, t)| edge(id, s, t)).collect();
        let t = index_model(&n, &e).expect("fits");
        let acyclic = Acyclic::of(&t);
        let arcs = Arcs::new(&t, &acyclic);
        let layer = assign_layers(&arcs);
        let layering = Layering::build(&arcs, &layer, DUMMY_BUDGET);
        let num_layers = layering.layer_of.iter().copied().max().map_or(0, |m| m + 1);
        (Ordering::build(&layering, num_layers), num_layers)
    }

    #[test]
    fn a_chain_has_one_vertex_per_layer_and_no_crossings() {
        let (o, num_layers) = ordering(&["a", "b", "c"], &[("ab", "a", "b"), ("bc", "b", "c")]);
        assert_eq!(
            (num_layers, &o.layers),
            (3, &vec![vec![0], vec![1], vec![2]])
        );
        assert_eq!(o.crossings, 0);
    }

    #[test]
    fn a_solvable_crossing_is_uncrossed_and_deterministic() {
        // a,b at layer 0; c,d at layer 1; ad and bc cross under (a,b / c,d) but not under
        // (a,b / d,c) or (b,a / c,d): the sweep must find the 0-crossing order.
        let nodes = ["a", "b", "c", "d"];
        let edges = [("ac", "a", "c"), ("bd", "b", "d"), ("ad", "a", "d")];
        let (o, _) = ordering(&nodes, &edges);
        assert_eq!(o.crossings, 0, "layers: {:?}", o.layers);
        let (again, _) = ordering(&nodes, &edges);
        assert_eq!((o.layers, o.crossings), (again.layers, again.crossings));
    }
}
