# graph-hub slice 5 (hub-report) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. This plan writes one report; it does not gate itself, because a report that gates itself certifies its own author.

**Goal:** `docs/reports/hub-dod.md` — every §8 row and every negative control run on the landing commit, the revision 4 condition (b) diff, the virtual-root diffs of slices 2, 3 and 5, and the numbers §6 asks to be measured.

**Architecture:** The report is prose plus tables, in the shape `docs/reports/service-dod.md` already uses
(six numbered `##` sections, verdict words, `rowname.rows:NN` citations). Nothing here writes Rust or
TypeScript: every task runs an existing gate, reads an existing log or a `git diff`, and fills in a
table cell. A cell that was not measured is written **not measured** with the reason. A negative
control that stayed green is written **as a failure of this slice**, because §8 says so.

**Tech Stack:** `scripts/orch/gate.sh`, `scripts/orch/timed`, `scripts/orch/gr`, `scripts/orch/drun`,
`scripts/orch/node-slim.sh`, `git diff` / `git log` (read-only), and the `docs/measurements/*.md` files
slices 2 and 3 produced. Timed gates run **one at a time**, each under `scripts/orch/timed`, so two
memory- or CPU-bound runs never contend.

**Spec:** `docs/superpowers/specs/2026-10-05-graph-service-plugins-design.md` (revision 5.1; §6
measured numbers, §8 every row and its negative control, §10 slice 5, §12 condition 17,
§14(c) hub-report 1–3, §14(b) the defect diff, §15(c) hub-report 4, §16(b) hub-report 5). Verdict:
`docs/decisions/graph-hub.md`.

## Global Constraints

- One deliverable: `docs/reports/hub-dod.md`. Nothing else is edited. In particular the plan does
  **not** edit `docs/decisions/graph-hub.md` (the ADR's status lines are §16's `S5`, already done in
  revision 5) and does not "fix" a row that fails — a red row is reported red with its log.
- Every claim in the report carries a `file:line` or a log path read **in this slice**, on the landing
  commit. A `file:line` copied from a slice report is cited as *that slice's* tree and named as such.
- A row that was not run is `not run`, never a pass. UNKNOWN is FAIL (§8 preamble).
- A negative control that exited 0 is a **finding against this slice**, recorded in §4 of the report,
  because §8's preamble makes a green control a failure.
- Timed gates run one at a time, each prefixed with `scripts/orch/timed` (the host-wide
  `flock` at `$GM_SCRATCH/orch/timed.lock`, `scripts/orch/timed:8`), never two at once. `hub-memory`,
  `hub-seq`, `hub-epoch` and `hub-pg-durability` all measure memory or timing; running two together
  would make both numbers wrong.
- Verdicts use the words the existing reports use: `met`, `met, with N ruled deviations`,
  `met on a quiet host, not robustly`, `not attempted`, `not run`, `won't fix (YAGNI)`.
- `not measured` is spelled out wherever no `docs/measurements/*.md` file covers the number.

## Decisions recorded here (spec silent or illustrative)

1. The report's §1 "Steps" table is one row per slice 1–5, keyed by slice, not one row per §8 row. The
   full row inventory with every negative control is §5, so a reader can check the two against each
   other. `service-dod.md` keys §1 by step and keeps the row detail in the "Rows" column; the same
   split is kept here.
2. The landing commit is named from the branch tip, since every first-parent subject on this repo is
   the literal string `updated` and a branch name is not greppable from the log
   (`docs/reports/service-dod.md:8-12`). Where the tip cannot be identified, the cell reads
   **landing commit not found** rather than a guess.
3. The virtual-root diff is recorded as the command, the baseline file's sha and the byte counts of
   `git diff --numstat`, plus the `hyper-util` `client-legacy` grep for slices 3 and 5. A diff with no
   output is recorded as `empty`, which is the pass.
4. The condition (b) diff is a table of §14(b)'s `N1`–`N16` with, for each, the commit that carries
   the fix (`git log --oneline <base>..<tip> -- <path>`) and the row that proves it. A defect with no
   row is `not claimed met`.
5. `docs/measurements/hub-memory.md` is slice 3's file and is read, never written here. A number §6
   asks for and that file does not carry is reported as `not measured` with the missing run named.

## Review Focus

1. A green negative control reported as a pass instead of as a failure of this slice
   (`docs/reports/hub-dod.md` §5, the "controls that stayed green" table).
2. A `file:line` copied from a slice report and presented as read on the landing commit
   (§3, "Do spec and code agree?" column).
3. A measured number quoted from the planning arithmetic of §6 rather than from
   `docs/measurements/hub-memory.md` (§2, the memory row).
4. Two timed gates run concurrently, which would make the memory and the seq numbers
   (§5, one `scripts/orch/timed` per row).
5. The virtual-root diff recorded as `empty` without the `client-legacy` grep that §8 names for
   slices 3 and 5 (§4).

---

### Task 1: The report skeleton and §1 Steps

**Files:**
- Create: `docs/reports/hub-dod.md`

**Interfaces:**
- Consumes: `docs/superpowers/specs/2026-10-05-graph-service-plugins-design.md` §10 slices 1–5;
  `docs/reports/service-dod.md` for the shape; `git log --oneline`, `git diff --stat` (read-only).

- [ ] **Step 1: Failing check.** `test -f docs/reports/hub-dod.md` is false and
  `grep -c '^## ' docs/reports/hub-dod.md` fails; there is nothing to fill in. Write the headings
  first, with every cell empty, so the rest of the plan has a place to write.
- [ ] **Step 2: Write the title and preamble.** Copy the shape of `service-dod.md:1-12`: the date, the
  branch, `git rev-parse --short origin/develop` and `git rev-parse --short HEAD`, one sentence per
  slice naming its landing commit, and the sentence that the first-parent subjects are all the literal
  string `updated` so a landing commit is named by tip, not greppable.
- [ ] **Step 3: Write the six `##` headings**, exactly as `service-dod.md` has them, with the
  additions this spec asks for under an existing heading rather than as new ones:

```markdown
# graph-hub DoD — final report

## 1. Steps
## 2. The micro-service
## 3. Resolved questions
## 4. Open review items
## 5. The full gate
## 6. What is not done
```

  The condition (b) diff goes in §3, the virtual-root diffs in §4, the measured numbers in §2, the
  row inventory in §5. No `###` headings, as in both reports read.
- [ ] **Step 4: Fill §1 Steps** with the five-column table `| Step | Verdict | Landing commit | Rows
  (+ negative control) | Measured vs target |`, one row per slice:

  | Step | Verdict | Landing commit | Rows | Measured vs target |
  |---|---|---|---|---|
  | 1. hub-contract | | `git log --oneline -1 hub-contract` | `hub-wire`, `hub-materialize` (pure half), `motor-lock`, `codegen --check` | `to_json` bytes unchanged on every ingest fixture (the pins of Task 1) |
  | 2. hub-store | | `git log --oneline -1 hub-store` | `hub-seq`, `hub-epoch`, `hub-changes-snapshot`, `hub-pg-durability`, `hub-virtual-root`, `svc-supply`, `hub-materialize` (database half) | §4 of this report |
  | 3. hub-api | | `git log --oneline -1 hub-api` | `hub-durability`, `hub-roundtrip`, `hub-authz`, `hub-reload`, `hub-events`, `hub-motor-map`, `hub-idem`, `hub-limits`, `hub-memory`, `hub-breaks-off`, `hub-floor` | `docs/measurements/hub-memory.md` |
  | 4. hub-sdk | | `git log --oneline -1 hub-sdk` | `hub-sync`, `hub-sdk` (+ `negctl-hub-sdk-*`, `GM_HUB_SDK_BREAK=1`) | `hub-sdk.rows` |
  | 5. hub-report | this commit | this file | none of its own | §5 |

  The "Rows" cell cites `rowname.rows:NN` for each row and its `negctl-*` control, the way
  `service-dod.md:19` does. Every empty cell is filled by Tasks 2–6.
- [ ] **Step 5:** the file exists with six `##` headings and the five table rows. Commit.

### Task 2: §2 The micro-service, with the measured numbers

**Files:**
- Modify: `docs/reports/hub-dod.md` (§2)

**Interfaces:**
- Consumes: `docs/measurements/hub-memory.md` (slice 3), `docs/measurements/hub-pg-epoch-probe.md`
  (the spec, revision 3), `docs/contract/hub-api.md`, `docs/deploy/hub.md`,
  `crates/graph-sdk-js/src/hub.ts`, `crates/graph-sdk-js/src/plugin.ts` on the landing commit.

- [ ] **Step 1: Fill the §2 table**, the same five columns, one row per artifact: the hub binary and
  its `deploy/hub.Dockerfile`, the hub keys file and `GRAPH_HUB_GRANTS_FILE` with its 0640 check, the
  `graph-store` migrations and the epoch triggers, the SDK `hub.ts` / `plugin.ts` and the `./hub` and
  `./plugin` exports, the contract module `crates/graph-contract/src/hub.rs`, the example plugin, and
  this report.
- [ ] **Step 2: Fill the memory numbers §6 asks to be measured.** Six cells, each with the
  `docs/measurements/hub-memory.md` line it came from, or the literal string `not measured`:

  | Number | §6 asks | Where it must come from |
  |---|---|---|
  | peak RSS at every cap at once | < 1 GiB under `drun` | `hub-memory.md`, the `drun --memory 1g --memory-swap 1g` run |
  | `F_w` | graph-server's measured 18.25 replaced by the hub's own | `hub-memory.md` |
  | `max_header` | one change header row as read, planning estimate 256 B | `hub-memory.md` |
  | last-seen map entry size | planning estimate 256 B | `hub-memory.md` |
  | `/layout` upload of a `GRAPH_HUB_MAX_DOC_BYTES` workspace | under graph-server's 10 s body timeout (`body.rs:20`) | `hub-memory.md`; a miss is a stop (§5.3) |
  | the planning total | about 425 MiB plus `base` and the `IO_BUF` terms | recomputed with the measured values and compared |

  The `F_w` cell must quote the planning value it replaces (`server/graph-server/src/config/slots.rs:22`)
  and the measured value beside it. A cell with no measured file behind it reads `not measured` and
  §6 repeats, in the same row, why the planning number is not a measurement.
- [ ] **Step 3: Fill the rest of §2**, each claim with a `file:line` read on the landing commit: the
  `GRAPH_HUB_*` defaults the running hub resolved (from `docs/deploy/hub.md` and the row `hub-limits`),
  the runbook's `UPDATE workspaces SET epoch = hub_next_epoch();`, and the six §5.3 restore-detector
  cases with what each measured (`docs/measurements/hub-pg-epoch-probe.md` `out.txt`, `out2.txt`,
  `out3.txt`, `run3.sh`).
- [ ] **Step 4:** §2 has no empty cell and every `not measured` has a reason. Commit.

### Task 3: §3 Resolved questions, and the condition (b) diff

**Files:**
- Modify: `docs/reports/hub-dod.md` (§3)

**Interfaces:**
- Consumes: the spec §12 (15 conditions), §13 (18 conditions, `D1`–`D13`), §14(a), §14(b) (`N1`–`N16`),
  §15(a) (`R1`–`R11`), §16(a) (`S1`–`S7`); `docs/decisions/graph-hub.md`; the landing commit's
  `crates/graph-contract/src/hub.rs`, `server/graph-store/src/`, `server/graph-hub/src/`.

- [ ] **Step 1: Fill the §3 table** `| Question | What the lines say | Do spec and code agree? |`,
  one row per condition of §12, §13, §14(a) and §16(b), each citing the spec line **and** the code
  line read on the landing commit, and answering `yes`, `no`, or `no — ruled deviation` with the
  deviation named. Conditions 1–15 of §12 and 1–18 of §13 are the rows; §14(a)'s nine partly-met
  revision 2 conditions and §16(b)'s hub-sdk condition 6 are the rest.
- [ ] **Step 2: Write the condition (b) diff** as its own table under §3, one row per §14(b) defect
  `N1`–`N16`:

```markdown
### The revision 3 → 4 condition (b) diff

| Defect | Fixed by | Commit | Row that proves it | Verdict |
|---|---|---|---|---|
| N1 trigger DDL refused; `TRUNCATE`, replica and restore bypass | §5.3 one trigger per event plus `TRUNCATE`, `ENABLE ALWAYS`; `hub.writer` on every write path; `hub_meta` detector; runbook | `git log --oneline <base>..<tip> -- server/graph-store` | `hub-epoch` | |
```

  `Fixed by` is the spec's own text, not a paraphrase. `Commit` is the first commit touching the named
  path that is not `updated`-noise, or **not found**. A defect whose row did not run is
  `not claimed met`, never `met`.
- [ ] **Step 3: Add §15(b)'s deviation as its own paragraph** under §3, quoted: the verdict's fix sent
  `event: resync` on a mid-stream pool timeout and revision 5 sends `event: busy` instead, because the
  cursor is still valid and a resync makes every subscriber read the whole `/graph` while the pool is
  already short. Name the row that pins it (`hub-events`, the `busy` cases) and the SDK row
  (`hub-sdk`). Say plainly that this is a **deviation from the verdict's fix**, accepted in revision 5,
  not a defect found here.
- [ ] **Step 4:** every `N1`–`N16` row has a verdict and every verdict names a row or says why none
  does. Commit.

### Task 4: §4 Open review items and the virtual-root diffs

**Files:**
- Modify: `docs/reports/hub-dod.md` (§4)

**Interfaces:**
- Consumes: the review files under `docs/reviews/` that name the hub (`graph-hub.md` and any
  successor); `git diff` against the pre-slice-2 baseline; `scripts/orch/gr cargo tree`.

- [ ] **Step 1: Write the severity prose**, the way `service-dod.md:48-50` does: the counts by
  severity, then the table `| Review | Verdict | Id | Severity | Finding |`, then the closing
  paragraph that names everything **not** listed (the reviews run, and the findings closed before this
  tree).
- [ ] **Step 2: Run and record the virtual-root diff for slice 2.** On the landing commit, from the
  baseline the slice-2 brief names:

```sh
scripts/orch/gr cargo tree --manifest-path server/Cargo.toml -e features --locked > /tmp/opencode/vr-base.txt
scripts/orch/gr cargo tree --manifest-path server/Cargo.toml -e features --locked > /tmp/opencode/vr-final.txt
diff /tmp/opencode/vr-base.txt /tmp/opencode/vr-final.txt | wc -l
```

  Record the command, the two line counts, the diff's byte count, and `empty` or the diff itself. Then
  the negative control the spec names (§8, `hub-virtual-root`): the scratch copy **without**
  `default-members`, in which `graph-store`'s `tokio` dependency also enables a feature absent from the
  baseline's graph-server subtree — record the injected feature's **name** (the slice-2 report names
  it; if the slice-2 report does not, this is a `not claimed met` and a `decisions needed` entry) and
  show graph-server's subtree differs. `R5` and §16(b) condition 18 make the feature name mandatory.
- [ ] **Step 3: Re-run the same diff for slices 3 and 5.** §10 slice 3 re-runs it with
  `client-legacy` present, so the cell must also carry the grep:

```sh
scripts/orch/gr cargo tree --manifest-path server/Cargo.toml -e features --locked | grep -c 'hyper-util .*client-legacy'
```

  Record the count for the baseline, for slice 3 and for this landing commit. A count that is 0 for
  slice 3 is `not claimed met`; §8 names the grep as part of the row.
- [ ] **Step 4: Record graph-server's lock entries.** `scripts/orch/gr cargo tree -p graph-server -e
  normal,build --locked` before and after, and `git diff --exit-code <base> -- server/Cargo.lock` on the
  graph-server subtree only: "no version change in graph-server's lock entries" (condition (e)). Record
  the exit and the `git diff --stat` output.
- [ ] **Step 5:** §4 carries the three virtual-root diffs, the three `client-legacy` counts and the
  lock-entry result, each with its command. Commit.

### Task 5: §5 The full gate — every row, every negative control

**Files:**
- Modify: `docs/reports/hub-dod.md` (§5)

**Interfaces:**
- Consumes: `scripts/orch/rows/hub-*.rows` (slices 1, 3, 4), `scripts/orch/rows/service-supply.rows`,
  `scripts/orch/rows/svc-floor.rows`, `scripts/orch/gate.sh`, `scripts/orch/timed`.

- [ ] **Step 1: List the inventory.** Every row of §8, by name, with the rows file and line it is on:
  `hub-wire`, `hub-materialize`, `hub-floor`, `hub-virtual-root`, `svc-supply`, `hub-seq`, `hub-epoch`,
  `hub-changes-snapshot`, `hub-pg-durability`, `hub-durability`, `hub-roundtrip`, `hub-authz`,
  `hub-reload`, `hub-events`, `hub-motor-map`, `hub-idem`, `hub-limits`, `hub-memory`,
  `hub-breaks-off`, `hub-sync`, `hub-sdk`, plus `cargo-deny-server`, `lock-parity`, `svc-features`,
  `codegen --check`, the graph-core wasm32 build, the `svc-*` rows and `motor-lock`. One line per row,
  found with `grep -n` in its rows file so the `:NN` is real and not remembered.
- [ ] **Step 2: Run them, one at a time.** Each row in its own log directory, each command prefixed
  with `scripts/orch/timed`:

```sh
scripts/orch/timed scripts/orch/gate.sh /tmp/opencode/hub-dod/<row> scripts/orch/rows/<rows-file>.rows
```

  Never two at once: `hub-memory` measures RSS under `drun --memory 1g`, `hub-seq` and `hub-epoch`
  measure timing and row counts, and a concurrent gate would make those numbers wrong. The timed
  wrapper's lock (`scripts/orch/timed:8`) serialises them anyway; running them one at a time is what
  makes a failure readable.
- [ ] **Step 3: Fill the gate table** `| Row | Expect | Exit | Verdict | Evidence |`, the column set
  `docs/reports/perf-p5.md:60` uses, one line per row with the real exit code and the log path.
  `Expect` is the rows file's own second field. A row whose rows file does not exist is `not run` with
  the missing file named — never a pass.
- [ ] **Step 4: Run every negative control, one at a time, and write a second table.** The controls are
  §8's third column plus the ones §10 and §14/§15/§16 name: `lax-reader`, `keep-dangling`,
  `keep-cells`, the unformatted-file scratch copy, `changes-read-committed`, `sequence-seq`,
  `sync-commit-unset`, `ack-before-commit`, `drop-record`, `skip-grant`, `reload-keys-only`,
  `skip-event`, `layoutfailed-as-502`, `no-idem`, `no-cap`, the `hub-memory` cap row, the `negctl`
  edge, `lsn-only`, `hw-after-lsn`, `one-trigger-origin`, `trigger-enable-origin`, `detector-at-start`,
  `no-deadlock-retry`, the `trigger dropped`, `sync-via-graph`, `GM_HUB_SDK_BREAK=1`, and the four
  `negctl-hub-sdk-*` unit rows. Table: `| Control | Break | Row it must turn red | Exit | Verdict |`.

```markdown
### Controls that stayed green

| Control | Row | Exit | Why this is a failure of this slice |
|---|---|---|---|
| | | | |
```

  That table is **empty only if every control went red**. §8's preamble: a green negative control is
  recorded as a failure. A non-empty table is a `met, with N ruled deviations` verdict at best, and
  each row names the commit that must fix it.
- [ ] **Step 5: Record the timed rows** as `service-dod.md:75-82` does: `| Row | Time | Line it ends on |`
  for every row over a minute, read from the log's own last line, and the wall-clock window of the
  whole run with the number of rows and the PASS count.
- [ ] **Step 6: Record the red-row history**, `service-dod.md:102-110`'s
  `| Red row | Exit | Cause (its log) | Repair |`, one line per row that went red at least once
  during this slice's runs. A run that was green first time has no line here, and that is stated.
- [ ] **Step 7:** §5 has a row for every §8 row and every negative control, each with a real exit and
  a log path. Commit.

### Task 6: §6 What is not done, and the verdict

**Files:**
- Modify: `docs/reports/hub-dod.md` (§6)

**Interfaces:**
- Consumes: §9 of the spec (every deferral), the `docs/measurements/` files, the §5 inventory.

- [ ] **Step 1: Write §6** as `| Item | Verdict | Why |`, one row per §9 deferral and per claim this
  report cannot make: the Redis and snapshot caches, two or more hub instances and `LISTEN/NOTIFY`,
  pull and webhook plugins, edge properties, a query language, `/graph?at=`, the HTTP deletions and
  manifest restructuring, TLS and publishing beyond the host, per-key write quotas, `pgvector`, live
  studio integration, `motor-alone` (which §8 says is not defined in any rows file today), and every
  `not measured` cell of §2 and §5. Each verdict is one of the report's own words and the `Why` cites
  the spec line.
- [ ] **Step 2: Write the verdict paragraph**, `service-dod.md:59-65`'s shape: every §8 row, every
  negative control, the condition (b) diff, the three virtual-root diffs and the measured numbers, each
  with its verdict, then the one sentence that says what a reader may rely on and what they may not.
- [ ] **Step 3: Cross-check the whole report against §8 before committing.** A checklist run by hand:
  every §8 row name appears in §5; every §8 negative control appears in §5's control table; no cell
  says `met` without a `file:line` or a log path; no `not measured` lacks a reason; the six §2
  numbers are all present; §1 has five rows; §3 has a verdict for every `N1`–`N16`; §4 has three
  virtual-root diffs and three `client-legacy` counts.
- [ ] **Step 4:** commit (`updated`). Do not run a gate; this slice gates nothing.

## Self-review

- Spec §10 slice 5 items → task: every row and every negative control on the landing commit, a green
  negative control is a failure (Task 5); the condition (b) diff and the virtual-root diffs of slices
  2, 3 and 5 (Tasks 3 and 4).
- §12 condition 17 (every row and negative control on the landing commit) → Task 5 steps 2–4.
- §14(c) hub-report 1 (green control is a failure) → Task 5 step 4's own table; 2 (the (b) diff and the
  virtual-root diffs) → Tasks 3 and 4; 3 (the verdict recorded in the ADR) → Task 6 step 2 names it,
  and the ADR itself is not edited here because §16's `S5` already set its status.
- §15(c) hub-report 4 (the revision 4 and revision 5 verdicts recorded in the ADR) → Task 3 step 1's
  table cites both verdicts with their `docs/decisions/graph-hub.md` lines; the edit is not this
  slice's to make.
- §16(b) hub-report 5 (`S5`, done in revision 5; the verdict recorded in the ADR) → Task 3 step 3,
  which records the accepted §15(b) deviation.
- §6 measured numbers → Task 2 step 2: peak RSS, `F_w`, `max_header`, the last-seen entry size, the
  `/layout` upload against the 10 s body timeout, and the planning total recomputed.
- Report shape → Task 1 step 3, copied from `docs/reports/service-dod.md` (`## 1. Steps` … `## 6. What
  is not done`) with `docs/reports/perf-p5.md:60`'s gate-table columns for exit codes.
- Path: `docs/reports/hub-dod.md` only. `docs/decisions/graph-hub.md` is read, not written;
  `docs/measurements/*.md` are read, not written; `docs/contract/*.md`, `server/**`, `crates/**`,
  `scripts/orch/rows/**` and the root `package.json` are read or run, never edited.
- Not run here, and named as such: nothing in this plan gates itself. §5's exits are the only evidence
  this slice produces, and they are runs on the landing commit, recorded whether green or red.