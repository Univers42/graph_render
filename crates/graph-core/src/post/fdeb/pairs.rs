//! The compatible edge pairs, pruned once and read in a fixed order by every iteration.
//!
//! The reference recomputes compatibility per pair per iteration (`fdeb.py:_produce_numpy`)
//! and never stores it, because an `(E, E)` `float64` matrix is 134 MB at 4096 edges. This
//! port stores only the pairs that *survive* the threshold, as a CSR over edges: edge `e`
//! owns entries `offsets[e]..offsets[e + 1]`, and each entry names a partner, whether the
//! two point opposite ways, and the compatibility they scored. The pair list itself is built
//! by one ascending scan — `i` outer, `j` inner — so it comes out sorted by
//! `(edge index, edge index)` with no sort at all (D5), and each edge's own row is therefore
//! ascending in its partner as well, which is the order every attraction sum is taken in
//! (D3).
//!
//! The threshold prune is the reference's (`fdeb.py:170-175`): a pair scoring below it never
//! attracts. The radius prune around it is not ported — see [`super`].

use super::FdebParams;
use super::compat::Frames;

/// One surviving pair, as one edge's row reads it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Entry {
    /// The other edge of the pair.
    pub partner: u32,
    /// Whether the two point opposite ways, so the partner's point `p` is read as its
    /// `k - 1 - p`: the corresponding point of an edge running the other way. `u_i · u_j` is
    /// exactly 0 for a perpendicular pair, and that tie takes the unflipped row.
    pub flipped: bool,
    /// The compatibility the pair scored, in `threshold..=1`.
    pub compat: f32,
}

/// The surviving pairs, CSR-shaped over edges.
#[derive(Debug, Clone, PartialEq)]
pub struct PairList {
    offsets: Vec<u32>,
    entries: Vec<Entry>,
    unbundled: Vec<u32>,
}

impl PairList {
    /// Every pair of distinct edges whose compatibility cleared `params.threshold`, and the
    /// edges that cleared it against nothing.
    pub fn of(frames: &Frames, params: &FdebParams) -> PairList {
        let (survivors, counts) = scan(frames, params);
        let offsets = offsets(&counts);
        let mut entries = vec![
            Entry {
                partner: 0,
                flipped: false,
                compat: 0.0
            };
            2 * survivors.len()
        ];
        fill(&survivors, &offsets, &mut entries);
        PairList {
            offsets,
            entries,
            unbundled: (0..frames.len() as u32)
                .filter(|e| counts[*e as usize] == 0)
                .collect(),
        }
    }

    /// Edge `e`'s row, ascending in the partner.
    pub fn row(&self, e: u32) -> &[Entry] {
        let (from, to) = (
            self.offsets[e as usize],
            self.offsets[e as usize + 1] as usize,
        );
        &self.entries[from as usize..to]
    }

    /// How many pairs survived in total — every pair counted once, though it appears in two
    /// rows.
    pub fn total(&self) -> u32 {
        (self.entries.len() / 2) as u32
    }

    /// The edges that no other edge is compatible with, ascending: drawn unbundled, and the
    /// failure the Ponytail marker names.
    pub fn unbundled(&self) -> &[u32] {
        &self.unbundled
    }
}

/// The surviving pairs, once each as `(i, j)` with `i < j` — the dot product is symmetric,
/// so the partner's entry differs only in `partner` — and how many rows each edge has. One
/// ascending scan over `i < j`, so both are already in the order the rows need.
fn scan(frames: &Frames, params: &FdebParams) -> (Vec<(u32, u32, Entry)>, Vec<u32>) {
    let edges = frames.len() as u32;
    let mut found = Vec::new();
    let mut counts = vec![0_u32; edges as usize];
    for i in 0..edges {
        for j in (i + 1)..edges {
            let compat = frames.compatibility(i as usize, j as usize, params.visibility);
            if compat >= params.threshold {
                let forward = Entry {
                    partner: j,
                    flipped: frames.dot(i as usize, j as usize) < 0.0,
                    compat,
                };
                found.push((i, j, forward));
                counts[i as usize] += 1;
                counts[j as usize] += 1;
            }
        }
    }
    (found, counts)
}

/// The CSR offsets over per-edge row lengths: `m + 1` entries from 0, never decreasing.
fn offsets(counts: &[u32]) -> Vec<u32> {
    let mut offsets = Vec::with_capacity(counts.len() + 1);
    let mut at = 0;
    for count in counts {
        offsets.push(at);
        at += count;
    }
    offsets.push(at);
    offsets
}

/// Writes each pair into both of its rows, at the position its edge's running count has
/// reached. The pairs are in `(i, j)` order, so each row fills in ascending partner.
fn fill(survivors: &[(u32, u32, Entry)], offsets: &[u32], entries: &mut [Entry]) {
    let mut at = offsets.to_vec();
    for &(i, j, entry) in survivors {
        entries[at[i as usize] as usize] = entry;
        at[i as usize] += 1;
        entries[at[j as usize] as usize] = Entry {
            partner: i,
            ..entry
        };
        at[j as usize] += 1;
    }
}
