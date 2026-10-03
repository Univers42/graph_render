//! Planarity testing and combinatorial embeddings, for `layout.packing.circle`'s
//! Collins–Stephenson packer (`SciGraphs/core/scigraphs_core/mesh/layouts/
//! circle_packing.py:46-92`, `_planar_triangulation`). Two ports of networkx 3.6:
//!
//! - [`planar_embedding`]: the Left-Right planarity test of Brandes 2009
//!   (`algorithms/planarity.py`, `LRPlanarity`/`check_planarity`), in [`lr`] and
//!   [`embed`]. **Never trust a planarity test's "planar" on its own** (user decision,
//!   `docs/decisions/planarity-fallback.md`): the returned [`Embedding`] goes through
//!   [`euler_certificate`] before it is handed back, and that gate is worth stating
//!   exactly, because it is narrower than "a wrong embedding is caught".
//!
//!   What it checks: the **face count**. It traces the faces of the returned rotation
//!   system and asks only whether `V - E + F == 1 + C`. A rotation system of a
//!   non-planar graph has no genus-zero rotation — its trace cannot produce that count —
//!   so an embedding built for a non-planar graph is refused, and the failure the phase
//!   most fears (a wrong embedding for a non-planar graph, silently overlapping circles
//!   downstream) surfaces as `None` rather than as a packing. That is a real guarantee,
//!   and it is the reason the gate exists — see
//!   `tests::faces::a_rotation_system_of_a_non_planar_graph_never_hits_the_euler_face_count`.
//!
//!   What it does **not** check: that the rotation is the *right* one. `V`, `E` and the
//!   face count are the same for every planar rotation of a given planar graph, so a
//!   mis-ordered but still-planar embedding passes here untouched. It is a necessary
//!   condition on one number, not a proof of planarity, and nothing downstream may read
//!   it as one.
//! - [`triangulate_embedding`]: `algorithms/planar_drawing.py`'s `triangulate_embedding`
//!   with `fully_triangulate = false`, the only mode SciGraphs calls.
//!
//! Every recursive method in the references has an iterative twin using an explicit
//! stack; only those are ported, so a 100k-node path does not recurse 100k deep. Multi-
//! edges and self-loops are removed first ([`adjacency::Adjacency::simple`], SciGraphs'
//! own `simple` reduction), and every remaining order — a row's neighbours, the DFS
//! itself — is over the dense index, never a hash: [`planar_embedding`] is
//! deterministic, byte for byte, on the same `(n, edges)`.

mod adjacency;
mod embed;
mod lr;
mod triangulate;

use adjacency::Adjacency;

pub use triangulate::triangulate_embedding;

/// A combinatorial embedding: every node's neighbours in clockwise rotation order,
/// CSR-shaped (`offsets`: `n + 1` rows; `neighbours`: one entry per half-edge, two per
/// undirected edge). Build one with [`planar_embedding`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Embedding {
    n: u32,
    offsets: Vec<u32>,
    neighbours: Vec<u32>,
}

impl Embedding {
    fn new(n: u32, offsets: Vec<u32>, neighbours: Vec<u32>) -> Self {
        debug_assert_eq!(offsets.len(), n as usize + 1);
        debug_assert_eq!(offsets.last().copied(), Some(neighbours.len() as u32));
        Self {
            n,
            offsets,
            neighbours,
        }
    }

    /// Nodes, `n`.
    pub fn node_count(&self) -> u32 {
        self.n
    }

    /// Simple-graph edges, `neighbours.len() / 2`.
    pub fn edge_count(&self) -> u32 {
        self.neighbours.len() as u32 / 2
    }

    /// `v`'s neighbours, clockwise; empty for an isolated node.
    pub fn rotation(&self, v: u32) -> &[u32] {
        let (a, b) = (self.offsets[v as usize], self.offsets[v as usize + 1]);
        &self.neighbours[a as usize..b as usize]
    }

    /// CSR row starts, `n + 1` of them.
    pub fn offsets(&self) -> &[u32] {
        &self.offsets
    }

    /// CSR values: every rotation, back to back, in node order.
    pub fn neighbours(&self) -> &[u32] {
        &self.neighbours
    }
}

/// Every face of an [`Embedding`], as the cyclic node sequence bordering it, CSR-shaped.
/// Built by [`faces`]; a plain half-edge trace, no correction for an isolated node (it
/// borders no face this way) — [`euler_certificate`] accounts for that separately.
///
/// `offsets` always holds one more entry than there are faces, *including* in the
/// [`Default`] value: a `Default` is a well-formed zero-face CSR (`offsets == [0]`), not
/// a malformed one, so [`Faces::len`]'s subtraction never underflows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Faces {
    offsets: Vec<u32>,
    nodes: Vec<u32>,
}

impl Default for Faces {
    /// The zero-face CSR: one offset (`0`), no node entries.
    fn default() -> Self {
        Self {
            offsets: vec![0],
            nodes: Vec::new(),
        }
    }
}

impl Faces {
    /// Faces traced. Zero on the [`Default`] value.
    pub fn len(&self) -> u32 {
        self.offsets.len() as u32 - 1
    }

    /// True when there is no face (no edge to trace one from).
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Face `i`'s nodes, in the order the trace visited them.
    pub fn face(&self, i: u32) -> &[u32] {
        let (a, b) = (self.offsets[i as usize], self.offsets[i as usize + 1]);
        &self.nodes[a as usize..b as usize]
    }
}

/// A row-sorted index over `embedding`'s neighbours, so `position(v, x)` — "at which
/// flat index does `x` sit in `v`'s rotation" — is a binary search instead of a scan.
/// Shared by [`faces`], [`euler_certificate`] and [`triangulate::rebuild`].
struct Positions<'a> {
    embedding: &'a Embedding,
    by_value: Vec<u32>,
}

impl<'a> Positions<'a> {
    fn build(embedding: &'a Embedding) -> Self {
        let mut by_value: Vec<u32> = (0..embedding.neighbours.len() as u32).collect();
        for v in 0..embedding.n {
            let (a, b) = (
                embedding.offsets[v as usize],
                embedding.offsets[v as usize + 1],
            );
            by_value[a as usize..b as usize]
                .sort_unstable_by_key(|&i| embedding.neighbours[i as usize]);
        }
        Self {
            embedding,
            by_value,
        }
    }

    /// The flat index of `x` within `v`'s rotation.
    ///
    /// The only caller is [`Self::next_face`], which asks about the twin of the half-edge it
    /// is walking, so the guarantee is **reciprocity**: every `(v, x)` half-edge has its
    /// `(x, v)` twin, which is what [`super::embed::into_embedding`] writes — it reads each
    /// row's own `degree(v)` slots, so both endpoints of every edge get one. That is why
    /// this is an `expect` naming the guarantee and not an error value: the rotation system
    /// is the input's own shape, and a non-reciprocal one cannot be built by anything in
    /// this crate — `embed::into_embedding` is the only writer of `neighbours`.
    ///
    /// Ponytail: nothing asserts reciprocity, so a bug in `embed` surfaces as this panic
    /// rather than as the `None` the module doc promises.
    /// `tests::properties::no_small_graph_reaches_an_expect_on_the_public_planarity_path`
    /// walks all 33 868 graphs on up to six nodes to hold it.
    fn position(&self, v: u32, x: u32) -> u32 {
        let (a, b) = (
            self.embedding.offsets[v as usize],
            self.embedding.offsets[v as usize + 1],
        );
        let row = &self.by_value[a as usize..b as usize];
        let found = row
            .binary_search_by_key(&x, |&i| self.embedding.neighbours[i as usize])
            .expect("reciprocal: every half-edge has its twin, by embed::into_embedding");
        row[found]
    }

    /// The next half-edge along the face to the right of flat position `p`
    /// (`next_face_half_edge`): from `w = neighbours[p]`, the neighbour just
    /// counterclockwise of `p`'s row-owner within `w`'s own rotation.
    fn next_face(&self, p: u32) -> u32 {
        let e = self.embedding;
        let v = row_of(e, p);
        let w = e.neighbours[p as usize];
        let back = self.position(w, v);
        let (start, end) = (e.offsets[w as usize], e.offsets[w as usize + 1]);
        if back == start { end - 1 } else { back - 1 }
    }
}

/// The row a flat index belongs to.
fn row_of(embedding: &Embedding, p: u32) -> u32 {
    embedding.offsets.partition_point(|&o| o <= p) as u32 - 1
}

/// Every face of `embedding`, traced from its rotation system
/// (`PlanarEmbedding.traverse_face`/`check_structure`).
pub fn faces(embedding: &Embedding) -> Faces {
    let positions = Positions::build(embedding);
    let mut visited = vec![false; embedding.neighbours.len()];
    let mut faces = Faces::default();
    for start in 0..embedding.neighbours.len() as u32 {
        if visited[start as usize] {
            continue;
        }
        let mut p = start;
        loop {
            visited[p as usize] = true;
            faces.nodes.push(row_of(embedding, p));
            p = positions.next_face(p);
            if p == start {
                break;
            }
        }
        faces.offsets.push(faces.nodes.len() as u32);
    }
    faces
}

/// Connected components of `embedding`'s underlying undirected graph, breadth first
/// (explicit queue): the count, and how many hold no edge at all.
fn components(embedding: &Embedding) -> (u32, u32) {
    let n = embedding.n;
    let mut seen = vec![false; n as usize];
    let (mut count, mut isolated) = (0u32, 0u32);
    for start in 0..n {
        if seen[start as usize] {
            continue;
        }
        count += 1;
        let mut queue = vec![start];
        seen[start as usize] = true;
        let mut edges_here = 0usize;
        let mut at = 0;
        while let Some(&v) = queue.get(at) {
            at += 1;
            for &w in embedding.rotation(v) {
                edges_here += 1;
                if !seen[w as usize] {
                    seen[w as usize] = true;
                    queue.push(w);
                }
            }
        }
        if edges_here == 0 {
            isolated += 1;
        }
    }
    (count, isolated)
}

/// Verifies `embedding` against Euler's formula for a (possibly disconnected) planar
/// graph, `V - E + F == 1 + C`, `F` traced from the rotation system: never trust a
/// planarity test's "planar" claim on its own. Rearranged to avoid a `u32` underflow:
/// `V + F + isolated == E + 2 * C`, where `F` is the half-edge trace's face count and
/// `isolated` corrects for a zero-edge component, which borders no traced face but
/// still shares the one common outer face with everything else (so `C` isolated nodes
/// alone contribute one face overall, not `C`).
///
/// This reads one number — `F` — and compares it. It is a **necessary** condition: a
/// non-planar graph admits no genus-zero rotation, so a rotation built for one fails
/// here, but every planar rotation of a planar graph has the same `V`, `E` and `F` and
/// passes, however wrongly ordered. See the module doc.
pub fn euler_certificate(embedding: &Embedding) -> bool {
    let v = u64::from(embedding.node_count());
    let e = u64::from(embedding.edge_count());
    let f = u64::from(faces(embedding).len());
    let (c, isolated) = components(embedding);
    v + f + u64::from(isolated) == e + 2 * u64::from(c)
}

/// Tests whether the simple graph on `n` nodes with `edges` (multi-edges and self-loops
/// removed first, as SciGraphs' own `simple` reduction does) is planar, returning a
/// combinatorial embedding when it is. `Some` only when [`euler_certificate`] accepts
/// the embedding — so a bug in the LR port that yields a rotation no drawing can have
/// fails safe, as `None`, never as a wrong "planar". The certificate checks the face
/// count only; see the module doc for what that does and does not rule out.
pub fn planar_embedding(n: u32, edges: &[(u32, u32)]) -> Option<Embedding> {
    let adjacency = Adjacency::simple(n, edges);
    let sides = lr::test(&adjacency)?;
    let embedding = embed::build(&adjacency, &sides);
    euler_certificate(&embedding).then_some(embedding)
}

#[cfg(test)]
mod tests;
