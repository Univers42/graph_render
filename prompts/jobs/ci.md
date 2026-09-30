# Job ci (agent build, CI workflow only)

Why: the user approved CI on GitHub Actions with an arm64 hash-gate arm (2026-09-30; plan step 6b).
The repo has no `.github/workflows/`. The merge floor is `scripts/orch/rows/quick.rows`; the toolchain
is the image `docker/rust.Dockerfile` (its header has the build line; no prebuilt vendor language image
may be used, CLAUDE.md "Hard constraints").

Facts:
- `scripts/orch/gate.sh <logdir> <rowsfile>` runs rows `name|expect|cmd` and writes `<logdir>/summary.txt`.
- `scripts/orch/gr` runs a command in `ge-rust` with the git top mounted at `/w` (read its header: it
  may assume `$GM_SCRATCH`, a lock, or `--pull never`; CI must work with the env it sets, e.g.
  `GM_SCRATCH=$RUNNER_TEMP/gm`).
- A fresh checkout needs `git submodule update --init .claude SciGraphs` and `npm ci` (the `cli_oracles`
  tests run the Node harness) before `cargo test`; see `scripts/orch/wt-new.sh`.
- `hashgate` prints per-seed, per-stage SHA-256 hashes (read `crates/graph-cli/src/hashgate/` for the
  output format and whether it can write them to a file).

Do: write `.github/workflows/floor.yml`:
1. Job `floor` on `ubuntu-latest` (x86_64): checkout with submodules, build `ge-rust` from
   `docker/rust.Dockerfile` (cache the layer with `actions/cache` or buildx gha cache), `npm ci` through
   the same path the repo uses (`scripts/orch/node-slim.sh` or inside ge-rust), then
   `scripts/orch/gate.sh "$RUNNER_TEMP/gate" scripts/orch/rows/quick.rows`, and upload the summary as an artifact.
2. Job `hash-arm64` on `ubuntu-24.04-arm`: same image build, then `hashgate --seeds 8` writing the
   per-stage hashes to a file, uploaded as an artifact. Do the same on x86 in job `floor`.
3. Job `hash-compare` (needs both): download both artifacts, `diff` them; any difference fails the job.
Triggers: push to `develop` and `main`, pull_request, workflow_dispatch. Pin every action by full sha
with a version comment. Add `timeout-minutes` to every job.

If a script under `scripts/orch/` cannot run on a runner as is (a host path, a lock dir), make the
smallest additive change (an env override with the old default), and say which in the return block.

Checks: `docker run --rm -v "$PWD:/w" -w /w ge-rust sh -c 'command -v actionlint'` probably fails; then
validate the YAML with `scripts/orch/node-slim.sh node -e "require('yaml')"`-style parsing if available,
else `python3 -c 'import yaml,sys;yaml.safe_load(open(sys.argv[1]))' .github/workflows/floor.yml` inside
`ge-python-oracle`. `shellcheck` any script you changed. Do not push workflow runs; you cannot see them.

Paths you may touch: `.github/workflows/floor.yml`, `scripts/orch/*.sh` (additive env overrides only).

Done when: the workflow parses, every step's command exists in the tree, and the return block lists
which steps are unverified until the first run on GitHub (they are, all of them: say so).
