# sg-fa2-seed — FORCEATLAS2: the reference arm runs ForceSim, not networkx

**Status: stopped at step 0. No code change.** The job body's premise — "scigraphs-utils is in no
oracle image, so the reference arm most likely ran networkx" — is false, and the body names this
outcome as a stop:

> If the reference ran ForceSim or `fa2`, stop and report under "decisions needed": graph-core's
> FA2 is a networkx port, and the matching port would be a different job.

It ran ForceSim. Proof below, then the four other body assumptions that fall with it.

Row before, and after this job, are the same measurement — nothing was touched:

| | tier | cause | median | f32 bitwise | f64 bitwise | coords |
|---|---|---|---|---|---|---|
| before | `bitwise` | `rng` | 2.406e-1 | 0 | 0 | 1020 |
| after | `bitwise` | `rng` | 2.406e-1 | 0 | 0 | 1020 |

(`target/gates/scigraphs-conformance.json`, `.functions.FORCEATLAS2`. Ceiling 1e0, so the row
passes; the job's "disparity 0.241" is `procrustes_median` 0.24061707780369648.)

## Step 0, the proof

Three independent checks, all in `ge-python-oracle`, all re-runnable.

**1. What the tier flags resolve to in the image.** `ForceSim` is pure numpy inside the
submodule, so its import cannot fail for want of the compiled `scigraphs-utils` package:

```
FORCESIM_AVAILABLE = True
FORCESIM_REASON    = None
NX_FA2 is None     = False
FA2_AVAILABLE      = False
ForceSim source    = /w/SciGraphs/core/scigraphs_core/mesh/layouts/simulation.py
scigraphs_utils = ABSENT: ModuleNotFoundError No module named 'scigraphs_utils'
quadtree_repulsion -> None
```

`scigraphs-utils` is absent, but it is not what `FORCESIM_AVAILABLE` tests: `forceatlas.py:6-18`
imports `ForceSim`, `random_positions`, `quadtree_repulsion` from `.simulation`, and only
`quadtree_repulsion` is the wrapper that needs the extension (`simulation.py:53`, resolved lazily
inside `try/except` at `:50-56`). So `FORCESIM_REASON` is `None` with the package missing, and
`forceatlas.py:167` — the first branch, checked before `NX_FA2` at `:175` — always wins.

**2. What the reference arm actually runs.** `sc_reference.py:48,51` wraps `apply_graph_layout` in
`redirect_stdout(transcript)` and never reads the transcript, so the tier's own line is discarded
and nothing in `ref/FORCEATLAS2.json` records it. Running the same call with stdout left alone
says it outright:

```
Computing ForceAtlas2 (3D, ForceSim) for 77 nodes...
  Barnes-Hut not used below 400 nodes (n=77): the exact all-pairs repulsion is cheaper there, and exact
  Still moving 0.10k per node after 50 iterations, so this layout is not settled; raise Iterations
  ForceAtlas2 completed in 0.01s
```

That is `_forceatlas2_forcesim` (`forceatlas.py:124-147`). The networkx branch the whole job was
written around is dead code in this image.

**3. That ForceSim is not networkx FA2**, so the port cannot be reached from here:

| | reference (ForceSim) | `layout.forceatlas2` (networkx port) |
|---|---|---|
| start | `default_rng(1767573729).random((n,3))`, `(r - 0.5) * 5.0`, **f32** (`simulation.py:1090-1091`, `DTYPE = np.float32` at `:12`) | `Mulberry32` in the unit square, **f64** (`state.rs:258-266`) |
| generator | PCG64 | Mulberry32 |
| gravity | `gravity * 0.1` (`_FA2_GRAVITY_NORM`, `forceatlas.py:33,134`) | `gravity`, unscaled (`state.rs:194`) |
| step cap | every move clipped to `k = scale / cbrt(n)` (`simulation.py:376, 757-761`) | uncapped (`state.rs:243-248`) |
| arithmetic | f32 throughout (`DTYPE`) | f64 throughout |
| `dim` | 3 by default (`forceatlas.py:153`) | 2 (`state.rs:4`) |
| `distributed_action` | n/a (ForceSim's own model) | `False` (`state.rs:4`) |
| iterations | `sim.step(50)` (`forceatlas.py:141`) | `max_iter` |
| rescale | `_fa2_rescale` (`forceatlas.py:47-56`, called `:147`) | none |

Six of the nine differ by more than a seed. Matching this row is a ForceSim port — a different
algorithm with its own move cap and f32 state — not a seed fix.

## The body assumptions that fall with the premise

- **"The seed is `_get_layout_rng().randint(0, 2**31 - 1)`, the first draw of the per-call
  `RandomState(981798123)`"** — the derivation is **right**, and pinned here in the oracle:
  `get_layout_seed() = 981798123`, first draw = **1767573729** (`forceatlas.py:122`). But the body
  assumed it is then handed to `random_layout`; it is handed to `default_rng`.
- **"networkx then calls `random_layout(G, dim, seed=…)` … the start is f32-narrowed MT19937"** —
  false. ForceSim's start is `(default_rng(seed).random((n,3)) - 0.5) * 5.0` narrowed to f32
  (`simulation.py:1091`). f32 narrowing survives; MT19937 does not — it is **PCG64**. Verified:
  `type(np.random.default_rng(s).bit_generator).__name__ == 'PCG64'`. Measured first rows for
  `n = 77`, seed 1767573729, `scale = 5.0`, `dimensions = 3`:

  ```
  row0 = [1.4719001 , 0.8315206 , 0.57074285]   (f32)
  row1 = [-1.8054718,  1.767594 ,  1.5292593]   (f32)
  ```

  Nothing in graph-core emits PCG64; `rng.rs` carries `Mt19937` and `Mulberry32`.
- **"`dim=int(dim)`, default 3; build the 3-D solve the way the spring kernel did"** — the default
  of 3 is right (`forceatlas.py:153`), and ForceSim really does start `(n, 3)`, so a z column is
  right. But it buys nothing: the geometry `dim` is the least of the nine differences above, and
  adding `distributed_action` to `Fa2Params` would be a field ForceSim never reads.
- **"`weight = 'weight' if edge_weight_influence else None` … read `_build_networkx_graph` and say
  whether conformance edges carry a `weight` attribute"** — answered, and it is the one premise
  that holds: `_build_networkx_graph` inserts bare 2-tuples (`common.py:297` `G.add_edges_from(
  edge_indices)`, built at `:204`), and the fixture hands them over as index pairs
  (`sc_fixture.py:77-78`). So every weight defaults to 1 and ForceSim's `_fa2_edges` returns
  `weights = None` outright (`forceatlas.py:72`). No `Fa2Params` weight field is needed.
- **"Read `forceatlas2_layout` for the dtype of `pos_arr` after an in-place update … the motor
  must narrow where numpy narrows"** — traced for the record, since it is the one part of the body
  that survives as a fact about networkx: `random_layout` casts f32 (`layout.py:120-121`), so
  `pos_arr` is f32 (`:1697`) while every force is f64, and `pos_arr += factored_update`
  (`layout.py:1870-1871`) is an in-place same-kind cast — **f32 stays f32, the f64 increment is
  rounded down every iteration**. graph-core's f64 `Vec` (`state.rs:56-57`) does not reproduce
  that. It is a real divergence from the *networkx* port's reference, but networkx is not this
  row's reference, so it is out of scope here.
- **"`jiggle` for coincident nodes unless networkx does something else there"** — networkx has no
  such guard: the dense form only protects the diagonal (`layout.py:1828-1829` `fill_diagonal`), so
  an off-diagonal coincidence gives `tmp / 0 = +inf` and then `0 * inf = NaN` through `einsum`
  (`:1830-1831`). graph-core's `jiggle` (`state.rs:150`, `:164-172`) is a deliberate D9 guard and
  is kept; its doc comment already records that networkx has none.

## What is corrected

`G_FORCEATLAS2_SEED`'s note in `conformance/gaps.rs` said the reference "draws its start from
`np.random.RandomState(get_layout_seed())`". That is wrong twice — wrong generator and wrong
integer — so the note now states the measured chain. **The gap stays open**; `rows.rs:63` still
carries it, and closing it was conditional on a matching start.

`gaps.rs` also still lists `G_SNAPSHOT_SCALE` for this row, which is separate and untouched.

## Commands

```
scripts/orch/gr cargo build --release -p graph-cli                                -> 0
scripts/scigraphs-conformance.sh                                                  -> 0  (PASS, FORCEATLAS2 ok — 0 f64, 0 f32 of 1020, median 2.406e-1)
scripts/orch/gr cargo run -q --release -p graph-cli -- emit-fa2-fixtures --seeds 1000 -> 0
docker run … ge-python-oracle python3 harness/oracle-fa2.py target/fa2-fixtures   -> 0  (1000 cases, worst 3.156e-8)
scripts/orch/gr cargo run -q --release -p graph-cli -- oracle-fa2                  -> 0  (worst 3.156e-8, ceiling 1e-7: ok)
```

`hashgate --seeds 8` and `cargo test --workspace` were not run: no code changed under the merge
floor, and both are the orchestrator's to gate.

## Recommendation

Queue a **separate** job for a ForceSim port of FA2, and decide its scope first, because it is
larger than a port:

1. a PCG64 generator in `rng.rs` next to `Mt19937`, plus `default_rng`'s stream shape;
2. a ForceSim `model='FA2'` kernel — f32 state, `k = scale / cbrt(n)` move cap, gravity `× 0.1`,
   `sim.step(50)`;
3. `_fa2_rescale` (`forceatlas.py:47-56`) in the conformance arm.

Keeping `layout.forceatlas2` as the networkx port is still right — `oracle-fa2` gates it against
networkx 3.6 at worst 3.156e-8 over 1000 seeds. A new id is the honest shape for a ForceSim port,
or the conformance arm overrides the seed and start on the existing one if a third id is not
wanted. **Do not** re-point `layout.forceatlas2` at ForceSim: that would break its own oracle.