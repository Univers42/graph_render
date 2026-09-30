# Job gv-oracle (agent build, oracle harness only)

Why: the user decided the native Graphviz engines must match Graphviz's own output (2026-09-30). The
motor needs a docker-only Graphviz oracle before p13-gv1 (twopi, circo, patchwork, osage) and p13-gv2
(neato, fdp, sfdp, dot) can be gated. Graphviz is EPL-1.0: it is an oracle and an algorithm reference;
it is never linked, vendored, or translated line by line into `crates/`.

Facts:
- Existing oracle images: `docker/python-oracle.Dockerfile` (pinned, minimal Debian base; its header has
  the build line). Pattern for a Python differential: `graph-cli emit-spectral-fixtures` →
  `harness/oracle-spectral.py` in the image → `graph-cli oracle-spectral` checks and records
  (`crates/graph-cli/src/command.rs`, `git grep -n oracle_spectral crates/graph-cli/src`). The closed-form
  one: `command.rs:134` and `harness/oracle-closed-form.py`.
- Fixtures are data: graph-cli emits them once; both arms load the same file (CLAUDE.md "Differential oracles").
- References are fetched by `scripts/orch/fetch-refs.sh` into `$GM_SCRATCH/refs`, pinned by sha256.

Do:
1. `docker/graphviz-oracle.Dockerfile`: Graphviz built from a pinned release tarball (add it to
   `fetch-refs.sh` with its sha256, passed as a build context like `nx=` in python-oracle), on the
   same minimal base style, no network at run time. Header: build line, run line, a `Ponytail:` line.
2. `harness/oracle-graphviz.sh` (or `.py` in the image): for each fixture graph, write DOT, run
   `<engine> -Tplain` with a fixed seed/`start` where the engine takes one, and write node positions
   (in points, with the graph's bounding box) as JSON keyed by node id.
3. No Rust gate yet (no native engine exists). Add a `graph-cli emit-graphviz-fixtures` only if the
   existing fixture emitters cannot be reused; prefer reusing one and say which.
4. Prove determinism: run the oracle twice over the same fixtures for twopi and circo, and `cmp` the outputs.
5. Draft `prompts/jobs/p13-gv1-twopi.md` in this brief's style: the native twopi port, its registry
   metadata, the differential against this oracle (what "match" means numerically: state the metric
   and a ceiling to measure, not a guess), hashgate entry, negctl.

Paths you may touch: `docker/graphviz-oracle.Dockerfile`, `harness/oracle-graphviz.*`,
`scripts/orch/fetch-refs.sh` (one additive entry), `prompts/jobs/p13-gv1-*.md`,
`docs/decisions/graphviz-oracle.md`. Nothing in `crates/` unless step 3 needs it.

Done when: the image builds (paste the last line), step 4's `cmp` is silent for both engines (paste the
commands), and the draft brief exists.
