# Job fix-dag-roundtrip-ctl (agent build: follow-up of fix-dag-roundtrip decisions 1 and 3)

Read `prompts/jobs/fix-common.md` first. Source: `docs/measurements/fix-dag-roundtrip.md`,
"decisions needed" 1 and 3. Start only once `fix-dag-roundtrip` is on develop (merge
`origin/develop` first and check `git log origin/develop` names it; if it does not, stop).

1. **A negative control for the dag arm of `roundtrip`.** The arm that fix-dag-roundtrip repaired has
   no `GM_MUTATE_*` knob, so nothing proves `roundtrip` turns red when the dag routes are wrong.
   Add one knob that perturbs the dag arm's per-edge `Route` assignment (for example: hand an arc's
   route to one edge outside its member list), in the place the other knobs are wired
   (`crates/graph-cli/src/hashgate/knob*` and the single knob list in
   `crates/graph-cli/tests/common/mod.rs`). Add its row to `scripts/orch/rows/quick-roundtrip.rows`
   with `expect` non-zero, and a test that the knob is in the list.
2. **"and N more".** `roundtrip` prints at most six failures; past six it must print
   `and N more` so a count of 7 and a count of 700 read differently. One test for the line.

Paths: `crates/graph-cli/src/hashgate/**`, `crates/graph-cli/src/snapshot_cmd.rs`, `crates/graph-cli/src/snapshot_cmd/**` (the `roundtrip` command),
`crates/graph-cli/tests/common/mod.rs`, `crates/graph-core/src/layout/sugiyama/**` (only if the knob
must be read there; a knob is read in one place), `scripts/orch/rows/quick-roundtrip.rows`,
`docs/measurements/fix-dag-roundtrip-ctl.md`.

Done when: `quick-roundtrip.rows` is green with the new control row failing as expected, the knob
flips `roundtrip --seeds 100` to non-zero and nothing else, fmt/clippy/test green.
