# The Barnes-Hut opening jiggle is keyed on the preorder cell, not the arena slot

Status: accepted, 2026-10-03, under full autonomy. Lands as its own commit inside the slice
`perf-bh-build` (`prompts/jobs/perf-bh-build.md`, step 1).

## Context

- `charge.rs`'s `approx` jiggles an exactly-zero axis with the key `(q.i, tree.node_id(k))`
  (`barnes_hut/charge.rs`, `approx`). `node_id(k)` is the shape-arena slot cell `k` was flattened
  from (`quadtree.rs:81-88`): the serial push order of a build that inserts points `0..n`.
- The tree itself does not depend on insertion order. A square is internal exactly when it holds two
  points the split loop separates, and that test is symmetric in the two points
  (`quadtree/build.rs`'s `insert_leaf`; `fix-qt-bail` made it hold after a late bail). The cells,
  their bounds and their point sets are a function of the point set. Only two things follow the
  insertion order: a leaf's chain order (most recent first) and the arena slots.
- At 1M nodes the two serial quadtree builds are 499 of 829 ms at 8 workers
  (`docs/measurements/perf-bh-1m.md`). A faster build inserts points in spatial order or builds
  subtrees apart. Either moves the arena slots, so the slot key pins the build to `0..n`.

## Decision

- The key becomes the preorder cell index `k`. It is unique per cell per build, as the slot was, and
  it is a function of the point set alone.
- `Quadtree::node_id` and its column are deleted.
- No other jiggle changes: the leaf case (`direct`) keeps its chain-head point key, and the collide
  and ForceAtlas2 walks never read `node_id`.

## Consequences

- Barnes-Hut bytes move only where `approx` meets `dx == 0.0` or `dy == 0.0` exactly. The slice
  measures it on its own commit (cross-tree hashes over the gate seeds and sizes) and reports which
  inputs moved. Every commit after it must be byte-identical to it.
- The hash gate is unaffected: it compares four arms of one tree, not two trees.
- d3-force's jiggle draws from a sequence and has no key, so no oracle differential depends on this
  one. The force-quality differential is a tolerance, not bytes.

Risk scores: blast 2 (one layout, one rare branch), reversibility 1 (one line), cost on failure 2,
confidence 4 until step 1's measurement. Conditions: an isolated commit, its cross-tree hashes
recorded in `docs/measurements/perf-bh-build.md`, and the test that pinned the slot key replaced by
one that pins the new key.
