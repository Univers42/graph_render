# The live force session

**Status:** decided (M1 landed; M2–M4 not started). **Date:** 2026-09-29.
**Code:** `crates/graph-core/src/layout/force/session{.rs,/}` — `ForceSession`, `LiveParams`,
`NodeRow`. **Numbering:** this is the twelfth ADR; the eleven before it are unnumbered files
(`circular-conventions.md` … `sugiyama-heuristics.md`), so `12-` is a count, not a phase.

## The deviation being recorded

`crates/graph-core/src/layout/force/params.rs` opened with *"The frozen force set (`P56_SPEC.md`
decision 7, devil C10)"* and closed with *"No setters: the frozen set is a single point, not a
knob surface, for this phase."* `ForceParams` is a `Copy` struct with twelve public fields, a
`Default`, and — as that header says — no way to change any of them after construction. A force
simulation in a motor that only ever runs 112 ticks from a golden spiral and hands back one
picture is not a force simulation; it is a function.

**The user authorised live physics on 2026-09-29.** That authorisation is what this ADR records,
and it is a deviation from a written decision, so it is written down rather than absorbed
silently: the frozen set is no longer frozen, `ForceParams` survives only as the *hashable stage
parameter* (graph-cli's `Setting` and the `GM_MUTATE_FORCE_THETA` control still name it, and the
4-way hash gate hashes `layout.force.barnes_hut` at exactly those values), and every new knob
lives on `LiveParams` instead.

## What did **not** change

**`layout.force.barnes_hut`'s bytes.** This is the whole constraint, and it is why M1 is a
refactor and not a feature. The frozen layout is now defined *as* the degenerate session:

> a `ForceSession` with [`LiveParams::default`], no pins, `alpha_target == 0`, `gravity == 0`,
> stepped `TICKS` (112) times.

`BarnesHut::run` builds that session and reads its positions out. The 65 golden digests in
`layout/force/session/tests/golden.rs` were captured from the **pre-refactor** stage over
`seeded_model(seed, gate_node_count(seed), REFERENCE_DEGREE)` for `seed` 0..64, and are
asserted twice: against the session's own columns and against the stage's. A byte that moves
is a failing test rather than a reviewer's diff, and a change that moved *both* together —
the only way a refactor of this shape could quietly pass — fails on the first. Three
details make the equality possible rather than nearly possible:

- **`alpha_target == 0` is bit-identical to the old `alpha += -alpha * alpha_decay`.** d3's own
  form is `alpha += (alphaTarget - alpha) * alphaDecay` (`simulation.js:45`); with
  `alphaTarget = 0.0`, `0.0 - alpha` is exactly `-alpha` for every `alpha > 0` that a cooling
  schedule produces, and for `alpha == +0.0` the only difference is the sign of a zero that is
  added to `+0.0`. Asserted tick by tick over all 112 rather than argued
  (`the_cooling_schedule_is_d3s_form_and_agrees_with_its_simplification`).
- **Pins are opt-in.** `pin` writes `fx`/`fy`; with both `None` the integration tail is the two
  statements it always was.
- **`gravity == 0` skips the force entirely**, as the milestone requires. The arithmetic behind
  that requirement is real — `(0 - x) * 0.0` is `+0.0` for `x < 0`, and `-0.0 + +0.0` is `+0.0`,
  a different `f64` with different bytes, pinned in `session/gravity.rs` — but it is **not
  observable through the tick today**, which is worth writing down rather than leaving for a
  mutant run to discover: every node's velocity passes through an addition in the many-body
  force on every tick (that force always contributes, at the very least `±0.0` past
  `distanceMax`), and IEEE addition yields `-0.0` only from a sum in which *both* operands are
  `-0.0`, which no state reachable from a session can be, because every velocity starts at
  `+0.0`. Replacing `>` with `>=` in `Sim::tick` therefore survives every test, and the
  milestone's report says so with the proof attached. The guard is kept: it is the specified
  contract, it costs one comparison per tick, and it is the only statement in the crate that
  says a force is *off* rather than *zero*.

## Contract narrowing

The frozen stage (`ForceSession::from_frozen`) refuses only non-finite parameters (D9); every finite value that ran before the refactor still runs, `theta` 0.1 included.
Range checks live on `LiveParams::validate` and the live setters (`reheat`, `set_alpha_target`) only.

## Alternatives considered

1. **Keep `ForceParams` frozen and add a second, live parameter struct.** Rejected: two structs
   with twelve of thirteen fields in common is a field-by-field conversion bug waiting to happen,
   and the frozen one would keep a knob surface (`GM_MUTATE_FORCE_THETA`) that the new one
   contradicts. One struct, one set of ranges, one `From<ForceParams>`.
2. **Make the stage parameter `LiveParams` outright and delete `ForceParams`.** Rejected *for
   M1*: `graph-cli`'s `Setting`, the `GM_MUTATE_FORCE_THETA` control and the ledger's oracle text
   all name `ForceParams`, and those are not this task's paths. `LiveParams: From<ForceParams>` is
   the whole conversion, and deleting the frozen struct is a one-line follow-up once the CLI is in
   scope.
3. **A second tick loop inside the session.** Rejected outright. The session *owns* the existing
   `Sim`; the forces, the quadtree and the scratch buffers are the ones the frozen layout already
   hashes. A second loop would be a second set of bytes and a second set of bugs.
4. **Validation by clamping instead of refusing.** Rejected: a clamp is silent. A caller that asks
   for `theta = 2.0` and gets `1.5` has been lied to, and the error surfaces as a layout that
   looks plausible. Every field is range-checked and every refusal names the field, the range and
   nothing else; `set_params` leaves the session untouched.
5. **Expose the dense index.** Rejected: the dense index never leaves the motor
   (`prompt.md` §4, "Identity"). `NodeRow` is the node's row in the snapshot columns — today that
   is the dense index, and `session/tests/m1d.rs` pins the mapping so it cannot quietly drift.

## What M2–M4 will add

Only the names are fixed here; each is a later job with its own tests and its own ADR entry if it
re-opens a settled question.

- **M2 — the WASM/SDK surface.** `gm_force_session_*` over the existing columnar ABI
  (`docs/contract/wasm-abi.md`): open, warm-start, step, pin/unpin, reheat, read the columns. The
  reason it is not M1 is that the ABI is versioned and hashed (`crates/graph-cli`'s transport
  stage), so a new verb is a wire change and belongs in its own change.
- **M3 — the interactive loop.** M1 landed the verbs a loop needs (`step`, `reheat`,
  `alpha_target`, the settle predicate, `set_params`); what is missing is a loop that uses
  them — one tick per frame against the 16.67 ms budget of `prompt.md` §5.2, measured on a
  real session rather than on a 112-tick one-shot, and the studio's frame driver on top of
  it. The performance claim in `prompt.md` §5.2 ("the single number to hold") has never been
  measured against an interactive loop, because there has not been one.
- **M4 — the layouts that need it.** The force family beyond the frozen set: Yifan Hu as
  multilevel coarsening over *this* session (registered under a name that says it is not
  Graphviz's `sfdp`, per `phase-06` step 4), and whatever the studio panels ask for.
