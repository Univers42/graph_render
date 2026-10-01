# Job p13-gv1-osage-gate (agent build, gate osage against Graphviz)

Why: `layout.packing.osage` landed as `implemented`. Its Graphviz differential agrees on the 18 seeds with
n <= 10 (gap 3.6e-3) and differs by 122..1785 pt on the 982 seeds with n >= 11. Two named causes:
(1) Graphviz sizes each node box from its rendered label (54 pt for `n0`..`n9`, 57.942 for `n10`..),
while the motor uses its own size; (2) `arrayRects` sorts boxes by area with `qsort`, and equal areas
leave the order to glibc's unstable sort. The user's rule: the Graphviz ports match Graphviz output.

Fix both causes in the fixture, not by widening a ceiling:
a. Fixtures give every node an explicit size, distinct per node within a graph (so no two areas tie), and
   the DOT the oracle feeds Graphviz pins it: `fixedsize=true`, `width`/`height` in inches matching the
   motor's points exactly, `label=""` (or a label that cannot affect the size), `margin=0` where it applies.
   The motor's osage reads the same sizes from the same fixture file (fixtures are data; one generator).
b. If the motor's osage has no per-node size input, add the narrowest one the layout needs and say where.
c. Re-run `emit-graphviz-fixtures --engine osage --seeds 1000`, the oracle in its image, and
   `oracle-graphviz --engine osage`. Report the worst gap and its seed.
d. If every seed agrees within a defensible ceiling (state the ceiling and why: float formatting of
   `-Tplain` is the expected floor), set the row `gated` with its evidence; otherwise keep `implemented`
   and name the remaining cause with a measured example. Never widen a ceiling to pass.
e. Keep the existing negctls (perturbed cmp, absent dir) and add one that turns the new differential red.
f. Other engines' fixtures (twopi, circo, patchwork) must not change: show their fixture hash before/after.

Checks (paste each last line): fmt --check, clippy -D warnings, `cargo test --workspace --no-fail-fast`,
wasm32 build of graph-core, `hashgate --seeds 8` and the `GM_MUTATE_REFERENCE_DEGREE=9` negctl (exit 1),
the osage differential and its negctls.

Paths you may touch: `crates/graph-core/src/layout/graphviz/osage*/**`, `crates/graph-core/src/registry/graphviz_osage.rs`,
`crates/graph-cli/src/oracle_python/{graphviz,osage}.rs`, `harness/oracle-graphviz.py`, the capabilities row for osage,
`docs/measurements/{scigraphs-coverage,p13-gv1-osage}.md`.

Done when: the return block lists (a)-(f) one line each with the measured numbers.

Note (2026-10-01), read before writing oracle code: develop has **one** generic Graphviz differential. `crates/graph-cli/src/oracle_python/graphviz.rs` holds `by_engine` and `ENGINES`, and `cli.rs` holds `emit-graphviz-fixtures --engine <e>` / `oracle-graphviz --engine <e>`. Add your engine as a `Differential` in `oracle_python/<engine>.rs` (shape: `oracle_python/osage.rs`) plus one `by_engine` arm and one `ENGINES` entry. Key engine specifics inside `harness/oracle-graphviz.py`. Do not add another subcommand, dispatcher or fixture layout. Append your `Capability` after the last entry in `LAYOUTS`. Other Graphviz engines (neato, patchwork, circo) land in parallel, so before you finish, `git fetch origin` and read `origin/develop`'s versions of those files.
