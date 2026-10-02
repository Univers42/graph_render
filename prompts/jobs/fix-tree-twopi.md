# Job fix-tree-twopi (agent build: review-layout-tree L-01, L-08)

Read `prompts/jobs/fix-common.md` first. Source: `docs/reviews/review-layout-tree.md` (ids `L-NN`;
lines may have moved). The pinned references are on the host at `/home/dlesieur/goinfre/refs` (read
them with the read tool; `scripts/orch/gr` does not mount them). Graphviz is EPL-1.0: read
`circle.c`/`twopiinit.c` for behaviour, never copy text.

1. **L-01, MAJOR.** `layout/radial/twopi.rs` loops over components and calls `tree::grow` per
   component, but `twopi/tree.rs` `search` and `count_leaves` each allocate three n-length vectors
   inside that loop: O(components x n), measured 32x nodes -> ~232x time on an edgeless graph. RED:
   a test that counts the allocations or the per-component work on an edgeless graph of 4096 nodes
   (not a timing assert), plus a `bench`/`memory.rs` row before and after at n = 2000, 16000, 128000.
   GREEN: one scratch buffer reset per component over the touched entries only, or one pass over all
   roots. The declared complexity at `registry/radial.rs` must hold after the fix; paste the row.
2. **L-08, MAJOR.** `twopi/adjacency.rs` `Neighbours::of` rebuilds the out/inbound CSRs `Topology`
   already holds (two `Vec<(u32,u32)>` of m edges and two `sort_unstable`). GREEN: build `Neighbours`
   from the topology's own row mapping (`crates/graph-core/src/index.rs`), keeping the neighbour
   order graphviz expects. If the order differs, keep the build and record the reason as a
   `Ponytail:` line with the measured cost instead.

Output must not move: `layout.twopi` geometry is byte-identical before and after. Paste
`hashgate --seeds 8` (exit 0), its `GM_MUTATE_REFERENCE_DEGREE=9` control (non-zero), and
`graph-cli oracle-twopi` (or `oracle-graphviz --engine twopi`, whichever exists) unchanged.

Paths: `crates/graph-core/src/layout/radial/twopi.rs`, `crates/graph-core/src/layout/radial/twopi/**`,
`docs/measurements/fix-tree-twopi.md` (one row per id: verdict, RED, GREEN, before/after numbers).

Done when: fix-common's done-when; both ids have a row.
