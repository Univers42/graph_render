# Job fix-force-jiggle (agent build: follow-up of fix-force-quadtree item 6)

Your worktree is cut from `fix-force-quadtree`, which is not on develop yet; do not merge develop.

Read `prompts/jobs/fix-common.md` first. Source: `docs/measurements/fix-force-quadtree.md`, row for
item 6, and `docs/reviews/review-layout-force.md` (the "unverified" item on `charge.rs:261` /
`link.rs:84`).

fix-force-quadtree found that `crate::rng::jiggle` can return exactly `+0.0`: when
`h >> 11 == 1 << 52`, `u == 0.5` and `(u - 0.5) * 1e-6 == 0.0`. That is one draw in 2^53, and a
1.6M-draw scan found no witness. `link.rs:84` divides by a distance that a zero jiggle leaves at
zero, as `charge.rs:261` did before fix-force-quadtree guarded it.

1. **rng.rs, the one guarantee.** Make `jiggle` never return `0.0`, in `jiggle` itself and not at
   its 14 call sites: map the one word `1 << 52` to a fixed non-zero neighbour (for example
   `(1 << 52) + 1`). RED: a unit test on the extracted word-to-value step (split it into a private
   `fn jiggle_of(word: u64) -> f64` so the test needs no hash pre-image), asserting
   `jiggle_of(1 << 52) != 0.0` and that `jiggle_of(w)` is unchanged for `w` in
   `{0, 1, (1 << 52) - 1, (1 << 52) + 1, (1 << 53) - 1}`. GREEN: the mapping, with a `Ponytail:` line
   naming the one word it moves and why no hash can move with it.
2. **link.rs:84.** Read the division. If item 1 makes its denominator provably non-zero, write that
   proof as the comment above it and add a test that a link between two coincident nodes yields a
   finite force. If it does not (the distance can be zero for another reason), guard it the way
   `charge.rs:261` is guarded and say so in the measurement row.
3. Output must not move: paste `hashgate --seeds 8` (exit 0) and its
   `GM_MUTATE_REFERENCE_DEGREE=9` control (non-zero). A moved hash means the mapping touched more
   than the one word: stop and report.

Paths: `crates/graph-core/src/rng.rs` (+ its tests), `crates/graph-core/src/layout/force/barnes_hut/link.rs`
(+ its tests), `docs/measurements/fix-force-jiggle.md` (one row per item: verdict, RED, GREEN).

Done when: fix-common's done-when; both items have a row; the hashgate pair above is pasted.
