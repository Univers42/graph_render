# Job svc-caps-contract — the `source=contract` ingest term of the service's per-slot memory

Branch `svc-caps-contract` (from `svc-server`, develop merged in). `fix-contract-quadratic` is on
develop, so the contract reader is no longer quadratic and its ingest term can be measured at the
64 MiB body limit. Today `PER_SLOT_BYTES` (`server/graph-server/src/config/slots.rs:12`) uses only the
studio term, and `docs/measurements/service-caps.md:154` reads "pending fix-contract-quadratic".

## Allowed paths

- `docs/measurements/service-caps.md`
- `server/graph-server/src/config/slots.rs`
- `server/graph-server/src/config/tests.rs` (only if a number there must move)
- `crates/graph-wasm/src/memory_measure/ingest_peak.rs` (the module doc comment only; no code change)

Anything else is a "decisions needed" item, not an edit.

## Tasks, in order

1. Run the measurement twice, one after the other, never in parallel:
   `scripts/orch/gr cargo test --release -p graph-wasm --lib -- --ignored --nocapture ingest_peak`
   Keep both printed tables verbatim (source, body bytes, nodes, ms, heap peak bytes, resident rise).
   If the contract row panics, times out or exits 137, stop: report the command and the last 30 lines.
2. Run `scripts/orch/gr cargo test --release -p graph-wasm --lib -- --ignored --nocapture contract_read_time`
   once and keep its table. It shows whether time per doubling is now ~2x (linear) or still ~4x.
3. Compute `contract_term` = the larger heap peak of the two `contract` rows from task 1.
   Compute `per_slot` = 67,108,864 + max(141,099,952, contract_term) + 3,343,908,864. Show the sum.
4. Edit `docs/measurements/service-caps.md`:
   - In the "Memory per slot" table, replace the `pending fix-contract-quadratic` cell with
     `contract_term` and a "How it was measured" cell in the style of the studio row (the command, body
     bytes, nodes, ms, both runs' heap peak and resident rise).
   - Set the `per_slot` row to the new value with its GiB form, and say which ingest term binds.
   - Recompute the N = 1, 2, 4, 20 container limits and the memory.max 4, 8, 16, 32, 64 GiB → workers
     list with `workers = min(cores, floor(memory.max / per_slot))` (base = 0). Show one division.
   - Retitle the section "Ingest term for `source=contract`: pending fix-contract-quadratic" to
     "Ingest term for `source=contract`". Keep the before-fix table and label it "before the fix". Add an
     "after the fix" table from tasks 1 and 2. Replace the bullets that estimate 1.23 GB and the
     "will be re-measured" line with the measured result.
   - In "## Caveat" → "Not covered", delete the bullet "The contract ingest term: pending, see above."
5. Edit `server/graph-server/src/config/slots.rs`:
   - `PER_SLOT_BYTES` becomes the task 3 value, with `_` digit groups.
   - Its doc comment names both ingest terms and which one binds.
   - In the Caveat, delete the sentence "It is wrong for `source=contract`, whose ingest term waits on
     fix-contract-quadratic." Keep the over-estimate and the `GRAPH_WORKERS` sentences.
   - If the new value makes the `BASE_BYTES` Caveat's "3.3 GiB" wrong, fix that number too.
6. `server/graph-server/src/config/tests.rs` is written in multiples of `PER_SLOT_BYTES`, so it should
   pass unchanged. Run
   `scripts/orch/gr cargo test --manifest-path server/Cargo.toml -p graph-server config`.
   Edit the file only if a test fails, and say why.
7. In `crates/graph-wasm/src/memory_measure/ingest_peak.rs`, fix the module doc comment (lines 8-11),
   which still says the reader "is quadratic in records". State the complexity task 2 measured.
   Change no code.
8. Run `scripts/orch/gr cargo fmt --all --check` and
   `scripts/orch/gr cargo fmt --manifest-path server/Cargo.toml --all --check`.

## Done when

- Both `ingest_peak` runs and the `contract_read_time` run are printed verbatim in your return block.
- `service-caps.md` has no "pending" left: `git grep -n pending docs/measurements/service-caps.md`
  prints nothing.
- `PER_SLOT_BYTES` equals the task 3 sum, and the doc table shows the same number.
- The config tests and both fmt checks pass, with their output lines in the return block.

## Return block, extra lines

- `per_slot: <old> -> <new>`
- `workers at 8g: <n>`. The svc-process job's `svc-memory` row expects 2 workers at
  `drun --memory 8g`. If <n> is not 2, add that row under "decisions needed".
