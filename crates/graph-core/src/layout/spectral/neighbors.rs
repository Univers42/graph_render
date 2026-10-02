//! The simple undirected adjacency (C6) both eigen layouts walk, and its components.
//!
//! One CSR, not a `Vec` per node: at 1 000 000 nodes a row per node was a million heap
//! blocks, grown push by push. The rows come out ascending, as the sorted pair list they
//! replace produced them, so every layout built on them keeps its bytes.

use crate::index::Topology;

/// Row `i` holds `i`'s distinct neighbours, ascending, self-loops dropped (C6).
#[derive(Debug)]
pub(crate) struct Neighbors {
    offsets: Vec<usize>,
    targets: Vec<u32>,
}

impl Neighbors {
    /// Nodes.
    pub(crate) fn len(&self) -> usize {
        self.offsets.len() - 1
    }

    /// `i`'s neighbours, ascending.
    pub(crate) fn row(&self, i: u32) -> &[u32] {
        &self.targets[self.offsets[i as usize]..self.offsets[i as usize + 1]]
    }

    /// The adjacency of already simple, ascending `rows`.
    #[cfg(test)]
    pub(crate) fn from_rows(rows: &[Vec<u32>]) -> Self {
        let mut offsets = vec![0];
        offsets.extend(rows.iter().scan(0, |end, row| {
            *end += row.len();
            Some(*end)
        }));
        let targets = rows.concat();
        Self { offsets, targets }
    }

    /// Every row, as owned lists.
    #[cfg(test)]
    pub(crate) fn rows(&self) -> Vec<Vec<u32>> {
        (0..self.len() as u32).map(|i| self.row(i).to_vec()).collect()
    }
}

/// `topology`'s simple undirected adjacency: count each endpoint's links, place them, then
/// sort and deduplicate each row in place. O(n + m + Σ d log d), no hashing (D4).
pub(crate) fn simple_neighbors(topology: &Topology) -> Neighbors {
    let n = topology.node_count() as usize;
    let edges = topology.edges();
    let links = || {
        edges
            .source
            .iter()
            .zip(&edges.target)
            .filter(|(s, t)| s != t)
    };
    let mut offsets = vec![0; n + 1];
    for (&s, &t) in links() {
        offsets[s as usize + 1] += 1;
        offsets[t as usize + 1] += 1;
    }
    for i in 0..n {
        offsets[i + 1] += offsets[i];
    }
    let mut next = offsets[..n].to_vec();
    let mut targets = vec![0; offsets[n]];
    for (&s, &t) in links() {
        for (from, to) in [(s, t), (t, s)] {
            targets[next[from as usize]] = to;
            next[from as usize] += 1;
        }
    }
    simplify_rows(&mut offsets, &mut targets);
    Neighbors { offsets, targets }
}

/// Sorts each row, drops its repeats, and shifts it left over the gap the earlier rows'
/// repeats left behind.
fn simplify_rows(offsets: &mut [usize], targets: &mut Vec<u32>) {
    let (mut start, mut write) = (0, 0);
    for i in 1..offsets.len() {
        let end = offsets[i];
        targets[start..end].sort_unstable();
        let row_start = write;
        for read in start..end {
            if write == row_start || targets[write - 1] != targets[read] {
                targets[write] = targets[read];
                write += 1;
            }
        }
        (offsets[i], start) = (write, end);
    }
    targets.truncate(write);
}

/// Connected components by BFS from the lowest unvisited index, members sorted
/// ascending (`_connected_component_indices`), components in discovery order — which is
/// already ascending by minimum index.
pub(crate) fn find_components(neighbors: &Neighbors) -> Vec<Vec<u32>> {
    let mut visited = vec![false; neighbors.len()];
    let mut components = Vec::new();
    for start in 0..neighbors.len() as u32 {
        if !visited[start as usize] {
            components.push(bfs_component(neighbors, &mut visited, start));
        }
    }
    components
}

/// `start`'s component. The member list is the BFS queue: a node is appended once, when
/// first seen, and read once, in the order it was appended.
fn bfs_component(neighbors: &Neighbors, visited: &mut [bool], start: u32) -> Vec<u32> {
    let mut members = vec![start];
    visited[start as usize] = true;
    let mut head = 0;
    while let Some(&v) = members.get(head) {
        head += 1;
        for &w in neighbors.row(v) {
            if !visited[w as usize] {
                visited[w as usize] = true;
                members.push(w);
            }
        }
    }
    members.sort_unstable();
    members
}

/// Each node's position in its own component's member list. One map serves every
/// component, as components are disjoint; a map per component cost O(n) each, so
/// O(n · components) over a graph of many small ones.
pub(crate) fn local_positions(components: &[Vec<u32>], n: usize) -> Vec<u32> {
    let mut local_of = vec![u32::MAX; n];
    for members in components {
        for (local, &node) in members.iter().enumerate() {
            local_of[node as usize] = local as u32;
        }
    }
    local_of
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::index_model;
    use crate::records::build::{edge, node};

    #[test]
    fn rows_are_simple_and_ascending_and_components_partition_the_nodes() {
        let nodes: Vec<_> = (0..6).map(|i| node(&i.to_string(), "")).collect();
        let pairs = [(3, 0), (0, 3), (0, 1), (1, 1), (4, 5), (1, 3), (3, 1)];
        let edges: Vec<_> = pairs
            .iter()
            .enumerate()
            .map(|(e, (a, b))| edge(&format!("e{e}"), &a.to_string(), &b.to_string()))
            .collect();
        let neighbors = simple_neighbors(&index_model(&nodes, &edges).expect("fits"));
        let want = [vec![1, 3], vec![0, 3], vec![], vec![0, 1], vec![5], vec![4]];
        assert_eq!(neighbors.rows(), want);
        let components = find_components(&neighbors);
        assert_eq!(components, [vec![0, 1, 3], vec![2], vec![4, 5]]);
        assert_eq!(local_positions(&components, 6), [0, 1, 0, 2, 0, 1]);
    }
}
