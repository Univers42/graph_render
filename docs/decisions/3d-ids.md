# 3-D capability ids: develop's scheme is canonical (Option A)

- Status: accepted
- Rules on: `docs/measurements/assess-3d-branches.md` §Branch 1, §Overlaps and §Recommendation ("Recommended: **A**"), on `p12-t4a` (`971318dc`, base `bdb04f15`, a tree 1573 files behind develop).

## Decision

Develop's ids win. Four of t4a's five 3-D arms are duplicates by content — the same run functions
(`spectral_stage::spectral_3d`, `spectral_stage::pivot_mds_3d`) and the same ceiling values, under different id strings — and Option A is the smaller diff and the only option that keeps the existing gates. Only `layout.random.3d` is new; it keeps t4a's name, its `.3d` suffix matching the `layout.force.*.3d` family. The map, t4a → develop:

- `layout.spectral.3d` → `layout.spectral3d` (`registry/spectral.rs:145`)
- `layout.mds.pivot.3d` → `layout.mds.pivot3d` (`registry/spectral.rs:153`)
- `layout.spiral.3d` → `layout.basic3d.spiral` (`layout/basic_3d/spiral.rs:58`)
- `layout.bipartite.3d` → `layout.bipartite_3d` (`layout/basic_3d/bipartite_3d.rs:58`)
- `layout.random.3d` → unchanged, and new on develop (`registry/three_d/random3d.rs`)

## Consequences

No `git merge` or `cherry-pick` of t4a: `registry.rs` cannot apply, its `LAYOUTS` table having moved to `registry/layouts.rs`. `LAYOUTS` is append-only (the wasm module maps a layout by index), so `layout.random.3d` takes the last slot — index 47, `[Capability; 47]` → `[Capability; 48]` (`registry/layouts.rs:49`). One ceiling disagreement resolves in develop's favour: t4a's `CLOSED_FORM_3D_CEILING = CLOSED_FORM_CEILING` (1 000 000) loses to `BASIC_3D_CEILING = MAX_BENCH_NODES` for the same spiral/bipartite content, and `layout.random.3d` takes `BASIC_3D_CEILING`. What was ported, what was not, and the measured numbers: `docs/measurements/p12-3d-oracles.md`. Alternatives B and the other branch's `..._3d` are argued in the assessment's §Recommendation, not repeated here.