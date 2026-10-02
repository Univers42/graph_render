# Job fix-force-quadtree (agent build: review-layout-force LF-02, LF-14, LF-17, LF-22)

Read `prompts/jobs/fix-common.md` first. Source: `docs/reviews/review-layout-force.md` (ids `LF-NN`,
re-verified against `74f8994`; lines may have moved). The pinned references are on the host at
`/home/dlesieur/goinfre/refs` (read them with the read tool; `scripts/orch/gr` does not mount them).

1. **LF-02, BLOCKER.** `quadtree.rs` `build` never returns on a `±inf` coordinate: `bounds_of`
   filters only NaN, `cover` doubles until the bounds go NaN, and `insert_leaf` then loops while
   allocating. RED: `build(&[f64::INFINITY, 0.0], &[0.0, 1.0])` (and `-inf`) under a test that
   cannot hang the suite (run the tree build in a thread and `recv_timeout`, or bound the loop by a
   counter the test reads). GREEN: refuse non-finite coordinates where NaN is refused today, add a
   "bounds stopped growing" bail to `cover` and a depth cap to `insert_leaf`; the caller gets an
   error it can turn into a `StageError`, never a hang. Each bail carries a `Ponytail:` line.
2. **LF-14, MAJOR.** `quadtree/preorder.rs` `key(k)` returns the shape-arena slot, while
   `quadtree.rs` documents "insertion-order node id". RED: `build` on `(1,1),(1,1),(9,9)` and a
   test that every `key(k)` is a point index reachable from `order()`. GREEN: store the chain head's
   point id, or rename the accessor and fix the doc; pick the one no caller has to change for.
3. **LF-22, MINOR.** `build` trusts `xs.len() == ys.len()` and casts `len as u32`. RED: lengths
   `[2]` vs `[1]`. GREEN: refuse the mismatch, `u32::try_from` for the count. Fix the doc that
   claims `quadtree.rs:219` and `bounds.rs:47` are under the parameter cap, or bring them under it.
4. **LF-17, MINOR.** `barnes_hut/charge.rs` `open: w * w / theta2` is NaN at `theta = 0` on a
   zero-width cell and overflows for `|w| > 1.5e154`. RED: `theta = 0` with coincident nodes must
   descend (exact path), and a cell at `x ~ 1e200`. GREEN: branch on `theta2 == 0.0`, compute
   `(w / theta) * (w / theta)`. Also check the unguarded `centre` division the row names.
5. **LF-26 (quadtree part).** `quadtree/tests.rs` `cover_grows_a_square_that_contains_every_point`
   uses small magnitudes only; add the `1e17`, `1e300` and `inf` cases so it can fail for LF-02.
6. The review's "unverified" item on `barnes_hut/charge.rs:261` / `link.rs:84`: read
   `crate::rng::jiggle` and record whether it can return exactly `0.0` (finding with a RED test, or
   `false`).

Barnes-Hut and FA2 output must not move for finite input: paste `hashgate --seeds 8` (exit 0) and
its `GM_MUTATE_REFERENCE_DEGREE=9` control (non-zero).

Paths: `crates/graph-core/src/layout/force/quadtree.rs`, `crates/graph-core/src/layout/force/quadtree/**`,
`crates/graph-core/src/layout/force/barnes_hut/charge.rs` (+ its tests), `crates/graph-core/src/rng.rs`
(read only), `docs/measurements/fix-force-quadtree.md` (one row per id: verdict, RED, GREEN).

Done when: fix-common's done-when; every id above has a row.
