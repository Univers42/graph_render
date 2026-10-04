//! Phase 3: resolve each oriented slot's side from the reference chain `add_constraints`
//! left behind (`LRPlanarity.sign`), then bake it into the nesting depth so
//! [`Lr::order_by_nesting_depth`]'s second pass gives [`super::super::embed`] the
//! true rotation order.
//!
//! The reference's `sign(e)` allocates its `old_ref` fresh per call, so a slot already
//! finalised by an earlier call (its `reference` cut, `old_ref` empty again) is read,
//! never re-multiplied. A slot can be *read* many times here — once per dependent that
//! chains through it, which `self.ref[...] = ...` can point at the same slot from
//! several places — but must be *finalised* exactly once, or a second multiply by the
//! same `±1` factor would silently undo the first. `resolved` marks that instead of
//! reallocating a fresh table per call: persistent, `O(1)` extra per slot, and a later
//! visit — top-level or as someone else's dependency — is a no-op the moment it is set.
use super::Lr;

impl Lr<'_> {
    /// Resolves every oriented slot's side and folds it into the slot's nesting depth
    /// (`for e in self.DG.edges: nesting_depth[e] = sign(e) * nesting_depth[e]`).
    pub(super) fn resolve_signs(&mut self) {
        for slot in 0..self.adjacency.total_slots() {
            if self.oriented[slot as usize] {
                let side = i32::from(self.resolve_side(slot));
                self.nesting_depth[slot as usize] *= side;
            }
        }
    }

    /// `sign`, iterative: walks the `reference` chain from `start`, cutting each link and
    /// finalising every slot it passes through exactly once, and returns `start`'s own
    /// final side.
    fn resolve_side(&mut self, start: u32) -> i8 {
        let mut stack = vec![start];
        while let Some(e) = stack.pop() {
            if self.resolved[e as usize] {
                continue; // already finalised by this walk or an earlier one
            }
            match self.reference[e as usize] {
                Some(r) => {
                    stack.push(e); // revisit e once r is finalised
                    stack.push(r);
                    self.old_reference[e as usize] = Some(r);
                    self.reference[e as usize] = None;
                }
                None => {
                    if let Some(d) = self.old_reference[e as usize] {
                        self.side[e as usize] *= self.side[d as usize];
                    }
                    self.resolved[e as usize] = true;
                }
            }
        }
        self.side[start as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::adjacency::Adjacency;
    use super::*;

    /// Builds an `Lr` sized for `slots` slots and overrides its reference/side tables —
    /// `resolve_side` only ever touches those three tables plus `resolved`.
    fn rig<'a>(
        adjacency: &'a Adjacency,
        slots: usize,
        reference: &[Option<u32>],
        side: &[i8],
    ) -> Lr<'a> {
        let mut lr = Lr::new(adjacency);
        lr.reference = reference.to_vec();
        lr.side = side.to_vec();
        lr.old_reference = vec![None; slots];
        lr.resolved = vec![false; slots];
        lr
    }

    /// A hand-built chain `2 -> 1 -> 0` (`reference[2]=1, reference[1]=0`), each link
    /// flipping the side, so resolving the far end must fold every flip in.
    #[test]
    fn resolve_side_folds_a_whole_reference_chain() {
        let adjacency = Adjacency::simple(1, &[]);
        let mut lr = rig(&adjacency, 3, &[None, Some(0), Some(1)], &[1, -1, -1]);
        assert_eq!(lr.resolve_side(2), 1); // -1 (edge 2) * -1 (edge 1) * 1 (edge 0)
        assert_eq!(
            lr.reference,
            [None, None, None],
            "every link cut on the way"
        );
        assert_eq!(lr.resolved, [true, true, true]);
    }

    #[test]
    fn a_slot_with_no_reference_keeps_its_own_side() {
        let adjacency = Adjacency::simple(1, &[]);
        let mut lr = rig(&adjacency, 1, &[None], &[-1]);
        assert_eq!(lr.resolve_side(0), -1);
    }

    /// The bug this design guards against: naively re-applying an already-resolved
    /// slot's dependency a second time would flip its side back — `1 -> 0`, resolve `1`
    /// once (correct), then resolve it again as if fresh.
    #[test]
    fn resolving_an_already_finalised_slot_again_does_not_flip_it_back() {
        let adjacency = Adjacency::simple(1, &[]);
        let mut lr = rig(&adjacency, 2, &[None, Some(0)], &[-1, 1]);
        assert_eq!(lr.resolve_side(1), -1);
        assert_eq!(
            lr.resolve_side(1),
            -1,
            "re-resolving must not multiply in again"
        );
    }

    /// Two different slots both reference the same target (`self.ref[...] = Q.right.high`
    /// can point several slots at one target): resolving the first must not stop the
    /// second from reading the target's correct, unchanged side.
    #[test]
    fn two_dependents_of_the_same_target_both_read_its_final_side() {
        let adjacency = Adjacency::simple(1, &[]);
        let mut lr = rig(&adjacency, 3, &[None, Some(0), Some(0)], &[-1, -1, 1]);
        assert_eq!(lr.resolve_side(1), 1); // -1 (edge 1) * -1 (edge 0)
        assert_eq!(lr.resolve_side(2), -1); // 1 (edge 2) * -1 (edge 0), read only, not re-cut
    }

    /// The other half of the phase, and the one every other test here bypasses: the
    /// driver loops over *every* slot, so it must (a) skip the ones the orientation never
    /// touched and (b) multiply each resolved side into that slot's nesting depth, which
    /// is what `order_by_nesting_depth`'s second pass turns into the final rotation. The
    /// `reference` chain is consumed on the way, so it is all `None` afterwards.
    ///
    /// The rig needs a real adjacency, because the driver walks
    /// `adjacency.total_slots()`, not a table length: an empty one would leave the loop
    /// body unentered and the test would pass without asserting anything.
    #[test]
    fn resolve_signs_folds_each_side_into_its_own_slot_and_cuts_the_chain() {
        // A path 0 - 1 - 2: four half-edges, so the rig can address all four.
        let adjacency = Adjacency::simple(3, &[(0, 1), (1, 2)]);
        // Chain 3 -> 2 -> 1 -> 0, two links flipping the side; slot 3 has no reference.
        let mut lr = rig(
            &adjacency,
            4,
            &[None, Some(0), Some(1), None],
            &[1, -1, -1, 1],
        );
        lr.oriented = [true; 4].to_vec();
        lr.nesting_depth = vec![3, 5, 7, 9];
        lr.resolve_signs();
        assert_eq!(
            lr.side,
            [1, -1, 1, 1],
            "slot 2 is -1 * -1 * 1, slot 1 is -1 * 1"
        );
        assert_eq!(
            lr.nesting_depth,
            [3, -5, 7, 9],
            "each into its own slot, not shifted"
        );
        assert_eq!(lr.reference, [None; 4], "every link cut on the way");
    }

    /// The skip arm of that same loop. An unoriented slot is a half-edge the DFS pointed
    /// the other way; its nesting depth is never read (it is in nobody's `ordered` row)
    /// and its `reference` is never set, so signing it would corrupt a table the rest of
    /// the phase left alone.
    #[test]
    fn an_unoriented_slot_is_left_completely_alone() {
        let adjacency = Adjacency::simple(3, &[(0, 1), (1, 2)]);
        let mut lr = rig(&adjacency, 4, &[Some(0), None, None, None], &[1, 1, 1, 1]);
        lr.oriented = [false, true, true, true].to_vec();
        lr.nesting_depth = vec![4, 6, 8, 10];
        lr.resolve_signs();
        assert_eq!(lr.side, [1; 4], "no side to read off, so none is written");
        assert_eq!(lr.nesting_depth, [4, 6, 8, 10], "and no depth is signed");
        assert_eq!(
            lr.reference,
            [Some(0), None, None, None],
            "nor is a link cut"
        );
    }
}
