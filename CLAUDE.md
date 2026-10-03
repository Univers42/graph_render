# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this repo is

Three things in one tree:

- **graph-motor** — the Rust workspace under `crates/`. It computes graph and diagram geometry and emits
  numbers; it is not a renderer and not an application. This is the product. Runbook: `prompt.md`;
  phases: `prompts/phase-NN-*.md`.
- **`@osionos/graph-engine`** — the TypeScript engine under `src/` (Canvas2D + d3-force). Since Phase 0
  it is the **differential oracle** for graph-motor: test infrastructure, never shipped, never deleted.
  Commands and architecture: `docs/oracle-engine.md`.
- **The studio** — `packages/graph-render`, `packages/graph-studio` and the host page `app/`: an
  Obsidian-style interface over the wasm motor. A product in its own right since 2026-09-29
  (`docs/decisions/studio-is-a-product.md`), with its own gates. See "Architecture (studio)" below.

Submodule: `SciGraphs/` (a Python Blender extension: the reference design and the source of the Python
oracles); run `git submodule update --init` if it is empty. The house rules come from the `devil` plugin
(`univers42/claude-deal-with-the-devil`), seeded by its setup; see the block at the end of this file.

Current state, newest first: `prompts/RESUME.md`, then `docs/reports/STATUS.md` and `docs/reports/HANDOFF.md`. Older
docs write paths as `/home/user/...`; that is a previous host, same files.

## Standing rules (set by the user, LESdylan)

Where `prompt.md` §0.7 or `prompts/ONBOARDING.md` §5.7 disagree with this section (they say no
auto-push and no commits to `develop`), this section wins.

### Talking to the user
- Be terse. No narration between tool calls, no recaps.
- Write to the user only when a phase finishes, when blocked, or when a decision is needed.
- Keep each message to a few lines.
- Phase reports in `docs/reports/` still follow the rules in full (§12 shape).

### Git
- Author every commit as `LESdylan <dev.pro.photo@gmail.com>`. No Co-Authored-By, no "Generated with" trailer.
- Commit message is exactly `updated`.
- Push directly to `develop`. No pull request.
- Commit and push after every green step or phase, so nothing depends on the container surviving.
- No model identifiers in commits, PRs, code or docs.

### Working mode
- Full autonomy: run phases 0 → 10 (`prompts/phase-NN-*.md`) per `prompts/ONBOARDING.md` and `prompt.md`.
- Keep an hourly self check-in armed with `send_later`.
- Follow the `devil` plugin's house rules (`.claude/rules/devil/`, repo `univers42/claude-deal-with-the-devil`).

### Hard constraints
- osionos (`/home/dlesieur/Documents/osionos`) is READ ONLY (`scripts/guard-osionos.sh`, exit 90 = broken).
- Docker-only toolchain; no prebuilt vendor language images (`FROM rust:*`, playwright, ...).
- Never claim an unrun result: UNKNOWN = FAIL, SKIP is not a pass.
- A missing reference is a stop, not an improvisation.
- Stay inside each phase's authorization envelope; report every deviation.
- Never print secret values.
- Do not hand-edit `.claude/rules/devil/`: change the kit and re-run its setup.

### Parallel branches (checked on 2026-09-29 with `git merge-tree` and a disk check; worth doing)
- Every independent unit of work (a phase, or a slice inside a phase) gets its own branch and its own worktree, made by `scripts/orch/wt-new.sh <branch>` under `$GM_SCRATCH/wt/`. Exactly one agent per worktree. Two agents in one worktree clobbered each other's edits on 2026-09-28 (p3fix-a and p3fix-c).
- Start a branch as soon as its dependencies allow it; don't wait for unrelated phases.
  - Dependency order: p3 → {p5, p6e, p6f, p8}; p4 → {p7's SDK row, p10}; p6f → p9 → p11. p4 and p7 are independent of p3.
  - A branch may start from an unmerged dependency branch. Once that dependency lands, merge develop into it.
- Slices inside a phase use sub-branches `pN-<slice>`, which merge into `pN` and never straight into develop.
- Merge into develop one branch at a time, in plan order. Merge the current develop into the branch first (never rebase) and check the merged tree.
  - Merge floor (user, 2026-09-29, `prompts/RESUME.md`): fmt, clippy `-D warnings` and `cargo test --workspace --no-fail-fast` green on the merged tree. The full gate (hashgate-1000, oracles, mutants) then runs once on develop, and its red rows become repair tasks. This replaced full gating per branch, which queued every branch behind one lock for hours.
  - Review and the phase report are still owed per phase; a gate that has not run is "not run", never "green".
- Some files are touched by several branches: `lib.rs`, `registry.rs`, `capabilities.rs`, `main.rs`, `Cargo.lock` and `canonical_json/schema.rs`.
  - Edit them additively only.
  - Resolve conflicts by keeping both intents. The measured conflicts are small (p4+p7: 3 hunks; p5+p6e: 1 hunk).
  - Conflicts in contract or registry files go to an independent judgement.
- CPU is the limit, not git.
  - Run at most one timed gate at a time (hashgate, mutants) — on 2026-09-28, `hashgate --seeds 1000` hit `CHILD_TIMEOUT` 2700 s while mutants ran alongside it.
  - Other cargo jobs pass `CARGO_BUILD_JOBS`/`RUST_TEST_THREADS` (`scripts/orch/gr`).
  - Ponytail: a thread cap is not a CPU cap. A gate that times out is re-run on its own and never counted as a pass.
- Disk: each worktree's `target/` is about 1 GB (33 GB free on host dlesieur42, 2026-09-30). Remove a worktree after its branch merges.

## Commands

No bare `cargo`, `rustc`, `npm` or `node`: the host has none that match the pins. The wrappers in
`scripts/orch/` mount the current git top-level at `/w`, so they act on whichever worktree you are in.

```sh
# once per host: the pinned references. ge-rust, ge-mutants, ge-profile and gm-chromium build
# themselves on first use and again when their Dockerfile changes (scripts/orch/image.sh); the
# Python and graphviz oracle images still build by the line in their Dockerfile header.
scripts/orch/fetch-refs.sh                                        # -> $GM_SCRATCH/refs

# the merge floor
scripts/orch/gr cargo fmt --all --check
scripts/orch/gr cargo clippy --workspace --all-targets -- -D warnings
scripts/orch/gr cargo test --workspace --no-fail-fast
scripts/orch/gr cargo build -p graph-core --target wasm32-unknown-unknown   # graph-core's purity gate

# one test
scripts/orch/gr cargo test -p graph-core <name_filter>
scripts/orch/gr cargo test -p graph-cli --test cli <name_filter>

# graph-cli gates. Exit 0 = passed, 1 = ran and failed, 2 = could not run.
scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8
scripts/orch/gr -e GM_MUTATE_REFERENCE_DEGREE=9 cargo run -q -p graph-cli -- hashgate --seeds 8  # negative control: expect NON-zero
scripts/orch/gr cargo run -q -p graph-cli -- capabilities --check
scripts/orch/gr cargo run -q -p graph-cli -- codegen --check
scripts/orch/gr cargo run -q -p graph-cli -- roundtrip --seeds 100

# timed gates: one at a time, under the host-wide lock
scripts/orch/timed scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 1000
scripts/orch/mutants.sh <base-rev>
scripts/orch/gate.sh <logdir> <rowsfile>      # rows are `name|expect|cmd`; writes <logdir>/summary.txt

# TypeScript oracle and the JS SDK
scripts/orch/node-slim.sh npm ci --ignore-scripts
scripts/orch/ge-check.sh                       # `npm run check` inside the repo Dockerfile
scripts/orch/node-slim.sh npm run sdk:typecheck
scripts/orch/node-slim.sh npm run sdk:test
scripts/orch/gr cargo build -p graph-wasm --release --target wasm32-unknown-unknown
scripts/orch/node-slim.sh npm run sdk:smoke

# the studio (node:22-slim in Docker; the header of each script is its manual)
scripts/studio.sh wasm        # build graph-wasm and stage it with fixtures/ into app/public
scripts/studio.sh             # dev server on http://127.0.0.1:5174 (STUDIO_PORT)
scripts/studio.sh check       # the studio's merge floor: tsc, unit + render tests, eslint, vite build
scripts/studio.sh test        # tests only; needs the pinned refs at $REFS (default $GM_SCRATCH/refs)
scripts/studio-nav.sh         # one browser gate over app/dist; siblings: perf, parity, interact, filters, ...
STUDIO_NAV_BREAK=1 scripts/studio-nav.sh   # its negative control: expect non-zero
scripts/studio-smoke.sh       # the load smoke over app/dist: no page error, no banner, a node drawn
STUDIO_SMOKE_BREAK=1 scripts/studio-smoke.sh   # its negative control: expect non-zero
scripts/studio-backend.sh      # the WebGL2 layer against Canvas2D: pixel parity, `auto`, the fallback, a lost context; STUDIO_BACKEND_BREAK=1 for its negative control
```

- A fresh worktree needs `npm ci` before `cargo test`: the `cli_oracles` tests run the Node harness and
  fail on a missing `node_modules`.
- Without `--no-fail-fast` cargo stops at the first failing test binary and hides the other crates.
- `gr` caps memory at 8g (`GR_MEM`); exit 137 on a legitimate row means raise it. `GR_IMAGE` picks the image.
- Node containers run `GM_NODE_IMAGE` (`scripts/orch/image.sh`): node 22.23.3 pinned by digest. Never
  run `npm` on the host: the toolchain is the image, not the host.
- Host-local state lives under `$GM_SCRATCH` (`scripts/orch/scratch.sh`: `/goinfre/$USER` where
  `/goinfre` exists, else `~/goinfre`): worktrees, references, logs, locks. A host change loses it; rebuild
  it with `fetch-refs.sh`, the image builds and `wt-new.sh`. Everything else is versioned: rows files in
  `scripts/orch/rows/` (`quick.rows` = the merge floor plus wasm32, hashgate 8 and its negctl), the job
  preamble `scripts/orch/common.md`, job briefs in `prompts/jobs/`.
- Python differentials run in three steps: `graph-cli emit-spectral-fixtures` (or `emit-fa2-fixtures`),
  then `harness/oracle-spectral.py` (or `oracle-fa2.py`) inside `ge-python-oracle`, then
  `graph-cli oracle-spectral` (or `oracle-fa2`), which checks the result against its ceiling and records it.
- The studio's tests refuse to run without the pinned references rather than skip, and a skipped
  `node:test` case fails `studio.sh`. The browser gates need `scripts/studio.sh build` first and never
  take the host gate lock.
- `docs/studio.md` describes the first studio. Its port and its `src/core` imports are superseded by
  `docs/decisions/render-ports-not-imports.md`; the script headers are current.
- Agent jobs run headless in OpenCode (`opencode.json`, `.opencode/agents/`): `scripts/orch/oc-job.sh`
  launches one in a worktree and gates it, and `scripts/orch/oc-status.sh` lists every job's state.
  `scripts/orch/oc-tabs.sh` opens one OpenCode window with a tab per live session (`-a`: every session of
  the project, minus probes and finished or landed sessions whose worktree is gone, and an unfinished
  session's removed worktree rebuilt first; `-p` / `-d`: only the sessions in progress / done; `-n`: add
  the tabs to a window already open in this directory).
  OpenCode 2.x ignores `opencode.json` `instructions` and reads only `AGENTS.md` (a link to
  `prompts/AGENT_BRIEF.md`); the kit's bridge `.opencode/plugins/devil.js` adds its always-on rules.
  The kit's agents, commands and bridge are untracked links that `devil setup --only opencode` makes per
  worktree (`wt-new.sh` runs it); the house agents under `.opencode/agents/` are tracked.
  `scripts/orch/job-check.sh start|wait|status|lint|commit` is the deterministic half: it runs the rows
  gate and commits only on a PASS over the same tree.
- The shell is zsh: an unmatched glob aborts the command and `echo ===` expands `=`. Use `git grep`.
  Kill by PID; `pkill -f <pattern>` matches your own command line.

## Architecture (graph-motor)

### A staged pipeline over an immutable topology

```
INGEST -> TOPOLOGY -> ANALYSIS -> LAYOUT -> POST -> SCALE -> GEOMETRY
```

Each stage is pure, optional and **hashed on its own**, so a native/wasm32 divergence names the stage it
started in. Only LAYOUT varies per diagram family. `Stage::run` (`crates/graph-core/src/stage.rs`) is an
associated function over `&Topology` and `&Params`: no `self`, no state carried between runs.

### Crates, in dependency order

| Crate | Role | Constraint |
|---|---|---|
| `graph-contract` | The wire format: geometry vocabulary, binary snapshot, canonical JSON, codegen | The single source of truth. `graph-cli codegen` writes the committed schema and TypeScript declarations; `--check` fails on a stale file |
| `graph-core` | The motor: topology (string arena, dense index, SoA columns, three CSRs), layouts, analysis, post | Pure: no I/O, clock, async or bindings. Builds for native and `wasm32-unknown-unknown`. Dependencies are a closed list (`libm`, `indexmap`, `petgraph`); adding one is a stop-and-ask |
| `graph-wasm` | `extern "C"` glue over graph-core | No wasm-bindgen: the motor passes numeric buffers, viewed zero-copy over linear memory |
| `graph-sdk-js` | TypeScript wrapper over the wasm ABI (`crates/graph-sdk-js`) | Not a cargo member. Ships source, no build; type-checks with the root `typescript` |
| `graph-cli` | The gates: hash gate, ledger, fixtures, differentials, bench | Test instruments, not product |

### The registry drives the ledger and the hash gate

`crates/graph-core/src/registry.rs` lists every layout in `LAYOUTS` with its `Metadata`. No field is an
`Option`: a layout cannot be registered without its oracle, complexity, `scale_ceiling`, `degradation`
and `ponytail`. `graph-cli capabilities` builds its layout rows from that list, and `hashgate` hashes
every entry, in registry order. Analysis and post rows are in `crates/graph-cli/src/capabilities/`.

Progress is never written by hand. A row's status is `absent | stub | implemented | gated`, and `gated`
needs a recorded 4-way hash and oracle differential for the current tree. Tests that find a capability
row should look it up by id: row indices move whenever an entry is inserted.

### Evidence is pinned to the tree

A gate writes `target/gates/<name>.json` with the fingerprint of the tree it ran on
(`crates/graph-cli/src/fingerprint.rs`: crates, harness, fixtures, `src`, docker, Cargo files and the
oracle's lockfile). Editing any of those voids the record until the gate runs again. Documentation is
not fingerprinted. The binary also refuses to record against a tree other than the one it was built from,
so run a gate only on the tree you intend to keep.

### The hash gate and its negative controls

`hashgate` runs native twice and wasm32 twice (under Node, on the real artifact) and requires all four
SHA-256 hashes to be equal per seed and per stage. Each `GM_MUTATE_*` knob perturbs one arm and must
turn the gate red; the knob list exists once, in `crates/graph-cli/tests/common/mod.rs`. A child that
runs past `CHILD_TIMEOUT` (2700 s, `runner.rs`) is reported as "could not run", exit 2.

### Geometry: a closed vocabulary with two faces

Nodes are `Point`, `Circle` or `Box`; edges are `Line`, `Polyline` or `Curve`; `Ribbon` and `Arc` have
reserved tags and no implementation. The kind is declared **once per snapshot**, not per element, which
keeps the columns pure SoA and the transport zero-copy. The canonical JSON face is the public contract;
the binary columnar face is the hot path. The hash is taken over the binary face, and the JSON face must
round-trip to the identical bytes (`roundtrip`). Only stable string ids cross the wire; the dense index
never leaves the motor. Specs: `docs/contract/`.

### Determinism (D1–D10, authoritative in `prompt.md` §6)

Output must be bit-identical native vs wasm32. Read §6 before touching motor math. In short: `libm` for
every transcendental, no FMA; fixed-order reductions; `IndexMap`/`BTreeMap`, never `HashMap`; wire integers
`u32`/`u64`, never `usize`; no clock; per-step kernels are gathers. Beyond §6: a new struct field needs
every constructor across crates, wire formats included. The wasm ingest once dropped
`EdgeRecord.child_first`, and only the hash gate caught it.

### Differential oracles

| Subject | Oracle | Harness |
|---|---|---|
| The 17 pure `core/model` functions | the TypeScript engine in `src/` | `harness/oracle-diff.mjs` |
| Hierarchy layouts, layered DAG | `d3-hierarchy`, `dagre-d3-es` | `harness/oracle-layouts.mjs` (`--dag`) |
| Force layout quality | `d3-force` | `harness/stress-d3.mjs`, `graph-cli stress --oracle d3` |
| Spectral, Pivot MDS | SciGraphs, scipy | `harness/oracle-spectral.py` |
| ForceAtlas2 | networkx 3.6 | `harness/oracle-fa2.py` |

Fixtures are data, not code: graph-cli emits them once and both arms load the same file, so no second
generator can drift. Edge ids sort undirected endpoints in **byte order**; the oracle's `localeCompare`
is the known defect (H1), exercised by `fixtures/adversarial-ids.json`.

Python oracles agree to a tolerance, not bitwise. Ponytail: the ForceAtlas2 coordinate differential
cannot gate as written, because networkx diverges from itself by the same margin under a 1-ulp
perturbation of the start (`prompts/RESUME.md` item 4).

### House limits

At most 40 lines per function, 4 parameters and 300 lines per file; split into child modules. Every
gate row has a negative control that must fail. Every heuristic carries a `Ponytail:` marker, which is
also a required ledger field. Decisions are recorded in `docs/decisions/`, measurements in
`docs/measurements/`.

## Architecture (studio)

| Layer | Path | Depends on | Must not depend on |
|---|---|---|---|
| motor | `crates/*` | as above | `packages/`, `app/`, `deploy/` |
| render | `packages/graph-render` | nothing at runtime | React, the SDK runtime, `src/`, `fetch` |
| studio | `packages/graph-studio` | graph-render, the SDK (in the worker only), React | `src/` |
| host | `app/` | graph-studio | anything else |

- The renderer's only input is snapshot **bytes** (`docs/contract/binary-layout.md`). It ports what it
  needs from the oracle and never imports `src/`: gate row `no-oracle-import`.
- Gate row `motor-alone` runs the Rust merge floor with `app`, `packages` and `deploy` removed. Red means
  the motor has grown a dependency on its viewer.
- `<graph-studio>` (`packages/graph-studio/src/element.ts`) is a custom element in its own shadow root.
  The motor runs in a Web Worker behind `src/motor/protocol.ts`.
- Every user action is registered once in `src/actions/registry.ts`. The dock, console, shortcuts and
  host all call `resolve`, so arguments are checked in one place.
- Layout, post and analysis pickers are filled from the motor's own registries (`Motor.layouts()`,
  `posts()`, `analyses()`). The one id the studio sources name is the default layout in
  `src/state/settings.ts`.
- The packages have no `node_modules` of their own: they type-check, lint, test and build with the
  toolchain pinned in `app/package.json`. The root `package.json`, lockfile and `tsconfig.json` are
  fingerprinted, so touching them voids gate evidence.

## Reference
- The TypeScript oracle engine (commands, architecture): `docs/oracle-engine.md`. Agent brief: `prompts/AGENT_BRIEF.md`.
- Where the math lives and which references are on disk: `prompts/REFERENCES.md`.

<!-- devil:start -->
## The devil kit

Installed as the Claude Code plugin `devil`.

- `/devil:guide` lists every command, workflow, skill and agent with its stage.
- `devil <tool>` runs a tool: `devil digest`, `devil quality --no-audit`, `devil selfcheck`.
- Its 12 always-on rules are seeded under `.claude/rules/devil/` and load every session.
- After a plugin update, run `/devil:setup --apply` to re-seed them.
<!-- devil:end -->
