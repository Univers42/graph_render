//! Face tracing and the two shape passes that use it: `make_bi_connected` (2-connect a
//! face that touches the same node twice) and `triangulate_face` (fan it into
//! triangles). Split out of [`super`] to keep that file under the house line cap.

use super::Builder;
use std::collections::HashSet;

impl Builder {
    /// `next_face_half_edge(v, w)`: from `w`, the neighbour just counterclockwise of `v`.
    fn next_face_half_edge(&self, v: u32, w: u32) -> (u32, u32) {
        let back = self
            .find(w, v)
            .expect("(v, w) is a half-edge, so w neighbours v");
        (w, self.to[self.ccw[back as usize] as usize])
    }

    /// Adds the chord `(v1, v3)`, anchored at their shared neighbour `v2`
    /// (`add_half_edge(v1, v3, ccw=v2)` and `add_half_edge(v3, v1, cw=v2)`).
    fn add_chord(&mut self, v1: u32, v2: u32, v3: u32) {
        let at_v1 = self.find(v1, v2).expect("v2 neighbours v1 along the face");
        self.splice_after(at_v1, v3);
        let at_v3 = self.find(v3, v2).expect("v2 neighbours v3 along the face");
        self.splice_before(at_v3, v1);
    }

    fn was_visited(&self, v: u32, w: u32) -> bool {
        self.find(v, w).is_some_and(|id| self.visited[id as usize])
    }

    fn mark_visited(&mut self, v: u32, w: u32) {
        let id = self
            .find(v, w)
            .expect("(v, w) must already be a half-edge to mark");
        self.visited[id as usize] = true;
    }

    /// `make_bi_connected`: traces the face right of `(starting_node, outgoing_node)`,
    /// adding a chord wherever a node recurs on it (so the graph stays 2-connected), and
    /// marking every half-edge it crosses visited. `[]` when already traced.
    pub(super) fn make_bi_connected(&mut self, starting_node: u32, outgoing_node: u32) -> Vec<u32> {
        if self.was_visited(starting_node, outgoing_node) {
            return Vec::new();
        }
        self.mark_visited(starting_node, outgoing_node);
        let (mut v1, mut v2) = (starting_node, outgoing_node);
        let mut face = vec![starting_node];
        let mut on_face = HashSet::from([starting_node]);
        let mut v3 = self.next_face_half_edge(v1, v2).1;
        while v2 != starting_node || v3 != outgoing_node {
            assert_ne!(v1, v2, "invalid half-edge");
            if on_face.contains(&v2) {
                self.add_chord(v1, v2, v3);
                self.mark_visited(v2, v3);
                self.mark_visited(v3, v1);
                v2 = v1;
            } else {
                on_face.insert(v2);
                face.push(v2);
            }
            v1 = v2;
            (v2, v3) = self.next_face_half_edge(v2, v3);
            self.mark_visited(v1, v2);
        }
        face
    }

    /// `find_faces`: every face, once, tracing from each row's current live rotation.
    pub(super) fn find_faces(&mut self) -> Vec<Vec<u32>> {
        let mut faces = Vec::new();
        for v in 0..self.node_count() {
            let Some(start) = self.any[v as usize] else {
                continue;
            };
            let mut p = start;
            loop {
                let face = self.make_bi_connected(v, self.to[p as usize]);
                if !face.is_empty() {
                    faces.push(face);
                }
                p = self.cw[p as usize];
                if p == start {
                    break;
                }
            }
        }
        faces
    }

    /// `triangulate_face`: fans chords from `v1` across the face right of `(v1, v2)`
    /// until it is all triangles, skipping a side that already has a chord.
    pub(super) fn triangulate_face(&mut self, v1: u32, v2: u32) {
        let mut v3 = self.next_face_half_edge(v1, v2).1;
        let mut v4 = self.next_face_half_edge(v2, v3).1;
        if v1 == v2 || v1 == v3 {
            return; // fewer than 3 nodes on this face
        }
        let (mut v1, mut v2) = (v1, v2);
        while v1 != v4 {
            if self.find(v1, v3).is_some() {
                (v1, v2, v3) = (v2, v3, v4);
            } else {
                self.add_chord(v1, v2, v3);
                (v2, v3) = (v3, v4);
            }
            v4 = self.next_face_half_edge(v2, v3).1;
        }
    }
}
