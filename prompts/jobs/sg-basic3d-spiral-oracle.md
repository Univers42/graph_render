# Job sg-basic3d-spiral-oracle (agent build; the fourth function of the basic-3d oracle arm)

Why: sg-spiral3d added `layout.basic3d.spiral` (SciGraphs `_spiral_layout_3d`) and `unproven.rs` routes it
to the `oracle-basic-3d` record, but `harness/oracle-basic-3d.py` covers three functions (sphere, helix,
cube) and has no spiral, so the record names an arm that never compares it. scigraphs-conformance already
holds the row at f32 1020/1020; this job adds the per-seed differential depth, nothing else.

Do:
1. Read `harness/oracle-basic-3d.py:79-100` (the function table and `theirs_of`) and
   `crates/graph-cli/src/oracle_python/basic_3d.rs`. Add spiral as a fourth entry of both tables, calling
   SciGraphs' own `_spiral_layout_3d` (find its line with `git grep -n _spiral_layout_3d SciGraphs`), with
   the same fixture key scheme. Extend the module's comparison text with how spiral is compared and why.
2. Emit the fixtures, run the arm in `ge-python-oracle`, record it through graph-cli, as the other three.
   Paste the per-function worst difference and the bit-for-bit seed count.
3. If the record's ceiling needs a spiral line, add it with a `Ponytail:` naming what it tolerates.

Out of scope: every layout other than `layout.basic3d.spiral`; every hash; `capabilities --check`'s
whole-ledger "run the gate" rows (they predate this job).

Done when: quick.rows green (hashgate-8 and its negctl included), the oracle-basic-3d arm exit 0 with the
spiral row present, and a `--break` (or the arm's mutation knob) turning that row red, every exit code pasted.
