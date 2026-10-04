# Job supply-audit: a supply-chain audit gate for the two Rust workspaces and the shipped JS

Worktree `~/goinfre/wt/supply-audit`, branch `supply-audit`, from develop.

The quality bar (`.claude/rules/devil/quality-bar.md`, layer 5) says "a known-vuln dependency fails
the gate". No row checks for that today: `git grep -n -i 'cargo.deny\|cargo.audit\|npm audit' --
scripts/orch/rows` is empty. The micro-service `server/graph-server` is internet-facing (axum 0.8.9,
hyper 1.11.1, tokio 1.53.2; `server/graph-server/Cargo.toml`), and its lockfile is
`server/Cargo.lock`, separate from the motor's `Cargo.lock`.

Dispatch at most 1 `explore` subagent.

## Exact tasks

1. **The image.** Write `docker/audit.Dockerfile` (`ge-audit`), modelled line for line on
   `docker/mutants.Dockerfile`: `ARG BASE=ge-rust`, `FROM ${BASE}`, the same optional `extra_ca`
   secret, `cargo install --locked cargo-deny --version "${CARGO_DENY_VERSION}"`, and the registry
   cleanup.
   - Pin `CARGO_DENY_VERSION` to the newest release that builds on the image's Rust 1.98.1
     (`docker/rust.Dockerfile:23`). Write in the header how you chose it.
   - Register it in `scripts/orch/image.sh` `gm_image_recipe`, as one added line:
     `ge-audit) echo "docker/audit.Dockerfile ge-rust" ;;`. That is the only change to `image.sh`.
   - Run with `GR_IMAGE=ge-audit scripts/orch/gr ...`. `gr` calls `ensure_image` (`gr:19-20`), so the
     image builds on first use.
2. **The policy.** Write `deny.toml` at the repo top. It serves both workspaces; pass it with
   `--config` for `server/`.
   - `[advisories]`: the defaults. No `ignore` entry.
   - `[licenses]`: `allow` lists exactly the licenses that `cargo deny list` reports today across
     both lockfiles, measured, not guessed. Paste that output in the return block. `confidence-threshold`
     stays at its default.
   - `[bans]`: `multiple-versions = "warn"`. `[sources]`: `unknown-registry = "deny"` and
     `unknown-git = "deny"`.
   - A header comment: what each section is for, and that an `ignore` entry needs a linked issue and
     a one-line reason (the quality bar's suppression rule).
3. **Run it, before writing any row.**
   - `GR_IMAGE=ge-audit scripts/orch/gr cargo deny --manifest-path Cargo.toml check advisories bans licenses sources`
   - The same with `--manifest-path server/Cargo.toml --config deny.toml`.
   - Paste both exit codes and their last 15 lines.
   - **If an advisory fires on either lockfile, STOP after task 6.** Do not add an `ignore`, and do not
     bump a dependency. Report each advisory's id, crate, version and patched range under
     "decisions needed". That is a finding for the orchestrator, not for this job. The rows of task 5
     are still written, with expect `0`.
4. **The shipped JS.** The service bundle (`app/vite.embed.config.ts`) ships the runtime
   dependencies of `app/package.json`. Run
   `scripts/orch/node-slim.sh bash -c 'cd app && npm audit --omit=dev --audit-level=high'` and paste
   the exit code and the summary line. A finding is handled as in task 3: report it, never suppress it.
5. **Rows**, in a new file `scripts/orch/rows/audit.rows`, each with a `#` comment line above it.
   Read `scripts/orch/rows/service-image.rows` first for the style.
   - `cargo-deny-motor|0|GR_IMAGE=ge-audit scripts/orch/gr cargo deny --manifest-path Cargo.toml check advisories bans licenses sources`
   - `cargo-deny-server|0|...` the same over `server/Cargo.toml` with `--config deny.toml`.
   - `npm-audit-app|0|...` the task 4 command.
   - `negctl-cargo-deny-advisory|0|...`. It builds `target/audit-negctl/` with a one-crate
     `Cargo.toml` (and an empty `src/lib.rs`) that depends on one exact crate version under a RustSec
     advisory. Pick the crate and version from the advisory DB that cargo-deny fetched, and cite the
     advisory id in the comment. The row generates the lockfile (`cargo generate-lockfile`), runs
     `cargo deny ... check advisories` on it, and requires exit non-zero with the advisory id in the
     output. Read the log, not only `$?`.
   - `negctl-cargo-deny-license|0|...`. It copies `deny.toml` to `target/deny-negctl.toml` with
     `MIT` removed from `allow`, runs `check licenses` on the motor workspace with that config, and
     requires exit non-zero with `MIT` in the output.
   - Every row's network need goes in its comment: the advisory DB and the npm registry are fetched at
     run time. Add a `Caveat:` line at the top of the file. These rows can turn red with no change in
     the tree, the day an advisory is published, and that is the purpose. An offline host gets exit
     non-zero, which `gate.sh` reports as FAIL: read the log before calling it a finding.
   - Append the five rows to `scripts/orch/rows/develop-full.rows` at its end, under one `#` banner
     line in that file's style, with the same commands. Do not touch `quick.rows`.
6. **Docs.** In `docs/deploy/service.md`, add a short section "Supply chain" of at most 8 lines: what
   the rows check, the two lockfiles, that an `ignore` needs an issue link, and the rows file's name.

## Allowed paths

- New: `docker/audit.Dockerfile`, `deny.toml`, `scripts/orch/rows/audit.rows`.
- Edits: `scripts/orch/image.sh` (one line), `scripts/orch/rows/develop-full.rows` (append only),
  `docs/deploy/service.md` (one section).
- Also allowed: `prompts/jobs/supply-audit.md` and `target/**`.
- Nothing else: no `Cargo.toml` or `Cargo.lock` under any workspace, no `package.json` or lockfile, no
  `crates/` or `server/` sources. A fix that needs another path is a stop: report it.

Every container goes through the `scripts/orch/` wrappers: never a bare `docker run`, `cargo` or `npm`.

## Run, in order, and paste each exit code and its last 3 lines

1. `scripts/orch/gate.sh target/wf/audit-gate scripts/orch/rows/audit.rows`, then `cat` its `summary.txt`.
2. `scripts/orch/drun-check.sh`

## Return block

- the cargo-deny version and how it was chosen;
- the `cargo deny list` output (task 2);
- both workspaces' `check` exit codes and any advisory (id, crate, version);
- the `npm audit` summary;
- the negctl advisory id and crate;
- the `audit.rows` summary;
- "decisions taken" / "decisions needed".

## Done when

Every one of these holds:
- `audit.rows` runs;
- both negctl rows PASS;
- the three check rows PASS, or each red one is explained by an advisory or npm finding named in
  "decisions needed";
- `drun-check.sh` exits 0;
- `git status --porcelain` shows only the allowed paths.
