> **Status (2026-09-28):** BUILT on branches p6e (spectral/MDS/eigen) and p6f (Barnes-Hut/FA2) — cores only; wiring, hashgate stages, stress/bench, python oracle image pending. Yifan Hu absent by decision. See docs/reports/STATUS.md.

# Phase 6 — Iterative and spectral layouts. The hard one.

**Read `prompt.md` and `prompts/REFERENCES.md` first.** Phase 5's gate must be green.
**A `devil` verdict is required before any code in this phase** (`prompt.md` §11 guardrail 7): physics
rewrite, wide blast radius, and the first genuinely chaotic determinism surface.

## Goal

Force-directed (Barnes-Hut), ForceAtlas2, Yifan Hu multilevel, **spectral**, and **Pivot MDS**.

## Why this is last among the layouts, and why that ordering is deliberate

Everything in Phases 2–5 is deterministic by construction: one pass, no cooling, no eigenspace, no
convergence criterion. This phase is the opposite, and it contains the two hardest determinism problems
in the project:

1. **Chaos.** A force simulation amplifies a 1-ULP difference into a visibly different picture within a
   couple of hundred ticks. Bit-identical or divergent — there is no "close enough". Every constraint in
   `prompt.md` §6 is load-bearing here, and D1 (`libm`) most of all.
2. **Degenerate eigenspaces.** When λ₂ = λ₃ — which happens on *grids and trees*, i.e. common input —
   the solver returns whatever rotation its start vector lands on. Without pinning, the layout changes
   between runs on the same machine.

By now the hash gate, the transport, the registry and the round-trip are all proven on calm cases, so a
divergence here is attributable to the algorithm. Reversing this order means debugging four things at
once.

## The reference already solved the determinism problem — do not re-derive it

`SciGraphs/core/scigraphs_core/mesh/layouts/networkx_layouts.py` contains the answers, with reasons, in
comments. Port these **as decisions**, not as optional polish. `prompts/REFERENCES.md` lists each with
its line number; the essentials:

- **`_eig_start_vector(n)`** (`:57-66`) — a *fixed* start vector orthogonal to the constant null vector.
  Its comment states exactly why: *"Grids and trees have lambda2 == lambda3, and inside a degenerate
  eigenspace the solver returns whatever rotation its start vector lands on, so an arbitrary one makes
  the layout differ between runs."*
- **`_fix_eigenvector_signs`** (`:68-74`) — eigenvector sign is arbitrary; pin by the largest-magnitude
  entry.
- **`_eig_converged`** (`:76-79`) — verify `L v = λ v` by residual, *"because a solver that stops on its
  iteration cap still returns numbers."* **Never trust a solver's exit status.**
- **Per-connected-component solving** (`:139-175`) — *"The Laplacian null space holds one vector per
  component, so a whole-graph solve hands back component indicators and every component collapses to a
  single point."*
- **`which='SM'` is rejected** (`:82-88`) with a measured reason: small Laplacian eigenvalues cluster, so
  ARPACK restarts to its `10 * n` cap — *"on a 200k-node mesh means hours rather than a failure the
  caller can fall back from."*
- **Tier constants** (`:7-15`): `_DENSE_EIG_LIMIT = 256`, `_EIG_RESIDUAL_TOL = 1e-2`,
  `_LOBPCG_MAXITER = 300`, `_MDS_PIVOTS = 100`, `_COMPONENT_SPACING = 2.5`.
- **`_reset_layout_rng`** (`common.py:53-61`) — seeds the shared RNG *and* stdlib `random`, because a
  library in the chain ignores the seed argument. Our analogue: every RNG is explicit and threaded; there
  is **no global RNG** in `graph-core` at all.

## Authorization envelope

**CREATE — exactly these:**
```
crates/graph-core/src/layout/force/{mod.rs,barnes_hut.rs,quadtree.rs,params.rs}
crates/graph-core/src/layout/{forceatlas2.rs,yifan_hu.rs,spectral.rs,pivot_mds.rs}
crates/graph-core/src/linalg/{mod.rs,lanczos.rs,dense_sym.rs}
crates/graph-core/src/rng.rs                  (explicit, threaded, no globals)
fixtures/force/{grid.json,tree.json,clustered.json,disconnected.json,single-node.json}
docs/decisions/eigensolver.md                 (REQUIRED — see step 5)
docs/measurements/phase06-{force,stress,eigen}.md
```

**MODIFY — exactly these:**
```
crates/graph-core/src/layout/mod.rs
crates/graph-core/src/registry.rs
crates/graph-core/Cargo.toml                  (no new deps beyond libm/indexmap — see step 5)
crates/graph-cli/src/{capabilities.rs,main.rs}
harness/oracle-layouts.mjs                    (the d3-force arm)
```

**FORBIDDEN:** `src/`, `tests/`, `verify/`. Edge bundling/routing (Phase 8). igraph-family layouts (not
ported). Anything in osionos.

## Steps

### 1. `rng.rs` — explicit, threaded, no globals

Seed is an input, never ambient. A named deterministic generator (document which — mulberry32 or PCG) with
its exact constants. **No `rand` crate default RNG**, no thread-local state, no global seeding. The
reference had to seed stdlib `random` process-wide to plug a library that ignored its seed; our version of
that fix is to have no ambient RNG to plug.

### 2. `quadtree.rs` — built into a reused buffer

Over the dense SoA arrays. **Reused across ticks — no per-tick allocation** (`dsa-and-memory.md`). Build
order must be deterministic: a fixed subdivision rule, fixed child ordering, fixed tie-break for
coincident points.

### 3. `force/` — Barnes-Hut

Oracle: `d3-force` (on disk, and the engine's existing reference at `forceLayout.ts:9-26`). Port the
cooling schedule, θ (opening angle), and the force terms to match.

**The honesty requirement, and it must appear in the report:** d3's `forceManyBody` *already* uses a
quadtree. This is **not** an asymptotic win — it is a constant-factor win (SoA memory, no GC, no JS object
overhead, more ticks per frame). Per `minimalism-ladder.md`'s performance override the justification is a
measured number at **N = 220 / 10k / 100k**, never an O-notation argument. N=220 is today's real graph
size and is the one most likely to show WASM *losing*; measure it first and report it even if it is
unflattering.

Determinism: seeded initial positions (the reference uses a golden-spiral seeder — see
`forceLayout.ts:63`), fixed accumulation order, **no parallel reduction** (D3), no `mul_add` (D2).

**Gather form from day one (D10, `docs/decisions/compute-tiers.md`).** Every per-tick kernel is written
as range kernels `step_range(state, range, out)`: node `i` reads only start-of-tick state and writes only
its own output, summing in a fixed order (quadtree order for many-body, CSR order for links). This is
the groundwork that lets Phase 11's SIMD, threads and GPU tiers be a port rather than a redesign.
- `forceManyBody` is already a per-node gather in d3 (`d3-force@3.0.0 src/manyBody.js`: one
  `tree.visit(apply)` per node) — port it as such.
- **d3's `forceLink` is a sequential Gauss–Seidel scatter** (`src/link.js` `force(alpha)`): each link
  adds into *both* endpoints, reading velocities already modified by earlier links. Two threads would
  race on a node, and the result depends on link order. Implement the **Jacobi/gather** form instead:
  each node sums its own incident links (CSR order) from start-of-tick state. This is a deliberate
  deviation from d3 — record it in `phase06-stress.md`; it is legitimate because force is gated on
  stress quality, not identity.
- Positions/velocities are double-buffered (reused buffers, no per-tick allocation); the quadtree is
  built once per tick, single-threaded, before the kernels run.
- Write the loops over SoA columns so they autovectorise across nodes (Phase 11 tier 1b); no
  horizontal reductions.

### 4. ForceAtlas2 and Yifan Hu

`prompts/REFERENCES.md` marks both as **upstream-reference-needed**: SciGraphs delegates FA2 to networkx/
`fa2`, and Yifan Hu to Graphviz `sfdp`. Per rule 0.6, **stop and ask for the reference** rather than
improvising the formulation. Do not infer FA2's repulsion/gravity/`linlog` semantics from its parameter
names.

Yifan Hu may alternatively be built as *multilevel coarsening over our own force implementation* — which
is honest and self-contained, but it is **a different algorithm from Graphviz's sfdp** and must be
registered under a name that says so. Ask before choosing.

### 5. The eigensolver decision — `docs/decisions/eigensolver.md` is a required deliverable

`graph-core`'s dependency allow-list is closed (`libm`, `indexmap`). Spectral layout needs an
eigendecomposition. There is no `scipy` here, so the fork is real and must be **decided and written down
before coding**:

| Option | Cost | Determinism |
|---|---|---|
| Dense symmetric (Jacobi or QL/QR, hand-written in `dense_sym.rs`) | O(n³) — hence the reference's `_DENSE_EIG_LIMIT = 256` | fully deterministic, fixed iteration order |
| Iterative (Lanczos / LOBPCG, `lanczos.rs`) | scales to large n | convergence path is float-reduction-order sensitive (**D3**) — bit-identical across targets is *not* free |
| Add a dependency (`nalgebra`) | — | requires opening the allow-list → **stop-and-ask** |

The reference's own answer is a **cascade**: dense below 256, LOBPCG above, shift-invert as fallback, each
verified by residual. Mirroring that cascade is the recommended path, with this caveat to record: if the
iterative tier cannot be made bit-identical across native and wasm32, then **spectral layout is registered
as per-platform reproducible only** — a documented exception to `prompt.md` §6, explicit in the ledger's
`degradation` field, not a silent weakening of the project's central guarantee.

Write the decision, the reason, and the measured residuals into `docs/decisions/eigensolver.md`.

### 6. `spectral.rs`

Laplacian per connected component, `dims` non-trivial eigenvectors above the null vector, fixed start
vector, sign pinning, residual verification. Component coordinates normalised by peak magnitude and spaced
by the documented constant.

Single-node and two-node components are degenerate — the reference skips components with `len < 2`
(`:159`). Handle them explicitly and test them; `fixtures/force/single-node.json` exists for this.

### 7. `pivot_mds.rs` — Pivot MDS, not classical MDS

Reference: `networkx_layouts.py:176-215`. The algorithm:

1. choose `k = min(_MDS_PIVOTS, n)` pivots by **farthest-point selection** (first pivot fixed at index 0
   for determinism, then repeatedly the node maximising its minimum distance to the chosen set);
2. BFS/Dijkstra hop distances from each pivot → an `n × k` matrix;
3. square, double-centre (subtract column means, subtract row means, add the grand mean, scale by −0.5);
4. leading eigenvectors of the `k × k` matrix `Cᵀ C`;
5. project, then pin eigenvector signs.

**Complexity: O(k(n+m)) time, O(nk) memory** — against O(n³)/O(n²) for classical MDS over a full distance
matrix. An earlier draft of the plan wrongly claimed an O(n²)-memory ceiling here by assuming classical
MDS; there is none. The `k × k` eigendecomposition is small, so the dense solver suffices and the Phase-6
eigensolver risk **does not apply to MDS** — only to spectral.

Unreachable pivots (disconnected components) give infinite distances; the reference zeroes them (`:198`).
Match that and name it in the Ponytail — it distorts geometry across components rather than failing.

### 8. The stress metric — because pixel identity is impossible here

Phase 3's layouts have oracles that agree exactly. Force layouts do not, even between two correct
implementations. So the gate is:

1. **Determinism** — 4-way hash equality. Non-negotiable.
2. **Quality, not identity** — a stress metric (correlation of graph-theoretic distance with euclidean
   distance, or edge-length variance). Assert we are **no worse than the d3 baseline by a stated margin**.
   The honest claim is *"different, but not worse"*, recorded in
   `docs/measurements/phase06-stress.md`.
3. **A measured speed win**, at all three N values.

## Gate

```sh
docker run --rm -v "$PWD:/w" ge-rust cargo fmt --check                                        # 0
docker run --rm -v "$PWD:/w" ge-rust cargo clippy --workspace -- -D warnings                   # 0
docker run --rm -v "$PWD:/w" ge-rust cargo test --workspace                                    # 0
docker run --rm -v "$PWD:/w" ge-rust cargo build -p graph-core --target wasm32-unknown-unknown  # 0

# determinism is the hard gate here
docker run --rm -v "$PWD:/w" ge-rust cargo run -p graph-cli -- hashgate --seeds 1000            # 0
docker run --rm -v "$PWD:/w" -e GM_MUTATE_REFERENCE_DEGREE=9 ge-rust \
  cargo run -p graph-cli -- hashgate --seeds 8                                                 # NON-ZERO

# no forbidden float constructs
docker run --rm -v "$PWD:/w" ge-rust sh -c 'grep -rn "mul_add" crates/graph-core/src && exit 1 || exit 0'  # 0

# eigen: residual-verified, degenerate eigenspace pinned, run-to-run stable
docker run --rm -v "$PWD:/w" ge-rust cargo test -p graph-core eigen_determinism                 # 0

# quality + speed, measured
docker run --rm -v "$PWD:/w" ge-rust cargo run -p graph-cli -- stress --oracle d3               # 0
docker run --rm -v "$PWD:/w" ge-rust cargo run -p graph-cli -- bench --n 220,10000,100000       # 0

docker run --rm -v "$PWD:/w" ge-rust cargo run -p graph-cli -- capabilities --check             # 0
docker run --rm -v "$PWD:/w" -w /w node:22-slim node harness/sdk-smoke.mjs                      # 0
docker build -t ge-check . && docker run --rm ge-check                                          # 0
```

## Ledger delta

To `gated`: `layout.force.barnes_hut`, `layout.spectral`, `layout.mds.pivot`. FA2 and Yifan Hu only if
their references were obtained — otherwise they stay `absent` with the reason recorded, which is a valid
and honest phase outcome.

`layout.spectral` must state its eigensolver tier and, if applicable, that it is **per-platform
reproducible only**. That exception belongs in `degradation`, visible, never implied.

## Ponytail requirements

- **Force layout is chaotic**: the same graph with one node added is a different picture, not a perturbed
  one. Name it; direction is cosmetic-but-surprising, escape hatch is a fixed seed.
- **Spectral in a degenerate eigenspace**: pinned by a fixed start vector; a *different* fixed vector
  gives a different-but-equally-valid layout. Name the failing input (grids, trees — λ₂ = λ₃).
- **Iterative eigensolver non-convergence**: falls back; if all tiers fail the layout is skipped for that
  component. Direction: **a skipped component sits at the origin** — visibly wrong, the dangerous
  direction. Escape hatch: the residual is reported.
- **Pivot MDS** approximates the full distance matrix from k pivots; accuracy degrades as k/n shrinks.
  Unreachable pivots are zeroed, distorting cross-component geometry.
- **Barnes-Hut θ** trades accuracy for speed; a large θ visibly clumps distant nodes.

## Stop-and-ask

- 4-way hash equality fails and D1–D9 are all satisfied → **stop and report**. This is the finding that
  matters most in the project; do not add a tolerance.
- FA2 or Yifan Hu reference unavailable → stop (rule 0.6). Do not improvise a force formulation.
- The eigensolver cannot be made bit-identical → stop, present the measurement, and get the per-platform
  exception approved explicitly. Do not take that decision alone.
- WASM is *slower* than the TypeScript at N=220 → **report it, do not hide it**. That is a legitimate,
  expected outcome at small N, and the `GraphSource` seam exists so the crossover can be a measured policy
  choice rather than a rewrite.

## Addendum (2026-09-29): `GM_MUTATE_FORCE_THETA` is live

The claim that `GM_MUTATE_FORCE_THETA` is inert, made in `docs/reports/phase-06.md`, is superseded.
Commit `b142c9a` ("updated", 2026-09-29) made the knob live by wiring the mutation into the
Barnes-Hut stage.

The negative control is the `negctl-force-theta` row of
`/sgoinfre/students/dlesieur/orch/rows/develop-full.rows` (line 40; the rows files live outside
this repository, see `CLAUDE.md:123`), which expects a non-zero exit from:

```sh
negctl-force-theta|nonzero|/goinfre/dlesieur/orch/bin/gr -e GM_MUTATE_FORCE_THETA=0.5 cargo run -q -p graph-cli -- hashgate --seeds 8
```

The value is `0.5`, not `0.9`: `0.9` was the compiled-in default
(`crates/graph-core/src/layout/force/params.rs:66`), so it could not diverge. `prompts/RESUME.md:45`
records that the row shipped that way — "`negctl-force-theta` mutated theta to 0.9 = the default
(now 0.5)" — and was fixed to `0.5`. A negative control that cannot fail proves nothing.

The knob reaches the stage through the `setting.force` parameter path:
`crates/graph-cli/src/hashgate/stages.rs:87` runs `BarnesHut::run(t, &setting.force)` rather than the
registry's compiled-in default. The mutation is native-arm-only, so a wired control surfaces as a
native-versus-wasm32 divergence on `layout.force.barnes_hut` and on no other stage. Measured on
this worktree on 2026-09-29, after `b142c9a`, with

```sh
gr -e GM_MUTATE_FORCE_THETA=0.5 cargo run -q -p graph-cli -- hashgate --seeds 8
```

which exited 1: `layout.force.barnes_hut` came back `4-way equal on 2/8 seeds`, and `topology`,
the other nine layouts and `transport.wasm.columnar` all `4-way equal on 8/8 seeds`. The run ended
`PASS`-free with a divergence count of 6 of 8 seeds. The knob's own stage diverges on 2 of 8 seeds,
not all 8, and no other stage moved — which is what a negative control is for. The knob is
documented as live at `crates/graph-cli/src/hashgate/knob.rs:48-56` — the `ForceTheta` doc comment and
the variant itself, which is what records that theta reaches the Barnes-Hut stage alone.

The original phase-6 report text in `docs/reports/phase-06.md` is kept as written. This addendum is
the correction.
