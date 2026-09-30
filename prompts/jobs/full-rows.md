# Job full-rows (agent build, one new file)

Why: the full gate on develop (`prompts/RESUME.md`, "First tasks" item 2 and "Remaining" item 3) ran
from `develop-full.rows`, `p6e-full.rows`, `p3` rows and `p12-t1.rows` kept under `/sgoinfre`, which
this host no longer has. They were never committed. Rebuild them as ONE committed file,
`scripts/orch/rows/develop-full.rows`, from what the tree still records.

Row format (read `scripts/orch/gate.sh` and `scripts/orch/rows/quick.rows` first): one row per line,
`name|expect|cmd`, `expect` is `0` or `nonzero`; `cmd` runs from the worktree root with bash. Lines
starting `#` are comments. Every command goes through `scripts/orch/gr` (or `scripts/orch/node-slim.sh`,
or `docker run --rm --pull never -v "$PWD:/w" -w /w ge-python-oracle ...` for Python oracles).

Sources, in this order (the tree and `--help` are the final authority on syntax):
- `scripts/orch/rows/quick.rows`: start from it, keep its rows unchanged.
- The gate tables in `docs/reports/phase-03.md` … `phase-08.md`, `phase-09-progress.md`,
  `phase-10-progress.md`, `phase-11-progress.md`, `phase-g0.md`: every row name with its command.
- `prompts/RESUME.md` "Remaining" item 3: the bench rows rewritten to the unified CLI
  (`bench --layout X --n 220,10000,100000 [--past-ceiling] [--vs-d3]`; the old `--nodes` form is
  gone), `stress --oracle d3 --seeds 1000`, the FA2 three-step rows, `hashgate --seeds 1000`, and the
  `no-mul-add` row that greps `mul_add[[:space:]]*[(]` (its first version matched comments).
- The knob list, which exists once: `crates/graph-cli/tests/common/mod.rs` (10 `GM_MUTATE_*` knobs on
  develop). One `negctl-<knob>` row per knob, `expect nonzero`, each running `hashgate --seeds 8` with
  that knob set (see quick.rows' negctl row for the shape; `gr -e NAME=VALUE`). quick.rows' own negctl
  pipes into `test $? -eq 1`; the knob rows use `nonzero` instead.
- p11 (`docs/reports/phase-11-progress.md`, `prompts/phase-11-*.md`): the threads arms.
- p12-t1 (`git log --oneline origin/develop | grep -i` and `docs/`): the closed-form oracle command
  (`oracle-closed-form` or the name `--help` shows).
- `docs/reports/phase-09-progress.md`: the 10^5/10^6 bench arms time out on this class of host; keep
  them as rows but put them last, under a `# slow:` comment.

Check each command's syntax, never its result: `scripts/orch/gr cargo run -q -p graph-cli -- --help`
and `-- <subcommand> --help` (cheap). Do NOT run hashgate above 8 seeds, mutants, bench, stress or any
row itself: the orchestrator runs the file once, under `scripts/orch/timed`.

Paths you may touch: `scripts/orch/rows/develop-full.rows` (new). Nothing else.

Done when:
- every row's subcommand and flags appear in the matching `--help` output;
- above each group of rows, one `# from <file>:<line>` comment names its source;
- a row whose command cannot be recovered from the tree is left out and listed in the return block
  as `missing: <row name> — <why>` (a missing reference is a stop, not an improvisation);
- the return block lists the row count per group and every `--help` you ran.
