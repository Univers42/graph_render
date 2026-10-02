# Job ux-forces-full (agent build: every live-force parameter the motor takes, as a studio control)

Why (user, 2026-10-03): on dense graphs whole regions are unreadable, nodes piled on one spot. The
user wants to steer link force, distance, gravity "and much more".

Facts (verified 2026-10-03, confirm before editing):
- `packages/graph-studio/src/motor/live.ts:7-27`: `ForceKnobs` has 4 fields (`gravity`, `charge`,
  `linkStrengthScale`, `linkDistance`), plus `KNOB_LIMITS` and `DEFAULT_KNOBS`.
- The motor's live session takes 13 `f64`s: `gm_force_session_set_params`
  (`crates/graph-wasm/src/exports/session.rs:60-75`), the SDK's `ForceParams`
  (`crates/graph-sdk-js/src/types.ts:137`) and `LiveParams`
  (`crates/graph-core/src/layout/force/session/live_params.rs:181`). That file's range constants are
  the only bounds (its header, line 9).
- Not exposed today: `collide_radius`, `velocity_decay`, `alpha_decay`, `theta`, `distance_max`,
  `distance_min`, `alpha_min`, `initial_alpha`.
- `packages/graph-studio/src/actions/forces.ts:~70` builds the 4 knob actions;
  `ui/ForcesPanel.tsx` draws them.

Do, in order:
1. Measure first. Use `force/clustered.json` and a synthetic 10 000-node source. After the settle at
   defaults, count the node pairs whose drawn discs overlap on screen. Write the count as a
   re-runnable probe in the `deploy/nav/` pattern; a one-off console paste does not count. Record it.
2. Add five knobs, each a registry action `forces.<name>` with a console alias, a slider and a
   persisted setting:
   - collide radius ("Node spacing")
   - velocity decay ("Friction")
   - alpha decay ("Cooling")
   - distance max ("Repel range")
   - theta ("Accuracy")

   Read each slider's limits from the motor's ranges; never let a slider go wider than the motor
   accepts. Saved state written before this job has no value for the new keys; it loads with the
   defaults.
3. Collide radius is in layout units and the user sees pixels. Convert the drawn node radius into
   layout units at the current scale. If that is not possible, say why in the measurement doc.
4. Add two presets:
   - `forces.spread`: collide radius = drawn radius plus a margin, and stronger repel.
   - `forces.compact`.

   `forces.reset` restores all nine knobs.
5. Measure after:
   - overlapping pairs at the defaults and after `spread`;
   - live-loop frames per second at 10 000 nodes, which must stay within 3% of before.

Paths: `packages/graph-studio/src/{motor/live.ts,actions/forces.ts,ui/ForcesPanel.tsx,state/*}` and
their tests, `deploy/nav/` (the probe), and `docs/measurements/ux-forces-full.md`.

Out of bounds:
- `crates/`
- `packages/graph-render/`
- `motor/liveLoop.ts` (perf-p6-live-copy)
- `studio/pipeline.ts`
- `element.ts`, `motor/{bridge,session,settle}.ts`: the open branch perf-pm-live edits them.
  perf-pm-live only appends to the end of `live.ts`, so edit `live.ts` inside its knob block
  (lines 1-30) only, and the merge stays clean. The knob-to-wire table is
  `motor/liveSession.ts:24`.

No new dependency. House limits: 40 lines per function, 300 lines per file, 4 parameters.

Done when:
- `scripts/studio.sh check` exits 0.
- `scripts/studio-live.sh` and `scripts/studio-interact.sh` exit 0, and each negative control
  exits non-zero.
- `perf-p5.rows` is green.
- A pw MCP pass on the built studio is done: open the clustered fixture, run `spread`, and save a
  before and an after screenshot under `docs/measurements/ux-forces-full/`. Read the store error,
  the console exceptions and the overlay text. Any one of them non-empty is a FAIL.
- The measurement doc has the before/after table.
