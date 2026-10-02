# Job perf-studio-fa2bh (agent build): the studio opens on ForceAtlas2 over Barnes–Hut

Why: the studio's default layout is `layout.forceatlas2`, the dense O(n²) port (ceiling 14 000;
about a second natively at 4 000 nodes, several in wasm). `layout.forceatlas2.barnes_hut` is the
same layout with the repulsion summed over a Barnes–Hut tree, O(n log n), ceiling 250 000
(`docs/measurements/perf-fa2bh.md`, `crates/graph-core/src/registry/forceatlas2_bh.rs`). The user
asked for 4 000 nodes to render fast; this is the switch. The exact layout stays in the catalog.

Facts:

- The one id the studio sources name is the default layout: `packages/graph-studio/src/state/settings.ts:159`
  (CLAUDE.md, "Architecture (studio)").
- The perf driver's force layout: `deploy/perf/run.py:35` `FORCE_LAYOUT`.
- Tests that read the default rather than a catalog of their own: `packages/graph-studio/tests/space-3d.test.ts:46`
  asserts `run.layoutId`; check `live-session.motor.test.ts:12` and `refusals.motor.test.ts`
  (they name the id themselves, so they may stay). Tests that only list `layout.forceatlas2` in a
  fake catalog (`desk.ts`, `drawn.ts`, `parity.test.ts`, `console.test.ts`, `registry.test.ts`,
  `recipe.test.ts`, `ui/*.test.tsx`) are about the catalog, not the default: leave them.
- Paths you may edit: the two files above, tests under `packages/graph-studio/tests/` that fail
  because of the switch, and `docs/reports/perf-studio-fa2bh.md`. Nothing under `crates/`,
  `packages/graph-render/`, `src/` or `app/` except what `scripts/studio.sh wasm` regenerates.

Do:

1. `scripts/studio.sh wasm` (stages the wasm and fixtures), then switch the two ids.
2. `scripts/studio.sh check` and fix only tests that read the default. Each fix keeps the test's
   intent; write the reason in one line in the report.
3. `scripts/studio.sh build`, then `scripts/studio-smoke.sh` and its negative control
   `STUDIO_SMOKE_BREAK=1 scripts/studio-smoke.sh` (expect non-zero), then `scripts/studio-nav.sh`.
4. Time the default open at 4 000 nodes before and after if a driver exists for it
   (`deploy/perf/`, `scripts/studio-perf.sh`; read their headers). If none runs headless here,
   write "not run" with the reason.
5. Report `docs/reports/perf-studio-fa2bh.md`: what changed, each gate with its exit code, the
   timing table or "not run".

Done when: `scripts/studio.sh check` exits 0, smoke exits 0 and its BREAK run exits non-zero,
studio-nav exits 0, and the report states each.
