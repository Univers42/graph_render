# Job sg-conformance-split (agent build, SciGraphs conformance, refactor only)

Read `prompts/jobs/sg-common.md` first; its precondition applies.

Why: three files of the landed conformance module break the 300-line house limit:
`crates/graph-cli/src/oracle_python/conformance/fixtures.rs` (309), `verdict/tests.rs` (309) and
`baseline/table.rs` (302). Every later `sg-*` job re-pins a row in `table.rs`, so it is split first,
before any of them edits it.

Do:
1. Record the untouched tree: `scripts/scigraphs-conformance.sh` exit 0, and
   `scripts/orch/gr cargo test -p graph-cli conformance` (paste the test count).
2. Split each file into child modules along a seam the file already has (for `table.rs`: one
   module per row family, e.g. basic / networkx / igraph / graphviz, in `rows.rs` order; for the
   tests: one file per behaviour under test). Pure moves: no renamed test, no changed value, no
   changed sha256. Each new file under 300 lines, each function under 40.
3. Re-run both commands: same exit, same test count, and
   `target/scigraphs-conformance/metrics.json` byte-identical to step 1's (`cmp`).

Paths: `crates/graph-cli/src/oracle_python/conformance/**` only.

Done when: the three files and every new one are under 300 lines (`wc -l`), step 3's `cmp` is
silent, `--break` exits 1 naming `SPRING_3D`, and the sg-common done-when holds.
