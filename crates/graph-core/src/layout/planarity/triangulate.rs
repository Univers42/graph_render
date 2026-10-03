//! `triangulate_embedding` (networkx 3.6 `algorithms/planar_drawing.py`), the
//! `fully_triangulate = false` mode only — the one SciGraphs' `_planar_triangulation`
//! calls: join every component with a bare edge, trace every face, 2-connect any face
//! that touches the same node twice, then fan-triangulate every face but the largest
//! (left as the outer boundary; on a tie for largest, the first in trace order wins).
//!
//! Builds its own growable rotation over half-edge **ids** (not the fixed slots
//! [`super::embed`] uses): triangulation adds edges the input never had, so the final
//! degree of a row is not known up front. `find(row, node)` — "does row already have
//! this neighbour, and at which id" — is a scan around the row's current cycle, cheap
//! for the faces this phase ever touches.

mod faces;

use super::Embedding;

/// Triangulates `embedding` (`triangulate_embedding(embedding, fully_triangulate=False)`):
/// returns a superset embedding whose every face but one is a triangle, and **that** face's
/// nodes as the outer boundary.
///
/// The face left alone is the **largest** one, and ties go to the first in trace order
/// (strict `>`, so an equal-length face never displaces the incumbent) — so on a tie, and
/// `K_{2,3}` ties three ways at length 4, the returned face may be an interior one. It is
/// deterministic either way, and the reference breaks the same tie the same way.
pub fn triangulate_embedding(embedding: &Embedding) -> (Embedding, Vec<u32>) {
    if embedding.node_count() <= 1 {
        return (embedding.clone(), (0..embedding.node_count()).collect());
    }
    let mut builder = Builder::from_embedding(embedding);
    for pair in builder.component_reps().windows(2) {
        builder.connect_pair(pair[0], pair[1]);
    }
    let faces = builder.find_faces();
    // Nothing traced: no face to leave alone and no pair of nodes to fan from, so the
    // fold below and its `faces[outer]` would both index an empty table.
    if faces.is_empty() {
        return (builder.into_embedding(), Vec::new());
    }
    let outer =
        faces.iter().enumerate().fold(
            0,
            |best, (i, f)| {
                if f.len() > faces[best].len() { i } else { best }
            },
        );
    for (i, face) in faces.iter().enumerate() {
        // A face with fewer than two nodes has no first edge to fan from.
        if i != outer && face.len() >= 2 {
            builder.triangulate_face(face[0], face[1]);
        }
    }
    (builder.into_embedding(), faces[outer].clone())
}

/// A growable rotation, one entry per half-edge id: `to` its target, `cw`/`ccw` its
/// neighbours in the same row. `any[v]` is an arbitrary existing id in `v`'s row (`None`
/// while `v` is isolated), the anchor every row-relative lookup starts from.
struct Builder {
    to: Vec<u32>,
    cw: Vec<u32>,
    ccw: Vec<u32>,
    any: Vec<Option<u32>>,
    visited: Vec<bool>,
}

impl Builder {
    /// Copies `embedding`'s rotation into a growable doubly linked form.
    fn from_embedding(embedding: &Embedding) -> Self {
        let total = embedding.neighbours().len();
        let mut builder = Self {
            to: embedding.neighbours().to_vec(),
            cw: vec![0; total],
            ccw: vec![0; total],
            any: vec![None; embedding.node_count() as usize],
            visited: vec![false; total],
        };
        for v in 0..embedding.node_count() {
            let (a, b) = (
                embedding.offsets()[v as usize],
                embedding.offsets()[v as usize + 1],
            );
            if a == b {
                continue;
            }
            builder.any[v as usize] = Some(a);
            for p in a..b {
                builder.cw[p as usize] = if p + 1 < b { p + 1 } else { a };
                builder.ccw[p as usize] = if p > a { p - 1 } else { b - 1 };
            }
        }
        builder
    }

    fn node_count(&self) -> u32 {
        self.any.len() as u32
    }

    /// Allocates a fresh half-edge id targeting `target`; `cw`/`ccw` left for the caller
    /// to wire in.
    fn new_half_edge(&mut self, target: u32) -> u32 {
        let id = self.to.len() as u32;
        self.to.push(target);
        self.cw.push(id);
        self.ccw.push(id);
        self.visited.push(false);
        id
    }

    fn splice_after(&mut self, anchor: u32, target: u32) -> u32 {
        let id = self.new_half_edge(target);
        let next = self.cw[anchor as usize];
        self.cw[anchor as usize] = id;
        self.ccw[id as usize] = anchor;
        self.cw[id as usize] = next;
        self.ccw[next as usize] = id;
        id
    }

    fn splice_before(&mut self, anchor: u32, target: u32) -> u32 {
        let id = self.new_half_edge(target);
        let prev = self.ccw[anchor as usize];
        self.ccw[anchor as usize] = id;
        self.cw[id as usize] = anchor;
        self.ccw[id as usize] = prev;
        self.cw[prev as usize] = id;
        id
    }

    /// `row`'s current half-edge to `target`, if any — a scan around the live cycle.
    fn find(&self, row: u32, target: u32) -> Option<u32> {
        let start = self.any[row as usize]?;
        let mut p = start;
        loop {
            if self.to[p as usize] == target {
                return Some(p);
            }
            p = self.cw[p as usize];
            if p == start {
                return None;
            }
        }
    }

    /// Every component's lowest-dense-index node, ascending — deterministic
    /// representatives for [`Self::connect_pair`] to join.
    fn component_reps(&self) -> Vec<u32> {
        let n = self.node_count();
        let mut seen = vec![false; n as usize];
        let mut reps = Vec::new();
        for start in 0..n {
            if seen[start as usize] {
                continue;
            }
            reps.push(start);
            self.visit_component(start, &mut seen);
        }
        reps
    }

    /// Marks every node reachable from `start` as seen, breadth first.
    fn visit_component(&self, start: u32, seen: &mut [bool]) {
        let mut queue = vec![start];
        seen[start as usize] = true;
        let mut at = 0;
        while let Some(&v) = queue.get(at) {
            at += 1;
            if let Some(first) = self.any[v as usize] {
                let mut p = first;
                loop {
                    let w = self.to[p as usize];
                    if !seen[w as usize] {
                        seen[w as usize] = true;
                        queue.push(w);
                    }
                    p = self.cw[p as usize];
                    if p == first {
                        break;
                    }
                }
            }
        }
    }

    /// Joins `a` and `b`'s components with one plain edge (`connect_components`):
    /// arbitrary but deterministic, next to whatever each row already anchors on.
    fn connect_pair(&mut self, a: u32, b: u32) {
        match self.any[a as usize] {
            Some(anchor) => {
                self.splice_after(anchor, b);
            }
            None => self.any[a as usize] = Some(self.new_half_edge(b)),
        }
        match self.any[b as usize] {
            Some(anchor) => {
                self.splice_after(anchor, a);
            }
            None => self.any[b as usize] = Some(self.new_half_edge(a)),
        }
    }

    /// Reads every row's current live cycle out into CSR form.
    fn into_embedding(self) -> Embedding {
        let n = self.node_count();
        let mut offsets = vec![0u32; n as usize + 1];
        let mut neighbours = Vec::with_capacity(self.to.len());
        for v in 0..n {
            offsets[v as usize] = neighbours.len() as u32;
            if let Some(start) = self.any[v as usize] {
                let mut p = start;
                loop {
                    neighbours.push(self.to[p as usize]);
                    p = self.cw[p as usize];
                    if p == start {
                        break;
                    }
                }
            }
        }
        offsets[n as usize] = neighbours.len() as u32;
        Embedding::new(n, offsets, neighbours)
    }
}

#[cfg(test)]
mod tests;
