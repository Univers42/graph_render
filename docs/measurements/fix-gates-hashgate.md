# fix-gates-hashgate — review repairs to the hash gate and the CLI front door

Source: `docs/reviews/review-gates.md` (`RG-NN`). Repair of the ids this job names:
RG-01, RG-05, RG-06, RG-07, RG-26, RG-27 (BLOCKER/MAJOR) and RG-35…RG-42, RG-48, RG-51,
RG-58 (MINOR).

Every row ships a negative control that fails on the pre-repair code. The RED runs are pasted
in the job transcript; the tests below are the ones that carry them.

| id | severity | verdict | test name | file:line |
|---|---|---|---|---|
| RG-01 | BLOCKER | fixed | `a_zero_node_count_is_refused_like_every_per_stage_count` | `hashgate/knob/setting.rs:242` (`apply`), parser at `knob/value.rs:18` (`nodes`) |
| RG-05 | BLOCKER | fixed | `seed_count_accepts_one_and_refuses_zero`, `no_gate_subcommand_takes_zero_seeds` | `command.rs:15` (`MIN_SEEDS`), used at `command.rs:20`; `hashgate.rs` (`arm_lines`) |
| RG-06 | MAJOR | fixed | `a_c20_shortfall_fails_the_record_and_the_exit_code_together` | `hashgate/report.rs` (`passed`, `exit`) |
| RG-07 | MAJOR | fixed | `a_tally_that_does_not_cover_every_stage_is_refused`, `an_arm_shorter_than_the_others_is_refused_rather_than_indexed_past_the_end` | `hashgate/report.rs` (`body`, `checked_arm_report`, `one_length`) |
| RG-26 | MAJOR | fixed | `a_misspelled_variable_is_refused_rather_than_ignored`, `a_lower_cased_control_variable_is_refused_too`, `a_misspelled_control_variable_is_refused_by_name` | `hashgate/knob/env.rs` (`refuse_an_unknown_knob`), `setting.rs` (`setting_named`) |
| RG-27 | MAJOR | fixed | `a_stage_named_but_not_handled_is_refused`, `every_threaded_stage_has_an_arm` | `hashgate/tiered.rs` (`geometry`) |
| RG-35 | MINOR | fixed | `a_node_count_past_the_u32_index_space_is_refused_by_name`, `a_per_stage_control_that_would_wrap_the_sum_is_refused_by_name` | `hashgate/stages/checks.rs` (`node_count`) |
| RG-36 | MINOR | fixed | `the_c20_tally_names_its_arm_and_refuses_a_list_without_it` (`tests/report.rs:185`) | `hashgate/compare.rs:18` (`C20_ARM`) and `:21` (`arm`), used at `hashgate/report.rs:212` |
| RG-37 | MINOR | doc-only | — | `hashgate/stages.rs` (`TRANSPORT` doc, `stages()` doc) |
| RG-38 | MINOR | fixed | `every_stage_id_appears_once_in_the_gate_list`, `a_layout_id_that_collides_with_a_graph_wasm_stage_is_refused` | `hashgate/stages/checks.rs` (`check`) |
| RG-39 | MINOR | **fixed (print) — negative control added** / deferred (`tier.rs`) | `the_threaded_parenthetical_only_appears_where_an_arm_was_hashed_twice`, `a_tier_outside_the_named_worker_counts_never_names_an_arm` (see `hashgate/tests/report.rs`) | `hashgate/report.rs:183` (`ways`); `hashgate/tier.rs:36` |
| RG-40 | MINOR | fixed | `a_duplicate_stage_seed_line_is_refused` | `hashgate/transport.rs` (`digests`) |
| RG-41 | MINOR | fixed (`stage_of`, `knobs::apply`) / **false** (record prefix) | `a_per_stage_control_naming_an_unknown_stage_is_refused` | `hashgate/knob/arms.rs:130`, `hashgate/knobs.rs:280` |
| RG-42 | MINOR | fixed | `no_op_controls_are_refused`, `an_option_field_carrying_its_default_is_still_a_no_op`, `a_zero_gravity_is_refused_as_a_no_op_control_and_records_nothing` | `hashgate/knob/setting.rs` (`refuse_a_no_op`, `Setting::bites`) |
| RG-48 | MINOR | fixed | `the_floorless_controls_bite_at_one_seed_which_is_what_their_floorlessness_means`, `a_permuted_name_is_inert_and_a_permuted_line_is_caught`, `only_stages_moved`'s length/presence assertions, `SEEDS` in `tests/pipeline.rs` | `hashgate/tests/knob/controls.rs`, `tests/compare.rs`, `tests/pipeline.rs` |
| RG-51 | MINOR | fixed (`$CARGO`/`node`) / deferred (`--fixtures`) | `a_program_on_the_path_is_resolved_to_an_absolute_path`, `a_missing_program_is_refused_by_name`, `a_non_executable_file_is_not_on_the_path`, `a_relative_target_dir_is_resolved_against_the_workspace_root` | `runner/resolve.rs`, used by `runner.rs` |
| RG-58 | MINOR | fixed | `the_emit_fixtures_seed_count_must_be_stated`, `stress_names_an_oracle_that_is_wired`, `an_ingest_check_must_name_the_member_it_parses`, `seed_count_accepts_one_and_refuses_zero` | `command.rs` |

## RG-41's record-name half: `false`

`records.rs:64` gives `Knob::ForceSessionGravity` the name
`forcegate-control-force-session-gravity`, outside the `hashgate-control-` prefix the other 44
records carry. The only consumer resolves records **by name**, not by prefix
(`capabilities/verdict.rs:35` walks `Knob::ALL` and reads `Knob::record()`), so the odd
prefix has no reader to mislead. Renaming it would change a ledger key another crate owns for
no gain. Recorded `false` with that evidence; the five shipped names are unchanged.

## RG-39's `tier.rs` half: deferred

`Tier::arm_name` maps `Tier::Threads(_)` outside `{1,2,3,4,7}` to one shared name, so two
worker counts outside that set would collide. Making the mapping total means returning
`String`, which changes `Arm`'s element type and ripples into `compare.rs:10`, `report.rs` and
`tests/compare.rs`. The fix is real and was left out rather than half-applied: `WORKER_COUNTS`
is the only source of `Tier::Threads`, so the collision is unreachable today.

## RG-51's `--fixtures` half: deferred

`oracle-diff` and `oracle-layouts` still default their fixture directory. Making `--fixtures`
required was implemented and then **reverted**: it fails the shipped row
`scripts/orch/rows/develop-full.rows:44` (`oracle-layouts|0|… -- oracle-layouts`, no
`--fixtures`), and the rows file is outside this job's paths. A test
(`the_two_oracle_subcommands_still_accept_the_default_fixture_directory`) pins the flag as
optional so the deferral cannot be forgotten.

## Files touched outside the allowed list

**Forced by RG-42's fix** — both were asserting the defect itself:

- `crates/graph-cli/src/forcecheck/tests.rs` — asserted `GM_MUTATE_FORCE_SESSION_GRAVITY=0`
  parses and yields the honest bytes. That is the defect; the test now asserts the refusal.
- `crates/graph-cli/tests/cli_force_gate.rs` — asserted `=0` exits 0 and writes a passing
  `forcegate-control-force-session-gravity` record. Now asserts exit 2 and **no record**.

**Ripples of RG-51's `Result`-returning resolver:**

- `crates/graph-cli/src/determinism_probe.rs:30` — `node_harness` now returns
  `Result<Command, String>`, so the one call site outside `hashgate/` gained a `?`. One line;
  no behaviour changed for a probe that already required node on `PATH`.
- `crates/graph-cli/src/runner/resolve.rs` — a new *source* file, not a new test file: the
  allowed list named `runner.rs`, and splitting the resolver out is what keeps both files
  under the 300-line cap. Its tests are `runner/resolve/tests.rs`.

No other path outside `crates/graph-cli/` was modified; `graph-core`, `graph-wasm`,
`graph-contract` and every `.rows` file are untouched.

## One pre-existing flake fixed in passing

`hashgate/tests/stages/arm.rs` had two `#[test]`s each calling `build_wasm`, so two cargo
builds raced on one shared `target/` and the loser reported "cargo built but wrote no
`graph_wasm.wasm"`. `artifact()` is now a `OnceLock` (a failed build is not cached).

## Commands and last lines

```
scripts/orch/gr cargo fmt --all --check                                  -> 0
scripts/orch/gr cargo clippy --workspace --all-targets -- -D warnings    -> 0
scripts/orch/gr cargo test --workspace --no-fail-fast                    -> see transcript
scripts/orch/gr cargo build -p graph-core --target wasm32-unknown-unknown -> 0
scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8          -> 0  (PASS, 4-way equal on 8/8 seeds)
scripts/orch/gr -e GM_MUTATE_REFERENCE_DEGREE=9 cargo run -q -p graph-cli -- hashgate --seeds 8 -> 1
scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 0          -> 2
scripts/orch/gr -e GM_MUTATE_NODE_COUNT=0 cargo run -q -p graph-cli -- hashgate --seeds 8 -> 2
scripts/orch/gr cargo run -q -p graph-cli -- hashgate-arm --seeds 0      -> 2
```

`hashgate --seeds 0`:
```
error: invalid value '0' for '--seeds <SEEDS>': 0 is not in 1..=100000
```

`GM_MUTATE_NODE_COUNT=0 hashgate --seeds 8`:
```
hashgate: could not run: GM_MUTATE_NODE_COUNT="0": a control that adds no node perturbs
nothing; accepted range 1..=4294967295
```

## This job moves no output

Every changed file is under `crates/graph-cli/`; `graph-core`, `graph-wasm` and
`graph-contract` are untouched, so the wasm32 arm's per-stage digests are the ones it already
produced. `hashgate --seeds 8` exits 0 with native ×2 and wasm32 ×2 agreeing on every stage and
seed, which is the equality claim the gate exists to make.
