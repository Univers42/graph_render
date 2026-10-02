# Job fix-scale-oracle, round 2 (agent build: the review's FIX items)

Read `prompts/jobs/fix-common.md` and `prompts/jobs/fix-scale-oracle.md` first; same paths, same
done-when except item 5 below. The branch already carries round 1
(`docs/measurements/fix-scale-oracle.md`). An independent review returned FIX. Each code item is RED
first (show it failing), then GREEN, then its report row updated.

1. **Ties cannot fail, MAJOR.** `harness/oracle-scale.py:127-137,159-162`: a case whose cut falls in
   a tie is counted under `ties` and the motor's mask is never read, so `budget.two`, `budget.path`
   and `budget.tie` pass with any mask. Detect the tie class from the degrees at the cut, never from
   which nodes the reference kept, so the case count no longer depends on the host's argsort. For a
   tied case, still check the motor: the same kept count, exact agreement on every node outside the
   tie class, and inside it the motor's own stated order (D2). Negative control: an env knob or
   `--break` value that drops one kept node from a tied case's motor mask must exit non-zero.
2. **graph-cli ignores the counters, MAJOR.** `oracle_python.rs` `judge` (or `scale.rs`): fail when
   `broken` is non-null, or when compared cases plus ties differ from the cases emitted. The report
   says "cases", not "closed cases" keyed by function.
3. **Self-loop link, MAJOR.** `crates/graph-core/src/scale/simplify/community.rs:43-55`
   `reanchor_links` keeps an `(r, r)` link when both chain ends fall in one community; `classify_edges`
   (:108), `simplify.py:216-219` and the row text drop it. RED: K4 plus the chain 0-4-5-1 under every
   pass, expecting no link with equal ends; `assert_links_survive` (`invariant_tests.rs:26-35`) gains
   that check. GREEN: drop `a == b`.
4. **Hash gate evidence, MAJOR.** Paste `hashgate --seeds 8` (exit 0) and
   `GM_MUTATE_REFERENCE_DEGREE=9 … hashgate --seeds 8` (non-zero), each with its last lines.
5. **`capabilities --check`.** It exits 1 in a fresh worktree because rows outside this job have no
   gate record there. Accepted for this job only if no problem names a `scale.*` row: paste the
   problem list filtered by `scale.` (empty) and the total count.
6. **Ledger rows, MINOR.** `crates/graph-cli/src/capabilities.rs:124-125`: give `scale.lod` and
   `scale.simplify` their differential's function names in `functions`; take `scale.adaptive` (a gap
   row, no differential) off `oracle-scale`. Fix the stale "no oracle differential" doc at :102-105.
7. **Docs and citations, MINOR.**
   - `cases.rs:128-131,74-76`: remove the doubled doc and the dead `[cases]` link. Move the
     `self_loop_on_a_leaf` loop onto a real leaf, since node 1 is in a triangle.
   - `harness/oracle-scale.py:6`: the output path is `target/scale-fixtures`.
   - `docs/measurements/phase09-lod.md:26`: cite `lod.py:81-85` for the sort, cut and mask, and `86-88`
     for the empty-mask guard (SciGraphs pin b7ccee6).
   - The F1 row: the reference has no fold or chain function, and its self-loop rule covers the
     coarse level only. Say so instead of citing simplify.py for fold and chain.
   - The merge floor: paste the commands' last lines, not a summary table. A hand `--break` run must
     write to its own output path, never the one `oracle-scale` records from.

Out of scope, stated in the report and not fixed: `verdict.rs` applies `MIN_SEEDS = 1000` to every
record, so a 12-case table can never back a gated row. That is a separate ledger job.
