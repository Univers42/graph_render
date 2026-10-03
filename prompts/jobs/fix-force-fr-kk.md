# Job fix-force-fr-kk (agent build: review-layout-force LF-04, LF-05, LF-08, LF-15, LF-20)

Read `prompts/jobs/fix-common.md` first. Source: `docs/reviews/review-layout-force.md` (ids `LF-NN`,
verified on `74f8994`). merge-p12-t4b, sg-igraph-dims and kk-newton-fix rewrote parts of these
modules since: confirm each defect on this tree first; one already fixed is `fixed-upstream` with the
commit. igraph licence: never open igraph C sources; the spec is `docs/layouts/layout.force.*.md`
and igraph's documented behaviour. The pinned references are on the host at
`/home/dlesieur/goinfre/refs` (read tool; `scripts/orch/gr` does not mount them).

1. **LF-04, MAJOR.** `fruchterman_reingold.rs` accepts a negative `start_temp` and diverges. RED:
   `start_temp: Some(-1.0)` on a 4-node path returns `Err`. GREEN: require finite and `> 0`
   (and `far` finite); the spec doc already says "only when the caller gives a positive fraction".
   Apply the same check to the 3-D variant if it shares the parameter.
2. **LF-05, MAJOR, decide before coding.** The review says the FR paper's `k = sqrt(area/n)` is
   never applied. The registered oracle is igraph's FR, not the paper. Settle it from
   `docs/layouts/layout.force.fruchterman_reingold.md` and igraph's documented force law: if the
   spec uses unit `k`, record `false` with the doc line; if it scales by `k`, fix it and say why
   the conformance row did not catch it. Never change the force law on the paper's word alone.
3. **LF-20, MINOR.** The disconnected-graph term at `fruchterman_reingold.rs` (`(c - r²·sqrt(r²)) /
   (r²·c)`) is quadratic where the docstring says linear. Match the spec doc; fix the code or the
   docstring, and make `fruchterman_reingold/tests.rs` able to fail on a sign flip.
4. **LF-26 (FR part).** `fruchterman_reingold/tests.rs` sets `start_temp: Some(0.0)`, which
   freezes every displacement, so deleting the coincident-nudge path stays green. Fix the test.
5. **LF-15, MAJOR.** `kamada_kawai.rs` declares `KK_CEILING = 2_000` and never enforces it;
   `all_pairs_hops` allocates `n * n` (overflow on wasm32). RED: `n = KK_CEILING + 1` returns a
   `StageError` without allocating. GREEN: gate `run` and use `checked_mul`. `lgl.rs` has the same
   declared-only ceiling: check it, gate it the same way (one-line edit).
6. **LF-08, MAJOR.** `forceatlas2/state.rs` caps the adaptive-speed jitter at a bare `10.0`.
   Read networkx 3.6's `forceatlas2_layout` under `/home/dlesieur/goinfre/refs/networkx-3.6/` and
   port its exact form; cite `file:line`. If the reference agrees with the code, record `false`.
   FA2's output moves: paste the `harness/oracle-fa2.py` differential (CLAUDE.md, three steps)
   before and after.

Paths: `crates/graph-core/src/layout/force/{fruchterman_reingold.rs,fruchterman_reingold/**,fruchterman_reingold_3d.rs,fruchterman_reingold_3d/**,kamada_kawai.rs,kamada_kawai/**,lgl.rs}`,
`crates/graph-core/src/layout/forceatlas2/state.rs` (+ tests), the matching `docs/layouts/*.md`
(doc fixes only), `docs/measurements/fix-force-fr-kk.md`.

Done when: fix-common's done-when; every id above has a row; `scripts/scigraphs-conformance.sh`
exits 0.
