//! Builds the per-vertex "flower" the radius solver and the placement walk need from a
//! triangulated embedding: every triangle each vertex sits in, and which vertices sit on
//! the outer boundary (`circle_packing.py:46-92`, `_planar_triangulation`, the part after
//! `triangulate_embedding` itself, which [`crate::layout::planarity`] already ports).

use crate::layout::planarity::{Embedding, Faces, faces};

/// The triangulated disk's flowers, boundary and triangle list.
pub(super) struct Flower {
    /// `at[v]`: every `(left, right)` corner pair at `v`, in triangle order.
    pub(super) at: Vec<Vec<(u32, u32)>>,
    /// Whether `v` sits on the outer boundary.
    pub(super) boundary: Vec<bool>,
    /// The boundary vertices, ascending dense index.
    pub(super) boundary_list: Vec<u32>,
    /// Every triangular face but the outer one, as found.
    pub(super) triangles: Vec<[u32; 3]>,
}

/// Traces `embedding`'s faces, finds the one matching `outer` (`triangulate_embedding`'s
/// own boundary), and builds every other face's flower — `None` when tracing does not
/// close into a disk of triangles (an interior face was not length 3, or there was none):
/// the same defensive refusal as `_planar_triangulation`'s own
/// `if not triangles or any(len(f) != 3 ...)` (`circle_packing.py:79-83`), so the caller
/// falls back exactly as SciGraphs does for a graph its planarity test cannot certify.
pub(super) fn flower(n: u32, embedding: &Embedding, outer: &[u32]) -> Option<Flower> {
    let traced = faces(embedding);
    let outer_i = outer_index(&traced, outer)?;
    let mut triangles = Vec::new();
    for i in 0..traced.len() {
        if i == outer_i {
            continue;
        }
        let f = traced.face(i);
        if f.len() != 3 {
            return None;
        }
        triangles.push([f[0], f[1], f[2]]);
    }
    if triangles.is_empty() {
        return None;
    }
    Some(build_flower(n, &triangles, outer))
}

fn build_flower(n: u32, triangles: &[[u32; 3]], outer: &[u32]) -> Flower {
    let mut boundary = vec![false; n as usize];
    for &v in outer {
        boundary[v as usize] = true;
    }
    let boundary_list = (0..n).filter(|&v| boundary[v as usize]).collect();
    let mut at = vec![Vec::new(); n as usize];
    for &[a, b, c] in triangles {
        at[a as usize].push((b, c));
        at[b as usize].push((c, a));
        at[c as usize].push((a, b));
    }
    Flower {
        at,
        boundary,
        boundary_list,
        triangles: triangles.to_vec(),
    }
}

/// The traced face matching `outer`'s node cycle, in either rotation direction
/// (`_face_key`, `circle_packing.py:36-42,74-76`): both tracers use the same
/// "predecessor about the shared neighbour" rule, so a match should need no reversal,
/// but SciGraphs checks both and so does this port.
fn outer_index(faces: &Faces, outer: &[u32]) -> Option<u32> {
    let forward = face_key(outer);
    let mut backward = outer.to_vec();
    backward.reverse();
    let backward = face_key(&backward);
    (0..faces.len()).find(|&i| {
        let key = face_key(faces.face(i));
        key == forward || key == backward
    })
}

/// Canonical rotation of a cyclic node sequence: rotated so its lowest dense index leads,
/// so two traces of the same cycle compare equal regardless of where each started
/// (`_face_key`, `circle_packing.py:36-42`).
fn face_key(face: &[u32]) -> Vec<u32> {
    if face.is_empty() {
        return Vec::new();
    }
    let start = (0..face.len()).min_by_key(|&i| face[i]).unwrap_or(0);
    face[start..]
        .iter()
        .chain(&face[..start])
        .copied()
        .collect()
}

/// Every triangulation edge once, ascending `(min, max)`: the surface
/// [`super::placement::refine_tangency`] chases, not the graph's own edges
/// (`circle_packing.py:353-357`: "report against the original graph's edges, not the
/// triangulated ones" is about the *test*, this is the *relaxation*, which explicitly
/// runs on the triangulation so the chords hold the packing together too).
pub(super) fn tri_edges(triangles: &[[u32; 3]]) -> Vec<(u32, u32)> {
    let mut pairs: Vec<(u32, u32)> = Vec::with_capacity(triangles.len() * 3);
    for &[a, b, c] in triangles {
        for (x, y) in [(a, b), (b, c), (c, a)] {
            pairs.push(if x < y { (x, y) } else { (y, x) });
        }
    }
    pairs.sort_unstable();
    pairs.dedup();
    pairs
}
