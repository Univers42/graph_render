# P4f — where the wasm32 `extend` timer goes, and the biggest share removed

`docs/measurements/perf-p4e-extend.md` measured the `columns` path at 1M and found the two wasm
arms missed the 30 ms budget by about 1.9×, and it named the next measurement rather than
guessing at it: *"time `encodeBatch` on its own. That is the one quantity A1 really asks for and
this document does not have; isolating it needs a second timer path in
`harness/wasm-stream-bench.mjs`."*

This document is that measurement, and then the fix it chose. `harness/wasm-stream-bench.mjs`
gained `--split`, which puts `encodeBatch` in a timer of its own; the difference
`extend − encode` is everything else the `extend` call does. A CPU profile of the batch loop
then says what that "everything else" is.

<!-- BEFORE-TABLE -->

## What the split says

<!-- SPLIT-NARRATIVE -->

## What the profile says

<!-- PROFILE-NARRATIVE -->

## What was removed, and why it was the biggest

<!-- FIX-NARRATIVE -->

<!-- AFTER-TABLE -->

<!-- VERDICTS -->

## Reproducing

<!-- REPRO -->

## Caveats

<!-- CAVEATS -->
