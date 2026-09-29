# Phase 4 report — the WASM transport, the JS SDK, and the ledger row behind them

Shape: `prompt.md` §12. Every command in §3 was re-run for this report on the tree as it
stands (p4 with `develop` merged, plus the ledger/evidence wiring, plus this session's
registry-driven SDK surface, plus §6c's stage-resolution fix — that last session re-ran all
of §3 again, unchanged, and the artifact is byte-identical to the figure quoted here).
Exit codes are the real ones. **One gate row was not run here** — `hashgate --seeds 1000` —
and §7 names it as UNKNOWN rather than quoting the earlier two-stage run as if it covered
the current stage list.

**Envelope.** The reviewer's BLOCKER was accepted as "amend the envelope": the phase's
literal MODIFY list could not hold what the phase's own Ledger delta requires (§1).

## 0. What this session changed

`docs/reports/STATUS.md` §3 p4 asks for a registry-driven ABI "so new layouts need no ABI
change". The gate's two arms were reconciled to that last session (§6a); this session closed
the consumer's half, which was still a literal: the SDK published no way to ask a module
what layouts it has, so `harness/sdk-smoke.mjs` — the phase's own third-party test, which
is specified to run *every* gated layout and print node counts and bounds — named
`layout.grid` and covered one layout forever. `Motor#layouts` is the missing half, and with
it the smoke test runs whatever the module registered, per layout asserting the contract's
whole column table. That table is what makes the reserved Circle `r` / Box `w,h` / Polyline
and Curve columns checked rather than documented: until this session nothing had read them
back through the export layer, because the only registered layout emits Point nodes and
Line edges. §6b has the RED evidence and the temporary probe rows that made the difference
observable.

**The session after that one found the gap §6a and §6b both left open on the *wasm* side
(§6c).** Both made the arms registry-driven, and neither noticed that the arm's stage
dispatch table was a JS object literal: a stage named after an `Object.prototype` member
answered with that member instead of `undefined`, so the arm *hashed* a stage the module
does not have — `toString` printed a digest of the string `"[object Undefined]"` and exited
0. Registry-driven means every registered layout is reachable; it does not mean every
unregistered name is refused, and that second half was untested. §6c fixes it, pins it in
both directions, and collapses the arm's three separate reads of the module's registry into
one.

## 1. Authorization compliance

**Created, on the envelope:** `crates/graph-wasm/src/{alloc.rs,views.rs}`,
`crates/graph-sdk-js/{package.json,tsconfig.json,src/index.ts,src/wasm.ts,src/views.ts,src/types.ts,README.md}`,
`harness/sdk-smoke.mjs`, `docs/contract/wasm-abi.md`,
`docs/measurements/phase04-transport.md`. (`src/errors.ts` joins `src/`: a typed-error
subclass per refusal, 82 lines, split out of `index.ts` rather than compressing it.)

**Modified, on the envelope:** `crates/graph-wasm/src/lib.rs`,
`crates/graph-contract/src/lib.rs`, `harness/wasm-run.mjs`, `package.json`.

**Deviations.** Each outside the envelope, with its cause. Envelope growth is the
accepted amendment, not an accident: the phase's own Ledger delta ("register
`transport.wasm.columnar` and `sdk.js` as capabilities with their own gates") cannot be
done from the files the envelope names, and the reviewer's BLOCKER on that row was
accepted as "amend the envelope" (`docs/reports/STATUS.md` §3 p4).

| path | why |
|---|---|
| `crates/graph-cli/src/capabilities/{registry.rs,verdict.rs}` | The phase names `crates/graph-cli/src/capabilities.rs`, which in this tree is the `ledger`/`problems` driver only: the registry, the verdict rules and the transport rows live in the `capabilities/` module an earlier phase split out. `capabilities.rs` itself was not touched. |
| `crates/graph-cli/src/capabilities/tests/{mod.rs,registry.rs}` | The two transport rows' status and record names are asserted here; the row count and the "gated rows are refused" count both move with them. |
| `crates/graph-cli/src/capabilities/tests/transport.rs` (new) | `tests/mod.rs` passed 300 lines once the transport row's own evidence tests went in (the house limit); split out, mirroring `tests/registry.rs`. |
| `crates/graph-cli/src/hashgate.rs`, `hashgate/{stages.rs,transport.rs,compare.rs}` | C20's proof had to become a stage of the hash gate for the ledger row to be backed by a record (`docs/contract/wasm-abi.md` "Hash-gate wiring"). `stages.rs` holds the registry-driven stage list and the native arm's bytes, `transport.rs` the C20 tally, `compare.rs` follows `stages()`. Not on the MODIFY list, and the phase's Ledger delta is the cause. |
| `crates/graph-cli/src/hashgate/tests.rs` → `tests/{mod.rs,stages.rs,stages/{marked,arm}.rs}` | The house-limit fix-up: the transport stage's own tests pushed the single file past 300; this session's stage-list reconciliation (§6a) added `stages.rs` beside it, and its fixture layouts went one level deeper again rather than compressing. `stages/arm.rs` (§6c) is the same split one level deeper: the tests that shell out to `harness/wasm-run.mjs` over a real artifact are the only ones in the module that need a second program, and `stages.rs` was already at 281 lines. |
| `crates/graph-core/src/registry.rs` (read, and briefly temporary registry rows in two sessions) | §6a and §6b: both defects are only observable with more than one registered layout, so the proofs need extra rows — §6a's second row, §6b's two (`layout.probe.circle`, `layout.probe.box`, the grid's positions with an `r` column and a Polyline path, and with `w`/`h` and a quadratic Curve). The rows are removed and the file is byte-identical to what it was; `git status` shows it unmodified in the delivered tree. Read-only there. |
| `crates/graph-cli/tests/cli.rs` | The integration rows for the transport stage: the honest run, both controls, the arm's line count, the ledger's refusal count. Already on the phase's deviation list for the pre-Phase-4 house-limit split. |
| `crates/graph-wasm/src/{exports/,handle.rs,ingest.rs,seed_ingest.rs,errors.rs,memory_measure.rs,ingest/tests.rs,seed_ingest/tests.rs}` | House limits. The real-ABI export surface is `exports/{mod,state,build,columns}.rs` because a single-file `exports.rs` measured 312 lines; `memory_measure.rs` is a `#[cfg(test)]`-only measurement file, not part of the ABI. |
| `crates/graph-wasm/Cargo.toml`, `Cargo.lock` | `graph-wasm` gained a dependency on `graph-contract` (already a workspace member; no new external crate, and `cargo tree -p graph-wasm` still shows no wasm-bindgen). `Cargo.lock` changes by exactly one line. |
| `crates/graph-contract/src/canonical_json.rs` | One export added (`pub use parse::{Value, parse};`, was `Value` only) so `ingest.rs` can call the existing parser. No parsing logic changed. |
| `crates/graph-cli/src/snapshot_cmd{,/exercise.rs,/exercise/tests.rs,/tests.rs}`, `crates/graph-contract/src/{canonical_json/tests/syntax.rs,snapshot/tests.rs}`, `docs/contract/binary-layout.md` | The `develop` merge's own changes, not this phase's; listed so the diff is complete. |
| `docs/reports/phase-04.md` | This report. |

Nothing under `src/`, `tests/` (the TypeScript oracle) or `verify/` was touched, and
nothing in osionos. No new layout, no `wasm-bindgen`, no `wasm-pack` (§3).

### 1a. The `docs/reports/STATUS.md` §3 p4 reconciliation list, item by item

| STATUS §3 p4 item | state on this tree |
|---|---|
| registry-driven ABI (`gm_layout_count`/`gm_layout_id`) so a new layout needs no ABI change | **In place, and this session closed the SDK side of it (§6b).** The gate's stage list and the wasm arm's were reconciled last session; what was still hard-coded was the *consumer's* view: the SDK had no way to name the layouts a module supports, and `harness/sdk-smoke.mjs` therefore named `layout.grid` in a literal — the phase's own step 7 ("runs each gated layout, and prints node counts and bounds") was satisfiable only for one layout. `Motor#layouts` is the missing half, and the smoke test now runs whatever the module registered. |
| expose the notes columns (`note.code`, `note.index`) | **In place, and this session proved the exposure end to end for the first time (§6b).** `views.rs` names ids 7 and 8 and resolves them to `Column::Absent` for every graph (the snapshot type has no notes section until p3's merge); `types.ts` carries `NoteCode`/`NoteIndex`; `views.ts`'s `columnApplies` returns `false` for them whatever the geometry kind; `sdk-smoke.mjs` now asserts their absence for *every* registered layout, not for one hard-coded run. This is the honest state: the ids exist and are reserved, and nothing pretends they carry data. |
| the reserved Circle `r` / Box `w,h` / Polyline columns | **In place, and this session read them back through the real ABI (§6b).** `views.rs` resolves them by wire name for `NodeGeometry::{Circle,Box}` and by path for `EdgeGeometry::{Polyline,Curve}`, unit-tested per kind including the n=0 present-but-empty case; `views.ts` mirrors the same presence table. Until this session the *export* layer had only ever been read for the grid's Point/Line snapshot, so the reserved columns were documented and unit-tested at the view layer but never observed through `gm_column_ptr`/`gm_column_len`. Two temporary registry rows (Circle/Polyline, Box/Curve) made the smoke test's per-layout column table exercise all of them, and it goes red on a wrong row in that table — see §6b. |
| `EdgeRecord` built without `..` (exhaustive) | **Already in place, verified as a compile property.** `ingest.rs::edge` names all eight fields. Exhaustiveness cannot be a runtime test — a `..` literal compiles today and would silently swallow p3's `child_first` — so it was verified by adding a ninth field to the literal and confirming the build fails (`E0560: struct EdgeRecord has no field named …`), then reverting. The comment at that site says why. |
| back the `transport.*` ledger rows with hashgate/evidence wiring | **Last session's work** — §2, §3, §4, §5 — re-verified on this tree by this session (§3). |
| record the envelope growth as a deviation | §1's table (the reviewer's BLOCKER accepted as "amend the envelope"). |
| the 265 838 B wasm, 4% over the soft ceiling, accepted | §8, and `docs/measurements/phase04-transport.md`, re-measured on this tree. |

## 2. Ledger diff

Before this session: 11 rows, 2 of them new and both `implemented` —
`transport.wasm.columnar` and `sdk.js`, each naming a `hash_stage`/`oracle_record` that no
code wrote, so the ledger printed `not backed` for both and no amount of re-running could
ever change that. After: `transport.wasm.columnar` is `gated` on two verdicts read out of
`hashgate.json`; `sdk.js` is still `implemented`, and now says why in its own `oracle`
text rather than implying a record exists.

| id | status before | status after | hash_4way | oracle_diff |
|---|---|---|---|---|
| `transport.wasm.columnar` | `implemented`, both columns `not backed` | **`gated`** | `equal/N seeds (transport.wasm.columnar stage; negative control hashgate-control-grid-spacing red)` | `byte-equal/N seeds (N cases)` — the C20 tally |
| `sdk.js` | `implemented`, both columns `not backed` | `implemented` (unchanged); its `functions` list and its `oracle` text now name what its gate actually covers (every registered layout, per layout) — and it still says why in its own `oracle` text rather than implying a record exists |
 `not backed: …` | `not backed: no sdk-smoke record: run the gate` |

The other 9 rows are unchanged in substance. `capabilities --check` against a scratch
8-seed record reads `11 rows, 20 problems` — 10 gated rows × 2 verdicts, all refused for
the same honest reason (`hashgate ran 8 seeds, need 1000`), plus `sdk.js` unbacked and
not claimed. The 1000-seed run is the orchestrator's row (§7).

## 3. Gate table

Re-run on this tree, after §6a's reconciliation. `hashgate --seeds 8` was run against a
scratch gates directory (`GM_GATES_DIR=target/tmp-demo`) so the short run cannot stand in
for a real record in `target/gates`.

| command | exit | result |
|---|---:|---|
| `gr cargo fmt --all --check` | 0 | — |
| `gr cargo clippy --workspace --all-targets -- -D warnings` | 0 | — |
| `gr cargo test --workspace` | 0 | 264 tests, 0 failed (was 259; §6a adds 5, and the 2 wasm probes stay `ignored` as they always were) |
| `gr cargo build -p graph-wasm --release --target wasm32-unknown-unknown` | 0 | 265 838 B, sha256 `1349dc10…` — unchanged by §6a, which touches no `graph-wasm` source |
| `gr sh -c 'cargo tree -p graph-wasm \| grep -qi wasm-bindgen && exit 1 \|\| exit 0'` | 0 | no wasm-bindgen, no wasm-pack |
| `gr … hashgate --seeds 8` | 0 | 3 stages 4-way equal, C20 8/8, `PASS` |
| `gr … -e GM_MUTATE_REFERENCE_DEGREE=9 hashgate --seeds 8` | **1** | `topology 0/8`, `layout.grid 8/8`, `transport.wasm.columnar 8/8` |
| `gr … -e GM_MUTATE_GRID_SPACING=2 hashgate --seeds 8` | **1** | `topology 8/8`, `layout.grid 0/8`, `transport.wasm.columnar 0/8` |
| `gr … capabilities --check` (8-seed scratch record) | 1 | `11 rows, 20 problems`, every one named; both transport refusals read `hashgate ran 8 seeds, need 1000` |
| `node-slim.sh node --experimental-strip-types harness/wasm-run.mjs … --assert-zero-copy` | 0 | 6/6 checks; view re-derivation 162.2 ns/call on this run, 160.3 and 169.0 ns/call on re-runs of the identical row (wall-clock, scheduling noise — `docs/measurements/phase04-transport.md`) |
| `node-slim.sh node --experimental-strip-types harness/sdk-smoke.mjs …` | 0 | 20/20 checks on this tree, and one `# layout.grid: 2 nodes, Point nodes / Line edges, bounds x[-0.5, 0.5] y[0, 0]` line per registered layout (§6b) |
| `npx tsc -p crates/graph-sdk-js/tsconfig.json --noEmit` | 0 | zero diagnostics (report-only row, C23) |
| `ge-check.sh` | 0 | 27/27 oracle tests |
| §6a's own two-layout probe (`LAYOUTS` temporarily 2 rows) | 0 | 4 stages 4-way equal, `PASS`; and with the spacing control, `layout.grid.demo 2/2` — its own stage, its own bytes, the grid's control not reaching it |
| `gr cargo run -p graph-cli -- hashgate --seeds 1000` | **not run** | the orchestrator's row under the host-wide lock; UNKNOWN, see §7 |
| `gr cargo mutants` | **not run** | the orchestrator's, under the host-wide lock; UNKNOWN, see §7 |

## 4. 4-way hash table

| stage | what hashes it | honest run | control |
|---|---|---|---|
| `topology` | `gate_exports::gm_topology` (unchanged) | 4-way equal | `GM_MUTATE_REFERENCE_DEGREE=9` → 0/8 |
| `layout.grid` | `gate_exports::gm_layout_grid` (unchanged) | 4-way equal | `GM_MUTATE_GRID_SPACING=2` → 0/8 |
| `transport.wasm.columnar` | the real ABI: `gm_seed_ingest → gm_alloc → gm_build → gm_run → gm_snapshot_bytes` | 4-way equal, and its own C20 tally 8/8 seeds matching the shim | `GM_MUTATE_GRID_SPACING=2` → 0/8 (the stage restates the grid's bytes natively) |

Measured at 8 seeds, quoted in full in `docs/measurements/phase04-transport.md`. The
1000-seed row is §7's UNKNOWN.

## 5. Coverage table

| changed symbol | the test that exercises it |
|---|---|
| `hashgate::stages` (registry-driven stage list) | `hashgate::tests::stages::the_stages_are_the_topology_then_every_registered_layout_then_the_transport` |
| `hashgate::stages::stage_bytes_for` (the native arm's bytes, one producer per stage, over an explicit registry slice) | `…::stages::a_second_registered_layout_joins_the_gate_with_no_change_to_the_stage_list`, `…::stages::every_registered_layouts_native_bytes_are_its_own_run_over_the_same_topology`, `…::stages::every_layout_gets_its_own_bytes_and_a_perturbed_one_diverges_from_its_own_default`, `…::stages::a_registry_without_the_transports_layout_or_with_a_repeated_id_is_refused` |
| the wasm arm's stage list, resolved from `gm_layout_count`/`gm_layout_id` | `…::stages::arm::the_wasm_arm_can_hash_every_stage_the_gate_asks_for` (reads the harness's own `stages` mode and compares both ways: every stage the gate asks for is offered, and nothing extra) |
| a stage the wasm arm does not have being refused by name, never hashed as an `Object.prototype` member (§6c) | `…::stages::arm::a_stage_the_arm_cannot_hash_is_refused_however_it_is_named`, both directions: six unknown names refused with the harness's exit 2, three shim-backed stages still hashed |
| `hashgate::stages::stage_bytes` (the transport stage's native bytes) | `…::stages::the_native_stages_are_the_registered_pipelines_own_bytes`, `…::stages::the_native_arm_hashes_the_transport_stage_from_the_pipelines_own_snapshot` |
| `hashgate::transport::agree_with_shim` (the C20 tally) | `…::the_transport_tally_counts_the_seeds_where_the_real_abi_matches_the_shim` (agree, one-seed divergence, short arm, each stage renamed, zero seeds) |
| `hashgate::LAYOUT`/`TRANSPORT` | `…::the_transport_stage_runs_the_layout_the_wasm_arm_names` |
| `hashgate::compare` (`stages()` instead of a fixed `STAGES`) | `…::agreeing_arms_have_no_divergence`, `…::one_arm_differing_on_one_line_names_that_line`, `…::vacuous_comparisons_are_refused`, `…::a_stage_whose_seeds_all_hash_alike_is_refused_as_one_input`, `…::per_stage_counts_equal_seeds_per_stage_and_distinct_bad_seeds` (per-stage count now sized from `stages().len()`, so it is not pinned to three) |
| `the_degree_knob_moves_the_topology_and_the_grid_knob_the_two_grid_stages` (which knob reaches which stage) | `…::stages::the_degree_knob_moves_the_topology_and_the_grid_knob_the_two_grid_stages`, now naming its stages by id rather than by position |
| `an_arm_prints_every_seed_of_one_stage_before_the_next` | `…::stages::an_arm_prints_every_seed_of_one_stage_before_the_next`, whose expected prefixes are built from `stages()` |
| `hashgate::conclude` / `record` (the tally in the record) | `tests/cli.rs::hashgate_passes_on_an_honest_run`, `…::the_spacing_control_goes_red_on_the_grid_stage_and_the_transport_that_restates_it`, `…::the_degree_control_goes_red_on_the_topology_stage_only` |
| `capabilities::tests::registry::every_row_but_the_sdks_names_a_record_the_gates_really_write` | itself: each row's `(oracle_record, hash_stage)` against the record graph-cli writes |
| `verdict::oracle_record` (`wasm-transport`) / `verdict::transport` | `capabilities::tests::transport::{the_transport_row_reads_back_both_of_the_hash_gates_own_verdicts, a_transport_tally_that_is_short_or_about_another_stage_backs_nothing, a_transport_tally_from_another_tree_is_refused_before_it_is_read}` |
| the `transport.wasm.columnar` ledger row (`gated`, its two record names) | `capabilities::tests::registry::every_row_but_the_sdks_names_a_record_the_gates_really_write` |
| `hashgate_arm`'s per-stage line count | `tests/cli.rs::hashgate_arm_prints_one_line_per_stage_and_seed` |
| `capabilities --check`'s refusal count | `tests/cli.rs::capabilities_needs_a_flag_and_refuses_gated_rows_no_recorded_run_backs` |
| a misspelt knob, or two at once, is could-not-run | `tests/cli.rs::a_misspelt_knob_or_two_at_once_is_could_not_run_never_a_green_control` |
| `ingest::edge`'s exhaustive `EdgeRecord` literal (no `..`) | `graph-wasm`'s `ingest` unit tests build every edge; exhaustiveness is a *compile* property — verified by adding a field to the literal and watching the build fail (`E0560: struct EdgeRecord has no field named …`), not by a runtime test, because a struct literal with `..` would compile here and only break at the merge |
| the notes columns (`note.code`, `note.index`) and the reserved `r`/`w`/`h`/Polyline columns | `graph-wasm/src/views/tests.rs` (every kind combination, incl. the n=0 present-but-empty case), and — from this session — `harness/sdk-smoke.mjs`'s per-layout restatement of the contract's table, which read them back through the real ABI for Circle/Polyline and Box/Curve under the §6b probe |
| `Motor#layouts` (the registry, published) and every registered layout run through the published SDK | `harness/sdk-smoke.mjs`'s registry checks and per-layout loop; §6b's RED table |
| a degraded motor refusing `layouts()` rather than answering `[]` | `harness/sdk-smoke.mjs`'s `degraded_motor_layouts_fails_predictably_kill_switch` |

## 6. Ponytail markers added

None. The two registry rows carry the markers they already had (a `scale_ceiling`
estimate each, measured natively and projected onto wasm32's 4 GiB). Nothing in this
session's change is a heuristic: the stage list is the registry, the tally is a count, the
verdict rules are equality checks — and this session's addition, `Motor#layouts`, is the
registry itself read through two `u32` exports, with a presence table copied from the
contract rather than approximated anywhere. The loader's `initFailed` latch and the
view-detachment rule keep the markers the phase already wrote, in
`docs/contract/wasm-abi.md` and `crates/graph-sdk-js/src/wasm.ts`.

## 6a. The gate's stage list and its native arm, reconciled

**The defect this session found and fixed.** `hashgate/stages.rs` claimed its list was
registry-driven and the claim was half true. `stages()` read
`graph_core::registry::LAYOUTS`, but `stage_bytes` — the native arm's bytes — named three
stages literally, and `harness/wasm-run.mjs`'s `STAGE_BYTES` named three more. The two
sides therefore agreed only while exactly one layout was registered. p3's four layouts
would have made `compare::diverged` refuse every run with `native run 1 printed N lines,
need M`, and the wasm arm would have exited 2 on `unknown stage layout.<new>`, before any
hashing happened: a green gate on p3 alone, and a *refusing* gate on p3 merged into p4.

**Observed, not argued.** A temporary second registry row
(`layout.grid.demo` = the grid's run with `x` scaled) was added to
`graph_core::registry::LAYOUTS`, the gate was re-run, and the failure reproduced on both
arms — `hashgate: could not run: … wasm-run: unknown stage layout.grid.demo` from the
wasm arm, and `hashgate-arm --seeds 2` printing 6 lines where 8 were needed from the native
one. The row was then removed and the tree restored (the wasm artifact's sha256 is back to
`1349dc10…`, byte for byte the pre-probe build).

**Fixed, and pinned by tests that fail without the fix.** Both arms are now driven by the
registry they already had:

- `stages::stage_bytes_for(seed, setting, layouts)` takes the registry slice as an
  argument; `stage_bytes` is that function over `LAYOUTS`. Each registered layout's stage
  is hashed from `(layout.run)(&topology)` — its own run, over the same topology — with
  the grid special-cased to the perturbed `run_pipeline` run so `GM_MUTATE_GRID_SPACING`
  still reaches it. A registry with a repeated id, or without the layout the transport
  stage restates, is refused rather than hashed.
- `harness/wasm-run.mjs` resolves any stage it is asked for that is not one of the three
  shim-backed ones through `gm_layout_count`/`gm_layout_id`, so a registered layout is
  hashable by name; an unregistered name is still refused. A new `stages` mode prints the
  arm's own list.
- `hashgate::tests::stages::the_wasm_arm_can_hash_every_stage_the_gate_asks_for` runs that
  mode and compares the two lists in both directions.

**Negative controls for the new tests**, both observed red first and then green again:
reverting `stage_bytes_for` to the three literal stages fails
`a_second_registered_layout_joins_the_gate_with_no_change_to_the_stage_list` and
`every_registered_layouts_native_bytes_are_its_own_run_over_the_same_topology`; making it
call each layout's `run` with the *grid's* instead fails
`every_layout_gets_its_own_bytes_and_a_perturbed_one_diverges_from_its_own_default`. That
last one is why `marked_layouts` exists: two copies of the grid under two ids produce
identical bytes, so a test using them would pass an implementation that hashed the grid
for every layout. The marked layouts write a distinct constant into `x[0]`, which is what
makes the substitution observable.

**The honest limit.** This is verified on a one-layout registry plus an injected second
row, not on p3's real layouts — those do not exist on this branch. What the temporary row
proves is the property the merge depends on: registering a layout is now sufficient to make
the gate hash it, on both arms, with no edit to either. `docs/reports/STATUS.md` §3 p5/p6
still owe each new layout its own registry row, its own ledger metadata, and a negative
control that goes red on *that* stage — the grid's spacing control does not reach a
layout whose own run ignores `GridParams`.

## 6b. The SDK's own view of the registry, and the reserved columns read back

**The defect this session found.** §6a made the *gate* registry-driven and left the
*consumer* hard-coded. The SDK could run a layout by name but could not say what layouts a
module has: `gm_layout_count`/`gm_layout_id` were read once, into a private map, and
nothing published it. So the phase's own third-party test — step 7 says it "runs each gated
layout, and prints node counts and bounds" — could only name one, and did:
`motor.layout(handle, "layout.grid")`, in a literal. The other half of the same claim was
equally untested: the contract's reserved Circle `r` / Box `w,h` / Polyline `offsets`,`pts`
/ Curve `degree` columns had never been read back through `gm_column_ptr`/`gm_column_len`,
because the only registered layout emits Point nodes and Line edges, so every reserved
column resolved to `Absent` on every run and the export layer's mapping for them was
exercised by nothing.

**Observed, not argued.** Two temporary registry rows were added to
`graph_core::registry::LAYOUTS` — `layout.probe.circle` (the grid's positions with an `r`
column and a Polyline edge path) and `layout.probe.box` (`w`/`h` and a quadratic Curve) —
and the module was rebuilt. Against that three-row registry:

- the **pre-fix** `harness/sdk-smoke.mjs` (the file as committed) printed 16 `ok` lines and
  exited **0**, having run one layout of three and printed no bounds at all;
- the **post-fix** script printed 28 checks, exited 0, and covered all three, with one
  `# <layout>: N nodes, <kind> nodes / <kind> edges, bounds x[…] y[…]` line each, the
  Circle/Polyline and Box/Curve runs included, and the contract's column table asserted
  kind by kind for every one of them. (28 = the one-layout run's 20 checks plus the four
  per-layout ones each of the two probe layouts adds.)

The rows were then removed, the file restored byte for byte (`git status` clean,
`cargo build` back to `graph_wasm.wasm` at 265 838 B, sha256 `1349dc10…`), and the
one-row tree re-verified — §3.

**Fixed, and pinned.** `Motor#layouts()` (`crates/graph-sdk-js/src/index.ts`) publishes the
registry, in registry order, from the same cached map `Motor#layout` resolves names
through — one derivation, read once per motor. The smoke test's per-layout loop and its
restatement of the contract's column table are the tests; the table is restated rather than
imported from `views.ts` on purpose, since a consumer checking the SDK against the SDK's own
`columnApplies` would agree with any mistake the SDK makes.

**RED, observed before each fix, not argued after it.**

| what | observed |
|---|---|
| the registry-driven loop, before `Motor#layouts` existed | 4 `not ok` lines and exit 1: `the SDK publishes the module's layout registry (C1)`, `the registry names layout.grid and repeats no id`, `every registered layout ran through the published SDK`, `a layout ran, so the transport could be exercised at all` |
| `layouts()` answering `[]` on a degraded motor instead of refusing | `not ok - degraded_motor_layouts_fails_predictably_kill_switch`, exit 1 — reverted to the refusal |
| one row of the restated column table claiming `NodeW` applies to `Circle` (observed with the two probe rows registered) | `not ok - layout.probe.circle: its columns match the contract's presence table` (`node column 3 is absent, but applies to Circle`) **and** `not ok - layout.probe.box: …` (`node column 3 is present but does not apply to Box`), exit 1 — both directions, and only on the layouts that make the row reachable |
| the same wrong row with only the grid registered | **passes**, which is the point: the reserved columns' absence is the only branch a one-layout tree has, so the table's non-trivial rows cannot be checked until a layout that needs them exists |

**What this does not settle.** The probe rows are proof of a property, not of p3's layouts:
what p3 brings is its own registry rows, its own `child_first` field (`ingest.rs`'s
exhaustive literal will name it or fail to build), its notes section (which fills ids 7/8,
the only change needed there), and its own negative control per stage — §7's last bullet,
unchanged. `sdk.js` is still `implemented` rather than `gated`: the smoke test is a script
over one fixture, not a recorded seed sweep, so it still writes no record a `gated` claim
could stand on (§2).

## 6c. The wasm arm refuses a stage it does not have, in every spelling

**The defect this session found.** §6a made the wasm arm registry-driven, and the stage
table it dispatches through is a JS object literal — so it inherits from
`Object.prototype`. `STAGE_BYTES["toString"]` answered with `Object.prototype.toString`, not
`undefined`, so the `?? fallback` that is supposed to send an unknown name down the
"is it a registered layout?" path never ran. Observed, on the shipped harness and the real
artifact, before the fix:

| stage asked for | before | after |
|---|---|---|
| `toString` | `toString 0 f388bc7c…` printed, **exit 0** — a green hash of the string `"[object Undefined]"` | `unknown stage toString: not a registered layout`, exit 2 |
| `constructor` | uncaught `TypeError: bytesOf is not a function`, node stack trace, exit 1 | refused by name, exit 2 |
| `__proto__` | `Object.prototype` is not callable — same trap, exit 1 | refused by name, exit 2 |
| `layout.nope` | refused by name, exit 2 | unchanged — the case the guard was written for and the only one that worked |

This is the gate's worst available failure, and it is silent in the dangerous direction:
`f388bc7c…` is a real 64-hex digest on a line shaped exactly like every other line the arm
prints, and `compare::diverged` would compare it happily against three arms that agree with
each other on nothing. The one thing the stage list must never do is hand back a verdict for
a stage nobody ran. It cannot happen through the gate's own `stages()` today — a registry id
must start with `layout.` — but the arm is a *published* file that takes a stage name from
its command line, and the four prototype names above are four ways in.

**Fixed, and pinned in both directions.** The table is built on a null prototype, so a
lookup cannot fall through to `Object.prototype` at all — the property is structural rather
than a check someone can forget to write. `hashgate::tests::stages::arm::a_stage_the_arm_cannot_hash_is_refused_however_it_is_named`
refuses `toString`, `constructor`, `__proto__`, `valueOf`, `hasOwnProperty` and
`layout.not.registered`, each demanding the refusal name the stage and the harness's exit 2,
and then hashes the three shim-backed stages so the refusals cannot be a blanket "refuse
everything" that passes. Observed red first: the assertion received
`["toString 0 f388bc7c…"]` — the bug's own output — as a success.

**Refactored while green, not as a separate change.** The arm resolved a layout name three
ways, and one of them (`layoutIndex`, used by `abiSnapshotBytes`) re-scanned
`0..gm_layout_count()` on every seed rather than reading the cached registry the other two
already use. That contradicts the ABI's own promise for `gm_layout_id` ("a caller finds it
by scanning `0..gm_layout_count()` once at load") and is the same one-derivation
discipline §6a's `stages::stage_bytes_for` exists to hold. `layoutIndex` is now a two-line
lookup in the one `layoutIndices()` map; at the gate's 1000 seeds that is 2000 fewer
registry scans and, more to the point, one place where the registry is read.

**What this does not change.** Nothing about the shipped bytes: the fix touches
`harness/wasm-run.mjs`, which is not compiled into `graph_wasm.wasm`, so the artifact stays
265 838 B at sha256 `1349dc10…` (§3). The stage list, the C20 tally and both ledger verdicts
are untouched, and the negative controls are unaffected — `GM_MUTATE_GRID_SPACING` reaches
the transport stage through the same `abiSnapshotBytes` call as before.

## 7. What could not be verified

- **`hashgate --seeds 1000` on the current stage list.** The transport stage is measured
  at 8 seeds and inside `cargo test`'s 4-seed integration run; the 1000-seed row was not
  run here, so `transport.wasm.columnar`'s `gated` claim is proven against an 8/4-seed
  record, not a 1000-seed one. UNKNOWN, not assumed. `docs/measurements/phase04-transport.md`
  keeps the earlier two-stage 1000-seed log quoted verbatim and labelled as the earlier
  run, rather than re-labelling it with the new stage list.
- **`cargo mutants` over this diff.** The orchestrator's, under the host-wide lock, per
  `AGENT_BRIEF.md` ("Never run … cargo mutants"). Not run, so the mutation result for this
  session's change is UNKNOWN.
- **The ledger at 1000 seeds, end to end.** §3's `capabilities --check` row is deliberately
  *red* (20 problems at 8 seeds) — that is the honest reading, and it means the
  `transport.wasm.columnar` row's `gated` claim has not been observed green against a
  1000-seed record. The verdict logic itself is pinned by unit tests over synthetic
  1000-seed evidence (`capabilities::tests::transport::*`), so what is unverified is the
  *real* 1000-seed record, not the rules that read it.
- **§6a on p3's actual layouts.** The stage-list fix is verified on a one-layout registry
  plus one injected second row, not on the four layouts p3 adds. What that probe proves is
  the property the merge needs — registering a layout is now sufficient for both arms to
  hash it — and it cannot prove the parts that are p3's to supply: each new layout's own
  registry metadata, and a negative control that goes red on *that* stage. The grid's
  spacing control does not reach a layout whose run ignores `GridParams` (observed:
  `layout.grid.demo 2/2` under `GM_MUTATE_GRID_SPACING=2`). Per the brief's "no row goes
  to `gated` without a control that must fail", a p3 layout that lacks its own red control
  cannot be `gated`, and `verdict::red_control` will refuse the row — which is the correct
  behaviour, not a gap this phase should paper over.
- **§6b on p3's actual layouts, and the two layout kinds no layout on this branch emits.**
  The smoke test now runs every registered layout and asserts the contract's column table
  for each, verified against two temporary Circle/Polyline and Box/Curve rows. That proves
  the checks bite and that a new registry row is covered with no edit to the harness or the
  SDK; it does not prove p3's layouts are *correct* — their own gate rows, their own
  negative controls and (for the notes section) the filling of ids 7/8 are p3's to supply.
  What p3's merge will change here is exactly three things and nothing else: the number of
  layouts the smoke test runs, the reserved-note-column assertions, which stay "absent"
  until the notes section exists, and `ingest.rs`'s exhaustive `EdgeRecord` literal, which
  must name `child_first` or fail to build.
- **Browser execution.** Unchanged from the phase: every JS check ran under
  `node:22-slim`. `WebAssembly.instantiateStreaming`'s browser `fetch` path is written but
  unexercised. UNKNOWN.
- **wasm32 peak-memory measurement.** `TRANSPORT_CEILING` is native, projected (the row's
  own Ponytail says so).

## 8. Stop-and-ask items

- *Zero-copy cannot be demonstrated → stop.* Demonstrated: a finite sentinel written
  through a `Float32Array` view over `memory.buffer` appears in the encoded JSON face
  (`--assert-zero-copy`, exit 0). A copying implementation could not pass it.
- *The SDK smoke test needs to import from `crates/` or touch the ABI → stop.* It imports
  only `crates/graph-sdk-js/src/index.ts`; it does not know the ABI.
- *`wasm-bindgen` appears in the tree → stop.* Absent (§3).
- *The release `.wasm` is far larger than ~250 KB → stop and report.* **265 838 B,
  ~259.6 KiB — 4% over the ~250 KB soft ceiling, accepted and documented** (not "far
  larger"; the phase's own instruction is to say so rather than let it pass unremarked).
  Re-measured on this tree: same bytes, same sha256. What carries it is one module holding
  both the retained hash-gate shim (so Phase 2/3's green gate keeps hashing what it always
  hashed) and the full new ABI, with the shared JSON parser and the doubled entry points
  on top — analysed in `docs/measurements/phase04-transport.md`.
