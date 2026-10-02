//! A component's Laplacian, addressed by local index (position in `members`), split out
//! of `spectral.rs` to stay under the house line cap.

use super::Neighbors;

/// A component's Laplacian, addressed by local index (position in `members`).
pub(super) struct ComponentGraph<'a> {
    members: &'a [u32],
    neighbors: &'a Neighbors,
    local_of: &'a [u32],
    pub(super) degree: Vec<f64>,
}

impl<'a> ComponentGraph<'a> {
    /// `local_of` maps a node to its position in its own component
    /// ([`super::local_positions`]).
    pub(super) fn build(members: &'a [u32], neighbors: &'a Neighbors, local_of: &'a [u32]) -> Self {
        let degree = members
            .iter()
            .map(|&g| neighbors.row(g).len() as f64)
            .collect();
        Self {
            members,
            neighbors,
            local_of,
            degree,
        }
    }

    pub(super) fn size(&self) -> usize {
        self.members.len()
    }

    /// `y = Lx`, a CSR-row gather (D10): row `i` reads only `x` and its own neighbours.
    pub(super) fn matvec(&self, x: &[f64], y: &mut [f64]) {
        for (li, &g) in self.members.iter().enumerate() {
            let mut acc = self.degree[li] * x[li];
            for &w in self.neighbors.row(g) {
                acc -= x[self.local_of[w as usize] as usize];
            }
            y[li] = acc;
        }
    }

    pub(super) fn dense_matrix(&self) -> Vec<f64> {
        let n = self.size();
        let mut a = vec![0.0; n * n];
        for (li, &g) in self.members.iter().enumerate() {
            a[li * n + li] = self.degree[li];
            for &w in self.neighbors.row(g) {
                a[li * n + self.local_of[w as usize] as usize] = -1.0;
            }
        }
        a
    }
}
