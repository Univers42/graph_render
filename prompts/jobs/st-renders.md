# Job st-renders (agent build, studio speed regression)

Why: the user, 2026-10-06: "the speedness of the app has dropped significantly". Measured with
`scripts/studio-perf.sh --label`, SwiftShader, 1920x1080, base `01b0a330` against `d2c34b67`:

| measure | base | now |
|---|---:|---:|
| React renders at open, 120 nodes DPR 1 | 749 | 1114 |
| React renders at open, 10000 nodes DPR 2 | 608 | 748–866 |
| open ms, 120 nodes DPR 2 | 25 | 44 |
| open ms, 10000 nodes DPR 2 | 168 | 249 |
| `perf-block` worst main-thread block per layout switch, 500 nodes | 7.1 ms | 16 ms |
| blocked ms at 500 nodes: grid / spectral / forceatlas2 / dag.sugiyama | 4.9 / 2.8 / 3.1 / 1 | 8.5 / 9.2 / 12.1 / 12.3 |

Frame rate did not regress (2000 nodes DPR 1: 13.9 → 18.5 fps). The cost is on the main thread at open
and at each layout switch, which is React and the studio pipeline, not the motor (it runs in a worker).
93 commits touched the studio between the two; `git diff --stat 01b0a330 d2c34b67 -- packages/graph-studio/src`.

Leads, to confirm or reject with a measurement each:
- `ui/Dock.tsx:118` and `ui/Console.tsx:77` subscribe to the whole store (`useStudioState`,
  `ui/useStudio.ts:7-9`), so every `patch` re-renders them and all their children. `ui/useStudio.ts:27`
  already has `useStudioSelector`, which `ui/Shell.tsx:93-100` uses.
- Panels added since the base (`ui/LayoutParamsPanel.tsx`, `ui/HoverCard.tsx`, `ui/Preview.tsx`,
  `ui/paramSpecs.ts`) and whether they are `memo`ized with stable props.
- `restyle` (`studio/pipeline.ts:105-110`) rebuilds the whole style on every draw, including a layout
  switch whose look did not change.

Do:
1. Record `scripts/studio.sh build && scripts/studio-perf.sh --label st-renders-before` first.
2. Fix the causes you measured, at the shared function. Behaviour must not change.
3. Record `scripts/studio-perf.sh --label st-renders-after` and write
   `docs/measurements/studio-renders.md`: the before/after rows above, the cause of each saving with its
   `file:line`, and a `Caveat:` line (SwiftShader timing on a loaded host; name the load average).

Paths: `packages/graph-studio/**`, `docs/measurements/studio-renders.md`. Nothing else.
Limits: ESLint `--max-warnings 0`, 300 lines per file, 40 lines per function, 4 parameters, no type
assertions. If an edit fails twice, read the whole file and write it once.

Done when: `scripts/studio.sh check` exits 0; React renders at open at 120 nodes DPR 1 <= 800 and the
`perf-block` row <= 10 ms in `target/studio-perf/st-renders-after/table.md`; `scripts/studio-smoke.sh`
exits 0 and `STUDIO_SMOKE_BREAK=1 scripts/studio-smoke.sh` exits non-zero.
