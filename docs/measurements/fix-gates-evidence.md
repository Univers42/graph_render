# fix-gates-evidence — repairs to the ledger, its evidence and the fingerprint

Review: `docs/reviews/review-gates.md` (`RG-NN`). Job: `prompts/jobs/fix-gates-evidence.md`, rules
`prompts/jobs/fix-common.md`.

Every change is inside the job's five paths (`crates/graph-cli/src/capabilities.rs`,
`capabilities/**`, `fingerprint.rs`, `evidence.rs`, `codegen.rs`) — 18 files, all under
`crates/graph-cli/src/`. **No `graph-core` file is touched**, so no motor byte moved and
`hashgate --seeds 8` was not re-run (see "Commands" for the diff that shows it).

`origin/develop` was merged in mid-job; this report covers the merged tree (see "Before and after"
for the re-taken baseline). Every comparison below is against **`9178255`, develop as merged**, with
this job's files reverted to it — the tree these repairs sit on top of.

## Verdicts

| id | severity | verdict | test | file:line |
|---|---|---|---|---|
| RG-02 | BLOCKER | fixed | `a_gated_row_that_names_no_oracle_function_is_refused` | `crates/graph-cli/src/capabilities/tests/refusals.rs:186` |
| RG-03 | BLOCKER | fixed | `the_boundary_is_the_wrappers_and_not_the_rest_of_scripts` (+ `an_edit_to_any_gate_wrapper_moves_the_fingerprint`, `an_edit_outside_the_list_moves_nothing`) | `crates/graph-cli/src/fingerprint.rs:206`, `:190`, `:173` |
| RG-08 | MAJOR | fixed | `a_measured_cell_above_the_declared_ceiling_is_refused` (+ `the_columns_are_read_by_the_headers_names_not_by_position`) | `crates/graph-cli/src/capabilities/tests/scale.rs:107`, `:134` |
| RG-17 | MAJOR | fixed | `a_failing_run_never_overwrites_a_passing_record` (+ `a_stale_passing_record_from_another_tree_does_not_block_this_trees_run`) | `crates/graph-cli/src/evidence/tests.rs:129`, `:227` |
| RG-23 | MAJOR | fixed | `every_row_family_is_reached_by_the_one_registry` | `crates/graph-cli/src/capabilities/tests/registry.rs:154` |
| RG-24 | MAJOR | fixed | `a_layout_id_no_arm_names_is_implemented_and_names_no_record` (+ `every_registered_layout_is_named_by_one_of_the_arms`) | `crates/graph-cli/src/capabilities/tests/registry.rs:189`, `:219` |
| RG-28 | MAJOR | fixed | `a_committed_generated_file_the_generator_no_longer_emits_is_a_finding` (+ `a_hand_written_document_beside_them_is_not_an_orphan`) | `crates/graph-cli/src/codegen.rs:113`, `:145` |
| RG-29 | MINOR | fixed | `a_scale_ceiling_above_the_largest_this_tree_declares_is_refused` | `crates/graph-cli/src/capabilities/tests/refusals.rs:73` |
| RG-30 | MINOR | fixed | `a_row_that_names_no_oracle_is_refused` | `crates/graph-cli/src/capabilities/tests/refusals.rs:95` |
| RG-31 | MINOR | fixed | `the_ceilings_table_is_read_by_check_and_a_missing_doc_is_a_finding` | `crates/graph-cli/src/capabilities/tests/ledger.rs:51` |
| RG-32 | MINOR | fixed | `every_row_says_whether_a_recorded_run_backs_it` | `crates/graph-cli/src/capabilities/tests/ledger.rs:8` |
| RG-33 | MINOR | fixed | `every_ingest_row_names_the_one_stage_this_file_declares` | `crates/graph-cli/src/capabilities/tests/registry.rs:236` |
| RG-34 | MINOR | fixed, both halves | `each_bundle_row_is_named_by_the_module_whose_metadata_it_carries` (id↔META pairing) + `a_row_whose_geometry_kind_this_ledger_cannot_name_is_refused` (catch-all) | `crates/graph-cli/src/capabilities/tests/registry.rs:261`, `tests/refusals.rs:110` |
| RG-50 | MINOR | fixed | `the_required_field_refusals_are_named_and_not_merely_counted` (+ the by-id read in `a_function_without_cases_or_with_an_unexplained_mismatch_is_refused`) | `crates/graph-cli/src/capabilities/tests/refusals.rs:130`, `:150` |
| RG-53 | MINOR | fixed, all three holes | `a_symlink_under_a_fingerprinted_path_is_refused_not_followed` (symlink) + `a_record_is_renamed_into_place_and_leaves_no_temporary` (atomic write) + `an_unparseable_record_reads_as_absent_and_a_re_run_repairs_it` and `an_absent_directory_is_empty_and_an_unparseable_record_is_absent_not_fatal` (parse error = absent) | `crates/graph-cli/src/fingerprint.rs:230`, `crates/graph-cli/src/evidence/tests.rs`, `crates/graph-cli/src/capabilities/verdict/records/tests.rs` |

### What is not closed

**Nothing in RG-53 is outstanding.** The first version of this report deferred two of the finding's
items; the orchestrator ruled on both (2026-10-03) and they are now done:

1. ~~*"treat a parse error on read as a failed record rather than a fatal error"*~~ — **done**, and
   it took two readers, not one. `read_from` (`evidence.rs:236`) is the by-name reader; the ledger's
   directory scan `capabilities::verdict::records::all` had its own parser (`read_one`,
   `records.rs`), and the CLI demonstration below is what caught it: with only `read_from` fixed,
   `capabilities --check` still exited 2. Both now name the file on stderr and return "no record",
   and both keep a genuine **unreadable** path (a directory named `<name>.json`) as an error. See
   "The two RG-53 decisions" below for the tests.
2. ~~*"`GM_GATES_DIR` is an unvalidated override"*~~ — **kept, and named.** The override stays (the
   tests and the gate rows depend on it: `crates/graph-cli/tests/common/mod.rs:86`), and
   `GATES_ENV` now carries a `Ponytail:` line stating that it is trusted input, what a hand-written
   record with this tree's fingerprint implies, and what the escape hatch is.

## RG-03: what the fingerprint boundary covers

The boundary is the transitive closure of the two roots that change a gate's result, and nothing
else (`GATE_SCRIPTS`, `fingerprint.rs:35-41`; declared in `FINGERPRINTED` at `:54-73`):

| file | why it is in the closure |
|---|---|
| `scripts/orch/gr` | root 1 — every gate row runs through it, and it builds the binary a record is evidence for |
| `scripts/orch/image.sh` | `gr:7` sources it (image selection and build) |
| `scripts/orch/scratch.sh` | `image.sh:11` sources it, and `scigraphs-conformance.sh:80` sources it (`$GM_SCRATCH`, the pinned references the oracle reads) |
| `scripts/orch/docker-env.sh` | `image.sh:12` sources it (finds the daemon) |
| `scripts/scigraphs-conformance.sh` | root 2 — it is itself a gate and writes `target/gates/scigraphs-conformance.json` |

A blanket `scripts` entry was **not** taken (the job body forbids it and the review allows the
narrower form): `scripts/orch/queue.txt`, `scripts/orch/gate.sh`, `scripts/orch/mutants.sh` and
the `studio-*.sh` wrappers change hourly and would void every recorded run for an edit that cannot
move a motor gate's byte. `the_boundary_is_the_wrappers_and_not_the_rest_of_scripts` pins both
halves — every wrapper in, two hourly orchestration files out.

### Adding these paths voids today's records — expected, and nothing was re-recorded

`target/gates/` held **no records at all** when the before/after comparison below was captured
(`ls target/gates/` → empty; the only file in it now is the `scigraphs-conformance.json` this
job's own merge-floor run wrote, after that comparison). So the fingerprint change voided nothing
in practice. Had records existed they would now read as stale,
which is the intended effect and the reason the job body says not to re-record from a fix job.
No gate was re-run to refresh evidence in this job; the only record written is the one
`scripts/scigraphs-conformance.sh` writes in its normal course as the merge-floor check, after the
before/after comparison below was captured.

## Negative controls

How each RED was obtained. A finding whose fix restores behaviour the untouched code already had
cannot fail on the untouched tree; those are named as such rather than claimed as RED.

### Batch 1 — RG-02, RG-03, RG-17, RG-53 (symlink)

Each fix hunk reverted, then the whole bin target's `evidence`, `fingerprint` and
`capabilities::tests::refusals` tests run:

```
failures:
    capabilities::tests::refusals::a_gated_row_that_names_no_oracle_function_is_refused
    evidence::tests::a_failing_run_never_overwrites_a_passing_record
    evidence::tests::a_stale_passing_record_from_another_tree_does_not_block_this_trees_run
    fingerprint::tests::a_symlink_under_a_fingerprinted_path_is_refused_not_followed
    fingerprint::tests::the_boundary_is_the_wrappers_and_not_the_rest_of_scripts

---- capabilities::tests::refusals::a_gated_row_that_names_no_oracle_function_is_refused ----
thread '…' panicked at crates/graph-cli/src/capabilities/tests/refusals.rs:132:5:
an empty differential backs nothing: []

---- evidence::tests::a_failing_run_never_overwrites_a_passing_record ----
thread '…' panicked at crates/graph-cli/src/evidence/tests.rs:172:5:
a body with no `pass` is not a record

test result: FAILED. 15 passed; 5 failed; 0 ignored; 0 measured; 317 filtered out; finished in 0.43s
```

RG-02's is the review's own probe (`PROBE hollow topology.index functions=[] problems=[]`) turned
into a test: an empty `functions` is refused **in `verdict::oracle_diff`**, once, so no caller can
promote a hollow row to `gated`. RG-17 shows both halves at once — a body that spells its outcome
anywhere but a boolean `pass` is not a record, and a passing record from another tree neither
blocks this tree's write nor stands in for it. RG-03 fails on the boundary, RG-53 on the symlink.

### Batch 2 — RG-23, RG-24, RG-28

```
failures:
    capabilities::tests::registry::a_layout_id_no_arm_names_is_implemented_and_names_no_record
    capabilities::tests::registry::every_registered_layout_is_named_by_one_of_the_arms
    capabilities::tests::registry::every_row_family_is_reached_by_the_one_registry
    codegen::tests::a_committed_generated_file_the_generator_no_longer_emits_is_a_finding

---- capabilities::tests::registry::a_layout_id_no_arm_names_is_implemented_and_names_no_record ----
thread '…' panicked at crates/graph-cli/src/capabilities/tests/registry.rs:197:5:
assertion `left == right` failed: not gated by inheritance
  left: Gated
 right: Implemented

---- capabilities::tests::registry::every_registered_layout_is_named_by_one_of_the_arms ----
thread '…' panicked at crates/graph-cli/src/capabilities/tests/registry.rs:212:9:
layout.grid: no arm names it

test result: FAILED. 10 passed; 4 failed; 0 ignored; 0 measured; 328 filtered out; finished in 0.03s
```

`every_row_family_is_reached_by_the_one_registry` is RG-23's second half: on the untouched tree
`capabilities::registry()` extends three families that `registry::registry()` never chains, so the
list `--check` iterates and the published list were different lists; the test now asserts they are
the same list, in the same order, and that each of the seven stages is reached.

### Batch 3 and batch 4 — RG-08, RG-29, RG-30, RG-34, RG-31, RG-32, RG-50, RG-33

Batch 3's run was observed failing in the session only (its output was never written to a file),
so it is **re-run as batch 4**, with the output kept. Two kinds of control, because two kinds of
finding:

**(a) The fix hunk reverted** — the untouched tree's behaviour. Seven hunks, one test each, so
nothing fails for a neighbour's reason:

```
    capabilities::tests::ledger::every_row_says_whether_a_recorded_run_backs_it
    capabilities::tests::ledger::the_ceilings_table_is_read_by_check_and_a_missing_doc_is_a_finding
    capabilities::tests::refusals::a_row_that_names_no_oracle_is_refused
    capabilities::tests::refusals::a_row_whose_geometry_kind_this_ledger_cannot_name_is_refused
    capabilities::tests::refusals::a_scale_ceiling_above_the_largest_this_tree_declares_is_refused
    capabilities::tests::scale::a_measured_cell_above_the_declared_ceiling_is_refused
    capabilities::tests::scale::the_columns_are_read_by_the_headers_names_not_by_position

test result: FAILED. 42 passed; 7 failed; 0 ignored; 0 measured; 302 filtered out; finished in 0.01s
```

The reverts were: the measured-above-declared comparison (`ceilings.rs`), the header-named columns
back to `cells[1]`/`cells[3]`, the ceiling bound at `MAX_SCALE_CEILING`, `oracle` out of the
required-field sweep, the unnamed-geometry refusal, `check_findings` back to `problems()` alone
(`--check` not reading the ceilings doc), and `how_backed` back to printing nothing. What each test
printed, verbatim:

```
tests/ledger.rs:16    assertion `left == right` failed          RG-32:  left: ""
                      right: "backed by a recorded run on this tree"
tests/ledger.rs:61    []                                        RG-31:  a missing ceilings doc produced
                                                                no finding at all
tests/scale.rs:116    []                                        RG-08:  a measured cell above its
                                                                declared ceiling produced no finding
tests/scale.rs:137    assertion `left == right` failed: `id` is column 3 here, not column 1
                      left: (0, 1)                              RG-08:  the reordered table measured
                                                                nothing and measured one row reasoned
tests/refusals.rs:77  []                                        RG-29:  u64::MAX produced no finding
tests/refusals.rs:98  a row with no reference claims a differential it does not have   RG-30
tests/refusals.rs:113 an unnamed kind is a finding, not a borrowed name                 RG-34
```

The three `[]` lines are the tests' own `{found:?}`: the empty problem list is the defect, not a
missing message.

**(b) The counterfactual the untouched code allowed** — for the findings whose fix changes no
behaviour, so no reversion can fail. RG-50's by-content assertion, RG-34's id↔META pairing and
RG-33's shared stage are pinned by mutating exactly what the review said could happen:

| mutation | test | result |
|---|---|---|
| one required-field message reworded (`is empty` → `was empty`), count still 3 | `the_required_field_refusals_are_named_and_not_merely_counted` | **FAILED** |
| the same mutation | `empty_required_fields_zero_ceiling_and_duplicate_ids_are_refused` (count-only, the old assertion) | **ok** — a count cannot see it, which is the finding |
| bundle `META`s swapped between the two ids — the mispairing `zip` allowed | `each_bundle_row_is_named_by_the_module_whose_metadata_it_carries` | **FAILED** |
| one ingest row's stage changed to its own name — the claim RG-33 describes | `every_ingest_row_names_the_one_stage_this_file_declares` | **FAILED** |

```
test result: FAILED. 43 passed; 6 failed; 0 ignored; 0 measured; 302 filtered out; finished in 0.01s
---- capabilities::tests::registry::every_ingest_row_names_the_one_stage_this_file_declares ----
assertion `left == right` failed: ingest.roles
  left: "ingest.roles"
```

Four of the six failures are collateral (the reworded message also breaks RG-30's content string,
the swapped `META` also breaks `a_post_row_carries_the_metadata_its_own_module_declares`, the stage
change also breaks the routing registry test); they are named here rather than counted as controls.

### Stated plainly: three tests are regression guards, not RED

- **RG-33's shared stage.** The fix names one `INGEST_STAGE` constant instead of writing
  `"ingest.build"` four times. The behaviour is identical before and after, so there is no RED on
  the untouched code and this job claims none: the constant does not exist there, so the test
  cannot even compile against it. The mutation control above is what makes the guard non-vacuous.
- **RG-34's id↔META pairing.** `bundles()` now takes `(fdeb::ID, fdeb::META)` pairs instead of
  zipping two arrays whose order happened to match. On the untouched tree the test would pass —
  the zip is correct today — so this is a guard against a future edit, and the swap mutation above
  is the only way to make it red.
- **RG-53's atomic write.** `a_record_is_renamed_into_place_and_leaves_no_temporary` passes on the
  untouched code too: `fs::write` in place leaves no `.tmp` behind either, because the untouched
  code never made one. Its subject is the guarantee, not a behaviour that differed before.
  (`fs::write`'s truncation window is exactly what the atomic write closes, but no test can observe
  a window that is only observable under a kill mid-write.)

## The two RG-53 decisions (orchestrator ruling, 2026-10-03)

Both deferred items were accepted and are implemented. They needed two tests, because the rule had
**two** readers, and only one of them showed up in the unit tests.

**RED, `read_from` (by name), `evidence/tests.rs`:**

```
---- evidence::tests::an_unparseable_record_reads_as_absent_and_a_re_run_repairs_it ----
panicked at crates/graph-cli/src/evidence/tests.rs:120:5:
assertion `left == right` failed: a record nobody can parse is not evidence, and is not a fatal error either
  left: Err("/tmp/gm-evidence-corrupt-302/hashgate.json: EOF while parsing a value at line 1 column 24")
 right: Ok(None)

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 367 filtered out
```

**RED, `records::all` (the ledger's directory scan), `verdict/records/tests.rs`:**

```
---- capabilities::verdict::records::tests::an_absent_directory_is_empty_and_an_unparseable_record_is_absent_not_fatal ----
panicked at crates/graph-cli/src/capabilities/verdict/records/tests.rs:70:27:
read: one mangled file is not a whole-ledger failure: "/tmp/graph-cli-records-16-ThreadId(2)/oracle-osage.json: expected ident at line 1 column 2"

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 367 filtered out
```

Two existing tests pinned the *old* rule and were updated with the decision, not weakened: the
`torn.json` assertion in `a_record_reads_back_as_written_and_only_absence_is_none` now expects
`Ok(None)`, and `an_absent_directory_is_empty_and_a_file_that_is_not_a_record_is_refused` was
renamed to `…_an_unparseable_record_is_absent_not_fatal` and rewritten. Each new test carries the
control that must **stay** an error: a path that is a *directory* is unreadable, not unparseable.

**What the unit tests did not catch, and the CLI run did.** With `read_from` fixed and
`records::read_one` not, a corrupt record in the real gates directory still took the whole ledger
down — `gr` mounts only the repo at `/w`, so the demo has to write inside `target/gates/`:

```
$ printf '{ "pass": true, "seeds":' > target/gates/hashgate.json      # (inside the container)
$ ./target/release/graph-cli capabilities --check
  /w/crates/graph-cli/../../target/gates/hashgate.json: not a record (EOF while parsing a value at line 1 column 24); it backs nothing, and is left in place
  /w/crates/graph-cli/../../target/gates/hashgate.json: not a record (EOF while parsing a value at line 1 column 24); it backs nothing, and is left in place
  …
capabilities --check: 73 rows, 36 problems
EXIT=1        # was: EXIT=2, "capabilities: reading the gate records: …EOF while parsing a value"
```

Two lines because two readers report it — the by-name read and the directory scan. The count stays
36, so the mangled record backs nothing at all, and the file is left where it is. (The demo file is
removed in the same invocation; `target/gates/` afterwards holds only the conformance record.)

## Before and after: `capabilities --check`

**Re-captured after the merge.** `origin/develop` was merged into this branch mid-job
(`502fa3d`, parents `3ce8db1` this branch + `9178255` develop; the `capabilities.rs` conflict
resolved in develop's favour for the scale rows, which now live in `capabilities/scale.rs` with
`oracle_record: oracle-scale` and their functions). Develop's rows and this job's repairs coexist —
`registry.rs:171` chains `super::scale::rows()` into the one row source — and the whole bin suite
passes on the merged tree (368 tests).

So the baseline is re-taken as **`9178255` (develop as merged), with this job's 18 files reverted to
it**: that is the tree this job's repairs sit on top of, and the only delta between the two builds
is this job. (The pre-merge comparison, against `45fca10`, gave the same result — 36 problems either
way, the difference being only RG-32's verdict lines — with 72 rows; develop has since added one.)

| | rows | exit | last line |
|---|---|---|---|
| baseline (`9178255`, develop) | 73 | 1 | `capabilities --check: 73 rows, 36 problems` |
| this tree (merged, repaired, + the two RG-53 decisions) | 73 | 1 | `capabilities --check: 73 rows, 36 problems` |

**The 36 problems are identical, line for line**: the `diff` of the two outputs is `73` added
lines and **zero** changed or removed. **The one difference is deliberate and named: RG-32 adds one
verdict line per row before the problems** — 73 lines like
`gated       layout.tree.tidy   no evidence: nothing recorded backs it`. Those are not problems and
are not counted in the summary line; they are the finding's requested output ("a green `--check`
distinguishes verified from not-yet-claimed"). No problem disappeared and none appeared: the
strictly-new checks (`MAX_SCALE_CEILING`, `oracle`, unnamed geometry, measured-above-declared, the
ceilings doc read) are all satisfied by the tree's own rows and doc, which the tests assert
(`problems(&registry(), &honest())` carries no such finding, and the doc is clean under the new
header-named reader).

`codegen --check`: baseline and this tree both exit 0 with byte-identical output — four
`up to date` lines. RG-28's orphan scan therefore adds no output on a clean tree.

## Commands

| command | exit | last line |
|---|---|---|
| `scripts/orch/gr cargo fmt --all --check` | 0 | (no output) |
| `scripts/orch/gr cargo clippy -p graph-cli --all-targets -- -D warnings` | 0 | only the four pre-existing `edition2024` manifest warnings |
| `scripts/orch/gr cargo test -p graph-cli --bin graph-cli` | 0 | `368 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out` (merged tree; 351 before the merge, 367 with develop's new tests, +1 for the parse-error test) |
| `scripts/orch/gr cargo test --workspace --no-fail-fast` | 0 | 20 binaries, `1819 passed; 0 failed; 12 ignored` |
| `scripts/orch/gr cargo build -p graph-core --target wasm32-unknown-unknown` | 0 | ``Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.29s`` |
| `scripts/orch/gr cargo build --release -p graph-cli` (baseline `9178255`, then this tree) | 0 | baseline `Finished release profile … in 28.80s`; this tree `… in 9.81s`, rebuilt `… in 12.29s` after the two RG-53 decisions |
| `scripts/orch/gr ./target/release/graph-cli capabilities --check` | 1 | `capabilities --check: 73 rows, 36 problems` |
| `scripts/orch/gr ./target/release/graph-cli codegen --check` | 0 | `up to date  docs/contract/ingest-schema.json` |
| `scripts/scigraphs-conformance.sh` | 0 | `PASS` — `scigraphs-conformance: 32/32 rows reached a reference` |

`hashgate --seeds 8` and `GM_MUTATE_REFERENCE_DEGREE=9` were **not** re-run: fix-common requires them
only for a change that moves a registered layout, post, analysis or scale output, and
`git diff 9178255 HEAD --stat` touches no `graph-core` file — all 18 changed files are under
`crates/graph-cli/src/`. No motor byte can have moved. Both gates are also explicitly excluded from
this job (no timed gate), and re-recording evidence from a fix job is forbidden.

## Conformance

`scripts/scigraphs-conformance.sh` runs `graph-cli scigraphs-conformance`, which writes
`target/gates/scigraphs-conformance.json` as its normal course. That run happened **after** the
before/after comparison above was captured, so it cannot have influenced it. No other record was
written by this job.