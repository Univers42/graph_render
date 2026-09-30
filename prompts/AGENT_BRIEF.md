# Agent brief — graph-motor (read before any task)

Rules of record: `CLAUDE.md`, `prompt.md` §0 and §6 (D1–D10), the phase prompt `prompts/phase-NN-*.md`.

## House limits
- ≤40 lines per function, ≤4 parameters, ≤300 lines per file. Split into child modules; never compress.
- One deliverable per task. Touch only the paths the task lists; anything else is a deviation to report.
- TDD: write the test, run it, observe RED, then implement. Every gate row has a negative control that must fail.

## Determinism (D1–D10, summary; prompt.md §6 is authoritative)
- libm transcendentals only; no `mul_add`, `powi`, relaxed-simd, FTZ/DAZ.
- Fixed-order reductions; ties broken by dense index; never iterate a HashMap for output order.
- No wall-clock, no randomness except the seeded generators; no `usize` on the wire.
- Kernels in gather form (D10). Output must be bit-identical native vs wasm32.

## Toolchain (Docker only — never a bare cargo, rustc, npm or node)
- `scripts/orch/gr <cmd>`: cargo inside the `ge-rust` image. `gr -e KEY=VAL cmd` passes env.
- `GR_IMAGE=ge-mutants scripts/orch/gr cargo mutants ...`: mutation testing.
- `scripts/orch/node-slim.sh node <script>`: node:22-slim.
- `scripts/orch/ge-check.sh`: the TypeScript oracle gate.
- `scripts/orch/gate.sh <logdir> <rowsfile>`: rows `name|expect|cmd`, writes summary.txt.
- Pinned references (read-only): `$GM_SCRATCH/refs/` (`/goinfre/$USER/refs` on the 42 hosts, `~/goinfre/refs` elsewhere: `scripts/orch/scratch.sh`) (`npm/<pkg>-<ver>/`, `networkx-3.6/`,
  `jama-1.0.3/`, `scipy-1.16.2/lobpcg.py`) and the `SciGraphs/` submodule. Older comments cite
  `/home/user/refs/...`; same files. A reference not present there is a STOP, never an improvisation.

## Tools to use (MCP servers, skills, subagents)
- Load the matching skill with the `skill` tool before the work it covers (debug, write-test, browser-testing, frontend, design-review, ...).
- MCP servers, inside `execute` as `tools.<server>.*`: `pw` (a browser on the studio ports; UI claims need its snapshot or screenshot), `context7` and `deepwiki` (library docs), `shadcn` (components), `ruflo` (memory, analysis). A first `Unknown tool` means the server is still connecting: `search()` again. Page `search()` with `next.offset`.
- Fan independent slices out to subagents in one message.

## Never
- Run git commands that change state (denied); the orchestrator commits. `git diff/log/show/status` are fine.
- Touch osionos, `.claude/`, `.opencode/`, `src/`, `tests/` or `verify/` unless the task says so.
- Claim a result you did not run in this task. UNKNOWN = FAIL. A skipped check is not a pass.
- Write model or vendor names in code, docs or reports. Print a secret.
- Weaken, delete or skip a test, gate row or negative control to get green.
- Put a Ponytail marker on exact code; heuristics get one (failing input, direction, escape hatch).

## Return block (last thing you write, ≤25 lines)
```
status: done | blocked | partial
changed: <files, or "none">
commands: <command -> real exit code, one per line>
findings: <reviewer/scanner only: file:line SEVERITY what, or "none">
deviations: <files outside the listed paths, or "none">
decisions needed: <question + your recommended answer, or "none">
```
