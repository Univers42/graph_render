# fix-wasm-ingest: the ingest byte ceiling (F-16) and the SDK's test script

Job brief: `prompts/jobs/fix-wasm-ingest.md` over `prompts/jobs/fix-common.md`. Sources:
`docs/decisions/wasm-ingest-limits.md` (decided) and the "Decisions needed" items 1 and 4 of
`docs/measurements/fix-wasm-abi.md`.

| id | severity | verdict | test name | file:line |
|---|---|---|---|---|
| F-16 | MAJOR | fixed: `read` refuses a buffer over `MAX_INGEST_BYTES` on its length before `from_utf8`, and `gm_build` publishes that as the appended code `IngestTooLarge` (19) instead of trapping | `a_document_one_byte_past_the_ceiling_is_refused_and_one_at_it_is_not`; `the_ceiling_is_the_measured_power_of_two`; `only_the_ceiling_gets_the_new_code`; `every_code_has_one_name_in_the_doc_and_in_the_sdk_in_wire_order` | `crates/graph-wasm/src/ingest.rs:53`, `:106`, `tests/ceiling.rs:26`, `errors.rs:86` |
| F-01 | BLOCKER | doc-only: `child_first` stays optional in version 1 — `docs/decisions/wasm-ingest-limits.md` "F-01", and the doc sentence it protects is pinned by `the_abi_doc_states_what_an_omitted_child_first_means` | `the_abi_doc_states_what_an_omitted_child_first_means`; `an_omitted_child_first_reads_parent_first_and_a_present_one_is_read` | `docs/contract/wasm-abi.md:445`; `ingest/tests/child_first.rs:35` |
| F-80 | MINOR | doc-only: a negative `strength` stays refused by the shortest-path centralities, not at ingest — `docs/decisions/wasm-ingest-limits.md` "F-80" ("Refusing it at ingest would change what `gm_build` accepts for every caller") | `a_negative_strength_refuses_the_shortest_path_centralities_instead_of_answering` (landed by `fix-wasm-abi`) | `docs/decisions/wasm-ingest-limits.md:35` |
| SDK-TEST | — | fixed (already in `HEAD` `0e85a20`; the worktree copy of `package.json` was stale and this job restored it — no diff remains) | `crates/graph-sdk-js/test/abi-version.test.mjs`, both tests | `package.json:54` |

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
| 1,000,000 | 1,000,000-1 | 1 | 349,970,233 | built |
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
| 950,000 | 4 | 799,922,860 | `refused code=19` |
| 1,000,000 | 4 | 842,132,644 | `refused code=19` |

### The number (step 3) and what it costs

`MAX_INGEST_BYTES = 536_870_912` (`2^29`): the largest power of two at or below the largest
document that built, 774,568,785 bytes. Step 4 fired and is reported below.

The step down to a power of two is the rule's margin and it refuses documents that **do** build
today: every document in `(536,870,912, 774,568,785]` bytes is refused although it built an
instant before. That set includes the studio's own 1M-node degree-3 document (678,016,813 B) and
its 500k-node degree-4 document (551,224,029 B). A ceiling placed at 774,568,785 would refuse
nothing that works, and the rule does not allow it.

## RED and GREEN, at the boundary and on the real artifact

| run | result |
|---|---|
| RED (unit) | `error[E0599]: no method named 'code' found for enum 'ingest::IngestError'` — `TooLarge`, `MAX_INGEST_BYTES` and `code()` do not exist yet, at `ingest/tests/ceiling.rs:59` |
| RED (artifact, the defect) | `536870913` bytes — one past the ceiling — **`built code=0`**, exit 0. `536870912` bytes also `built code=0`. Both are the 400k-node degree-4 document (335,260,546 B) padded with leading JSON whitespace to an exact byte count |
| GREEN (unit) | `cargo test -p graph-wasm --lib ceiling`: 3 passed; 0 failed |
| GREEN (artifact, the boundary) | `536870912` bytes → `built code=0`; `536870913` bytes → `refused code=19`, exit 1 |
| GREEN (`cargo test -p graph-wasm --lib`) | 140 passed; 0 failed; 1 ignored (`every_code_has_one_name_in_the_doc_and_in_the_sdk_in_wire_order` among them) |

The negative control for the boundary is built into the test: the same buffer one byte shorter
is asserted **not** to be a `TooLarge` refusal, and `only_the_ceiling_gets_the_new_code` pins
every other ingest refusal to `IngestInvalid`.

## Commands

| command | exit | last lines |
|---|---|---|
| `scripts/orch/gr cargo fmt --all --check` | 0 | (no output) |
| `scripts/orch/gr cargo clippy --workspace --all-targets -- -D warnings` | 0 | `Finished dev profile` |
| `CARGO_BUILD_JOBS=6 RUST_TEST_THREADS=4 timeout 3000 scripts/orch/gr cargo test --workspace --no-fail-fast` | see below | see below |
| `scripts/orch/gr cargo build -p graph-wasm --release --target wasm32-unknown-unknown` | 0 | `Finished release profile` |
| `scripts/orch/gr cargo run -q -p graph-cli -- codegen --check` | 0 | `up to date docs/contract/ingest-schema.json` |
| `scripts/orch/gr cargo run -q -p graph-cli -- capabilities --check` | **1** | `71 rows, 36 problems`, all of them missing gate records (see below) |
| `scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8` | 0 | `4-way equal on 8/8 seeds` / `PASS` |
| `scripts/orch/gr -e GM_MUTATE_REFERENCE_DEGREE=9 cargo run -q -p graph-cli -- hashgate --seeds 8` | 1 (control, as required) | `4-way equal on 0/8 seeds` / `FAIL: 8 of 8 seeds diverge` |
| `scripts/orch/node-slim.sh npm run sdk:test` | 0 | `# pass 2` / `# fail 0` |
| `scripts/orch/node-slim.sh npm run sdk:smoke` | 0 | `# pass` |

### `capabilities --check` is not green here, and why

All 36 problems are evidence records, never a claim this change touches: 18 rows read
`gated, but hashgate ran 8 seeds, need 1000` and 18 read `gated, but no <oracle> record: run
the gate`. `crates/graph-cli/src/evidence.rs` reads them from `target/gates`, which only the
full timed gate run writes; the brief forbids this job from running `hashgate --seeds 1000`,
`ge-check.sh` or `gate.sh`. UNKNOWN is not a pass, so this row is reported unmet rather than
claimed. Before this job ran anything the same command said `no hashgate record: run the gate`
for all 36 — the count did not move.

## Deviations

`crates/graph-wasm/src/exports/build.rs` is not in this job's paths. `gm_build` is the only
producer of an ingest refusal, and the job requires the new code to be *appended and named*,
so the reader's refusal has to reach `errors::set` instead of the blanket `IngestInvalid`. The
change is 6 lines and moves no other behaviour: `let Ok(..) else { errors::set(Code::IngestInvalid) }`
became a `match` on the refusal with `errors::set(refusal.code())`. `IngestError::code()`
itself lives in `ingest.rs`, in this job's paths, and is unit-tested there.

## Findings not fixed here

- `crates/graph-wasm/src/ingest.rs:21` — the `EdgeKind`/`NodeKind` imports are used only by
  `ingest/tests.rs`, so the **wasm32 release** build prints `warning: unused imports`. Pre-existing
  on this tree (present before this job's first edit), invisible to the native floor because
  `mod ingest` is `#[cfg(any(test, target_arch = "wasm32"))]`, and not this job's finding. It is
  in this job's paths and is a two-line move into `#[cfg(test)]`.
- `ingest/tests/ceiling.rs` allocates `MAX_INGEST_BYTES + 1` = 512 MiB of whitespace to pin the
  boundary with the real constant. It is the cheapest honest way to test the boundary at the
  measured number, and it is the reason that test takes ~4 s.

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