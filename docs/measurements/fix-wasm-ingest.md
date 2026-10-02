# fix-wasm-ingest: the ingest byte ceiling (F-16) and the SDK's test script

Job brief: `prompts/jobs/fix-wasm-ingest.md` over `prompts/jobs/fix-common.md`. Sources:
`docs/decisions/wasm-ingest-limits.md` (decided) and the "Decisions needed" items 1 and 4 of
`docs/measurements/fix-wasm-abi.md`.

| id | severity | verdict | test name | file:line |
|---|---|---|---|---|
| F-16 | MAJOR | fixed: `read` refuses a buffer over `MAX_INGEST_BYTES` on its length before `from_utf8`, and `gm_build` publishes that as the appended code `IngestTooLarge` (19) instead of trapping | `a_document_one_byte_past_the_ceiling_is_refused_and_one_at_it_is_not`; `the_ceiling_is_the_largest_that_built_rounded_down_to_a_whole_mib`; `only_the_ceiling_gets_the_new_code`; `every_code_has_one_name_in_the_doc_and_in_the_sdk_in_wire_order` | `crates/graph-wasm/src/ingest.rs:62`, `:115`, `tests/ceiling.rs:27`, `errors.rs:86` |
| F-01 | BLOCKER | doc-only: `child_first` stays optional in version 1 — `docs/decisions/wasm-ingest-limits.md` "F-01", and the doc sentence it protects is pinned by `the_abi_doc_states_what_an_omitted_child_first_means` | `the_abi_doc_states_what_an_omitted_child_first_means`; `an_omitted_child_first_reads_parent_first_and_a_present_one_is_read` | `docs/contract/wasm-abi.md:445`; `ingest/tests/child_first.rs:35` |
| F-80 | MINOR | doc-only: a negative `strength` stays refused by the shortest-path centralities, not at ingest — `docs/decisions/wasm-ingest-limits.md` "F-80" ("Refusing it at ingest would change what `gm_build` accepts for every caller") | `a_negative_strength_refuses_the_shortest_path_centralities_instead_of_answering` (landed by `fix-wasm-abi`) | `docs/decisions/wasm-ingest-limits.md:35` |
| SDK-TEST | — | fixed (script already in `HEAD` `0e85a20`; this job restored the stale worktree copy and wired the script into the gate: `sdk-test` beside `sdk-typecheck` in `develop-full.rows`, its `sdk-test-control` row beside it, and the command in `CLAUDE.md`'s block) | `crates/graph-sdk-js/test/abi-version.test.mjs`, both tests | `package.json:54`; `scripts/orch/rows/develop-full.rows:58`; `CLAUDE.md:114` |

## The measurement (decision record steps 1-2)

Generator: `harness/ingest-ceiling.mjs`, one document per process so the driver can bound each
run with `timeout`. The records come from the studio's own generator
(`packages/graph-studio/src/source/synthetic.ts` — deterministic mulberry32, `MAX_NODES =
1_000_000`), which is the producer at the scale target the decision record names, so the density
here is the real one; `--verify` checks the streamed document against `syntheticIngest`'s own
length. Driver: `scripts/orch/node-slim.sh`, the artifact
`target/wasm32-unknown-unknown/release/graph_wasm.wasm` from
`scripts/orch/gr cargo build -p graph-wasm --release --target wasm32-unknown-unknown`. Path:
`gm_alloc` → copy → `gm_build` → `gm_free`. Documents are written to
`target/ingest-ceiling/` and are never committed.

Sweep at the studio's mid density (degree 4 ≈ 4 edges per node), doubling, then the two
degrees that bracket it at the scale target:

| nodes | edges | degree | bytes | outcome (before the ceiling) |
|---:|---:|---:|---:|---|
| 50,000 | 199,984 | 4 | 40,512,194 | built |
| 100,000 | 399,984 | 4 | 82,799,521 | built |
| 200,000 | 799,984 | 4 | 166,515,424 | built |
| 400,000 | 1,599,984 | 4 | 335,260,546 | built |
| 800,000 | 3,199,984 | 4 | 673,275,084 | built |
| 900,000 | 3,599,984 | 4 | 757,654,499 | built |
| **920,000** | **3,679,984** | **4** | **774,568,785** | **built — the largest that built** |
| 950,000 | 3,799,984 | 4 | 799,922,860 | **trapped** (`unreachable`) |
| 1,000,000 | 999,999 | 1 | 349,970,233 | built |
| 1,000,000 | 1,999,996 | 2 | 514,076,008 | built |
| 1,000,000 | 2,999,991 | 3 | 678,016,813 | built |
| 1,000,000 | 3,999,984 | 4 | 842,132,644 | **trapped** (`unreachable`) |
| 1,000,000 | 4,999,991 | 5 | 992,910,579 | **trapped** (`unreachable`) |

Every row ran under `timeout 900`; the largest wall time was 26.8 s and no run reached the
timeout, so no row is a timeout.

The trap is F-16's defect verbatim, and it is not in the parser:

```
RuntimeError: unreachable
    at graph_wasm.wasm.…alloc5alloc18handle_alloc_error
    at graph_wasm.wasm.…graph_core5arena…StringArena6intern
    at graph_wasm.wasm.…graph_core5index11index_model
    at graph_wasm.wasm.gm_build
```

The same documents, after the ceiling, are nameable refusals — no trap:

| nodes | degree | bytes | outcome (after the ceiling) |
|---:|---:|---:|---|
| 1,000,000 | 3 | 678,016,813 | **built** — the studio's scale target, 95,832,275 bytes under the ceiling |
| 920,000 | 4 | 774,568,785 | `refused code=19` — 719,697 bytes over the ceiling, by the whole-MiB rounding alone |
| 950,000 | 4 | 799,922,860 | `refused code=19` |
| 1,000,000 | 4 | 842,132,644 | `refused code=19` |

### The number (steps 3-4) and what it costs

`MAX_INGEST_BYTES = 773_849_088`: the largest document that built, 774,568,785 bytes, rounded
**down** to a whole MiB (738 MiB). The power-of-two step down the decision record asks for is
deferred to `fix-ingest-scale`, because at `2^29` this ceiling refused the studio's own 1M-node
degree-3 document (678,016,813 bytes), which builds — and a ceiling that refuses what works
replaces nothing, since the trap is the only failure it prevents.

What the new number costs, stated exactly: the rounding is 719,697 bytes, so the sweep's own
largest document (920,000 nodes at degree 4) is refused where it built. Every document the studio
builds at its declared scale target is under the ceiling — 1M nodes at degrees 1, 2 and 3, up to
678,016,813 bytes, all `built code=0` on the artifact above — and nothing between 774,568,785 and
the first trapping document (799,922,860 bytes) has been shown to build at all.

## RED and GREEN, at the boundary and on the real artifact

| run | result |
|---|---|
| RED (unit) | `error[E0599]: no method named 'code' found for enum 'ingest::IngestError'` — `TooLarge`, `MAX_INGEST_BYTES` and `code()` do not exist yet, at `ingest/tests/ceiling.rs:59` |
| RED (artifact, the defect) | `536870913` bytes — one past the then-ceiling — **`built code=0`**, exit 0. `536870912` bytes also `built code=0`. Both are the 400k-node degree-4 document (335,260,546 B) padded with leading JSON whitespace to an exact byte count |
| GREEN (unit) | `cargo test -p graph-wasm --lib ceiling`: 3 passed; 0 failed |
| GREEN (artifact, the boundary) | `536870912` bytes → `built code=0`; `536870913` bytes → `refused code=19`, exit 1 |
| GREEN (`cargo test -p graph-wasm --lib`) | 140 passed; 0 failed; 1 ignored (`every_code_has_one_name_in_the_doc_and_in_the_sdk_in_wire_order` among them) |

The negative control for the boundary is built into the test: the same buffer one byte shorter
is asserted **not** to be a `TooLarge` refusal, and `only_the_ceiling_gets_the_new_code` pins
every other ingest refusal to `IngestInvalid`.

Round 2, at the new number (the ceiling moved from `2^29` to the measurement rounded down to a
whole MiB, so the boundary had to be re-run on the artifact):

| run | result |
|---|---|
| boundary, `773849088` bytes (exactly the ceiling) | `400000 1599984 4 773849088 built code=0`, exit 0 |
| boundary, `773849089` bytes (one past) | `400000 1599984 4 773849089 refused code=19`, exit 1 |
| the studio's scale target, 1M nodes at degree 3 | `1000000 2999991 3 678016813 built code=0`, exit 0 |
| the first document measured to trap, 950k at degree 4 | `950000 3799984 4 799922860 refused code=19`, exit 1 |
| `cargo test -p graph-wasm --lib ceiling` | 3 passed; 0 failed (`the_ceiling_is_the_largest_that_built_rounded_down_to_a_whole_mib`, `a_document_one_byte_past_the_ceiling_is_refused_and_one_at_it_is_not`, `only_the_ceiling_gets_the_new_code`) |

## Commands

| command | exit | last lines |
|---|---|---|
| `scripts/orch/gr cargo fmt --all --check` | 0 | (no output) |
| `scripts/orch/gr cargo clippy --workspace --all-targets -- -D warnings` | 0 | `Finished dev profile` |
| `CARGO_BUILD_JOBS=6 RUST_TEST_THREADS=4 timeout 3000 scripts/orch/gr cargo test --workspace --no-fail-fast` | **0** (round 2) | 1763 passed, 0 failed, 12 ignored — the whole floor green in one run |
| `CARGO_BUILD_JOBS=6 RUST_TEST_THREADS=4 timeout 3000 scripts/orch/gr cargo test --workspace --no-fail-fast` | 101 (round 1) | 1749 passed, 14 failed, 12 ignored; every failure was in a `graph-cli` integration binary and every one of them was green re-run alone (see below) |
| `scripts/orch/gr cargo build -p graph-core --target wasm32-unknown-unknown` | 0 | `Finished dev profile` |
| `scripts/orch/gr cargo build -p graph-wasm --release --target wasm32-unknown-unknown` | 0 | `Finished release profile`, no warning from any code (round 2; round 1 printed `warning: unused imports: EdgeKind and NodeKind`) |
| `scripts/orch/gr cargo run -q -p graph-cli -- codegen --check` | 0 | `up to date docs/contract/ingest-schema.json` |
| `scripts/orch/gr cargo run -q -p graph-cli -- capabilities --check` | **1** | `71 rows, 36 problems`, all of them missing gate records (see below) |
| `scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8` | 0 | `4-way equal on 8/8 seeds` / `PASS` |
| `scripts/orch/gr -e GM_MUTATE_REFERENCE_DEGREE=9 cargo run -q -p graph-cli -- hashgate --seeds 8` | 1 (control, as required) | `4-way equal on 0/8 seeds` / `FAIL: 8 of 8 seeds diverge` |
| `scripts/scigraphs-conformance.sh` | 0 | `scigraphs-conformance: 32/32 rows reached a reference` / `PASS` |
| `scripts/orch/node-slim.sh npm run sdk:test` | 0 | `# pass 2` / `# fail 0` |
| `scripts/orch/node-slim.sh npm run sdk:smoke` | 0 | `# pass` |
| the two new rows through the real harness: `scripts/orch/gate.sh <logdir> <rows-with-just-them>` | 0 | `PASS sdk-test exit=0 expect=0` / `PASS sdk-test-control exit=1 expect=nonzero` |
| `scripts/orch/node-slim.sh bash -c '… sdk-test-control row verbatim …'` | 1 (control, as required) | `not ok 2 - a module reporting this SDK's ABI version loads` / `# fail 1` |

### `cargo test --workspace` was red in round 1, and why

Round 1's run exited 101 with 14 failures, all in `graph-cli`'s integration binaries
(`cli_force`, `cli_force_gate`, `cli_igraph`, `cli_ledger`, `cli_oracles`, `cli_p3`,
`snapshot`), every one of them an assertion that a spawned `graph-cli hashgate` / `oracle-diff`
sub-run exited `0` (or `1` for a control) and got `2` — graph-cli's "could not run". Those
tests spawn nested `cargo build`s of `target/wasm32-unknown-unknown/release/graph_wasm.wasm` and
then read that artifact, so with four test threads running the whole workspace at once they race
each other on the file they share. Re-run alone, all eight `graph-cli` binaries were green
(`ok. 8`, `ok. 3`, `ok. 4`, `ok. 5`, `ok. 4`, `ok. 4`, `ok. 3`, `ok. 3`, 34 tests, 0 failed).
Round 2, with the same command and the same parallelism, exited **0** with 1763 passed and 0
failed — the race, not the code. Nothing here touches ingest: the models these tests drive are
2..41 nodes.

### `capabilities --check` is not green here, and why

All 36 problems are evidence records, never a claim this change touches: 18 rows read
`gated, but hashgate ran 8 seeds, need 1000` and 18 read `gated, but no <oracle> record: run
the gate`. `crates/graph-cli/src/evidence.rs` reads them from `target/gates`, which only the
full timed gate run writes; the brief forbids this job from running `hashgate --seeds 1000`,
`ge-check.sh` or `gate.sh`. UNKNOWN is not a pass, so this row is reported unmet rather than
claimed. Before this job ran anything the same command said `no hashgate record: run the gate`
for all 36 — the count did not move, and it did not move in round 2 either.

## Deviations

`crates/graph-wasm/src/exports/build.rs` is not in this job's paths. `gm_build` is the only
producer of an ingest refusal, and the job requires the new code to be *appended and named*,
so the reader's refusal has to reach `errors::set` instead of the blanket `IngestInvalid`. The
change is 6 lines and moves no other behaviour: `let Ok(..) else { errors::set(Code::IngestInvalid) }`
became a `match` on the refusal with `errors::set(refusal.code())`. `IngestError::code()`
itself lives in `ingest.rs`, in this job's paths, and is unit-tested there.

## Fixed in round 2

- `crates/graph-wasm/src/ingest.rs:21` — the `EdgeKind`/`NodeKind` imports had no user in the
  wasm32 release build (`ingest/tests.rs` reaches them through `use super::*`, `record.rs`
  imports its own), so that build printed `warning: unused imports`. They are now a
  `#[cfg(test)]` import and `cargo build -p graph-wasm --release --target wasm32-unknown-unknown`
  prints no warning from any code — only the four workspace-wide
  `cargo-features = ["edition2024"]` manifest warnings, which come from the four
  `crates/*/Cargo.toml` files and are outside this job's paths.

## Findings not fixed here

- `ingest/tests/ceiling.rs` allocates `MAX_INGEST_BYTES + 1` = 738 MiB of whitespace to pin the
  boundary with the real constant. It is the cheapest honest way to test the boundary at the
  measured number, and it is the reason that test takes ~4 s.
- Every `crates/*/Cargo.toml` carries `cargo-features = ["edition2024"]`, stabilized in Rust
  1.85, so all four workspace manifests warn on every cargo invocation. Recommended fix: delete
  the line from the four manifests (four files, none in this job's paths).

## Decisions taken

- The generator is a `harness/` script, not a `graph-cli` subcommand: `seed_ingest::document`
  (the one writer `ingest::read` is proven against) is private to `graph-wasm` and its module is
  `#[cfg(any(test, target_arch = "wasm32"))]`, so graph-cli — which builds `graph-wasm` natively —
  cannot reach it without un-gating ingest for every native build. The studio's own TypeScript
  generator is the better source anyway: it is the actual producer at 1M nodes.
- `sdk:test` keeps `--experimental-strip-types`: the one test file today is `.mjs`, but it
  imports `../src/wasm.ts`, and `node --test crates/graph-sdk-js/test/` fails two ways on the
  pinned Node 22.23.3 — the directory argument is taken as a module (`Cannot find module
  '/w/crates/graph-sdk-js/test'`), and the `.ts` import needs the flag. The script passes the
  quoted glob `"crates/graph-sdk-js/test/**/*.test.mjs"` so a nested test file is still found.
- `sdk-test-control` copies the whole SDK package into `target/` rather than editing the test in
  place, so the row leaves no file behind and the copy's `../src/wasm.ts` import still resolves.
  It breaks the test's input — the hand-assembled module is made to report one version above the
  SDK's, which the loader refuses (C4's handshake) — and never the expected exit.

## Decisions needed

1. **"Rounded down to a whole MiB" and "no document that built is refused" cannot both hold.**
   The largest document that built is 774,568,785 bytes; the largest whole MiB at or below it is
   773,849,088, so the instruction's own arithmetic refuses that one document by 719,697 bytes.
   Implemented as instructed (`773_849_088`), and the consequence is stated in the constant's
   `Ponytail:`, in `docs/contract/wasm-abi.md` and in the decision record rather than papered
   over. Every document the studio builds at its 1M-node scale target (degrees 1, 2, 3 — up to
   678,016,813 bytes) is under it and `built code=0` on the artifact.
   **Recommended answer if the invariant is meant to include the sweep's own top row: round up to
   `774_897_664`** (739 MiB), which is still 25,153,196 bytes below the first document measured to
   trap (799,922,860) and accepts nothing unmeasured *and* untrapped in that gap. That is a
   one-token change to the constant plus the three documents that name it.
2. **The arena defect is `fix-ingest-scale`'s, and the decision record now says so.** The trap is
   in `graph_core::index_model`'s `StringArena::intern`, past ingest, and this job does not touch
   graph-core. Until that job lands, a document under this ceiling can still trap — which the
   `Ponytail:` line names rather than hides.
3. **`sdk:test` rows are in place; the floor command is in place.** `scripts/orch/rows/develop-full.rows`
   gained `sdk-test` beside `sdk-typecheck` and `sdk-test-control` (`nonzero`), and `CLAUDE.md`'s
   command block gained `scripts/orch/node-slim.sh npm run sdk:test` beside `sdk:typecheck`. Both
   rows were run verbatim: exit 0 and exit 1 with `not ok 2`. Nothing is outstanding here.
