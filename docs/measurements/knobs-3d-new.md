# knobs-3d-new: per-stage hash-gate controls for the two newest 3D layouts

Folded into `sg-spiral3d` by the orchestrator, whose review (`docs/reviews/rv-sg-spiral3d.md`,
finding **S3-1**, MAJOR) found that `layout.basic3d.spiral` and `layout.bipartite_3d` were both
hash-gate stages with **no per-stage negative control of their own**.

The hole is structural and worth stating plainly. `hashgate` compares the four arms of one run
against each other, so `4-way equal` proves the native and wasm32 builds agree — it does **not**
prove the stage is reading anything. A layout whose bytes went constant or empty (an `interp`
returning `0.0`, a `columns` returning a zeroed `Vec`) would still print `4-way equal on 8/8
seeds`. `AGENTS.md` asks for a negative control on every gate row; nothing was asking the same
question of the *tables*, so a layout could be registered without one and no test would notice.

`prompts/jobs/knobs-3d-new.md` is the fix. Its precondition was "sg-spiral3d and sg-bipartite3d
are both on develop"; the orchestrator retired that job and replaced the precondition with "this
branch", which is where the work happened.

## The coverage test, and why it is RED first

`crates/graph-cli/src/hashgate/tests/knob/coverage.rs` lists every `registry::LAYOUTS` id that
`knobs::all()` does not tabulate and compares it with an explicit allow list in the test. Before
this job's step 1, and after the 24 pre-existing gaps were measured into the allow list, it
failed naming **exactly the two ids this job is about**:

```text
---- hashgate::tests::knob::coverage::every_registered_layout_has_a_per_stage_control_or_a_reason_it_does_not stdout ----

thread '...' panicked at crates/graph-cli/src/hashgate/tests/knob/coverage.rs:86:5:
2 registered layout(s) have no per-stage negative control and no allow-list entry:
["layout.basic3d.spiral", "layout.bipartite_3d"]. A layout the gate hashes but cannot move is
a stage whose bytes could go constant and still read `4-way equal`. Either add it to
hashgate::knobs (the family table beside THREE_D_LAYOUT_STAGES), or add it to
NO_PER_STAGE_CONTROL above with the kind of gap it is.

test result: FAILED. 2 passed; 1 failed; 0 measured; 329 filtered out
```

### The allow list is measured, and it has two kinds of entry

An earlier draft of this test derived each control's variable name from its id
(`layout.x.y` → `GM_MUTATE_X_Y_NODES`). **That derivation is wrong**, and building the list is
what caught it — two of the twenty-four do not follow it:

| id | variable | why the derived name would have failed |
|---|---|---|
| `layout.circular.radial` | `GM_MUTATE_CIRCULAR_NODES` | spelled for the family, not the id |
| `layout.twopi` | `GM_MUTATE_TWOPI_NODES` | two O's, because the stage is `twopi` |

So the test carries an explicit `VARIABLE_FOR` table and runs each variable through the gate's
own `setting()` parse, asserting it comes back naming **that** stage. An entry therefore cannot
claim a control that does not exist, nor one that moves some other stage.

The list holds **24 entries in two kinds**, and the difference is load-bearing:

- **`Gap::HasOwnStageNodes` (6)** — reached by a `Knob` variant that sets
  `Setting::stage_nodes` for that stage alone (`knob/setting.rs:151-195`): `tree.tidy`,
  `treemap.squarified`, `treemap.patchwork`, `twopi`, `circular.radial`,
  `circular.hierarchy`. These are **real per-stage controls** that predate `knobs::all()` and
  live in the `Knob` enum rather than the stage tables, so reading `knobs::all()` alone would
  wrongly list them as gaps.
- **`Gap::NoControl` (18)** — nothing scopes a perturbation to that stage. The shared
  `GM_MUTATE_NODE_COUNT` and `GM_MUTATE_REFERENCE_DEGREE` move it, but they move **every** stage,
  so they prove nothing about this one. **These 18 are pre-existing debt, untouched by this job**,
  which added knobs for the two ids it names and opened no new holes.

A third test holds the list honest in the other direction: an id that *gains* a control must
**lose** its exemption, or the list becomes a place a working knob hides behind an excuse.

## The two knobs

Both are the re-drawn-model probe the other 3D rows use, and both are additive:

| stage | variable | record |
|---|---|---|
| `layout.basic3d.spiral` | `GM_MUTATE_BASIC3D_SPIRAL_NODES` | `hashgate-control-basic3d-spiral-nodes` |
| `layout.bipartite_3d` | `GM_MUTATE_BIPARTITE_3D_NODES` | `hashgate-control-bipartite-3d-nodes` |

Files, all additive: `hashgate/knobs.rs` (`THREE_D_LAYOUT_STAGES` `[Stage; 5]` → `[Stage; 7]`),
`hashgate/knob/three_d.rs` (`ENV` and `RECORD` `[&str; 5]` → `[&str; 7]`),
`hashgate/knob.rs` (two `Knob` variants, `ALL` 45 → 47), `hashgate/knob/arms.rs` (`ALL` and
`env()`), `hashgate/knob/records.rs` (two arms), `crates/graph-cli/tests/common/mod.rs`
(`KNOBS` 45 → 47), `hashgate/tests/knob/three_d.rs` (`NODES` and `STAGES` 5 → 7), and the new
`hashgate/tests/knob/coverage.rs`.

**Why a node count is the right probe for both.** `layout.basic3d.spiral` reads the node count
and no edge, so its model is its entire input — and a sharper probe than it is for `sphere`,
because the 65 536-entry arc-length table is rebuilt per call, so a perturbed node count re-runs
the whole inversion. `layout.bipartite_3d` **does** read the graph, so a re-drawn model moves it
through its edges as well; that is still the right probe, because it is scoped to this stage
alone, which a shared control could not be.

## Proof: each knob turns the gate red, and only for its own stage

```text
$ scripts/orch/gr -e GM_MUTATE_BASIC3D_SPIRAL_NODES=1 cargo run -q --release -p graph-cli -- hashgate --seeds 8
  DIVERGED layout.basic3d.spiral 0:
    native run 1  layout.basic3d.spiral 0 9552ff93aa7a6b12c37aae97c52b05afc4c0e6f7345a10b2384c4f3469fc22bd
    native run 2  layout.basic3d.spiral 0 9552ff93aa7a6b12c37aae97c52b05afc4c0e6f7345a10b2384c4f3469fc22bd
    wasm32 run 1  layout.basic3d.spiral 0 3aa5c75139dc7d998104bc70ff548e5b64583a0bd0db843a53beb7953b968022
    wasm32 run 2  layout.basic3d.spiral 0 3aa5c75139dc7d998104bc70ff548e5b64583a0bd0db843a53beb7953b968022
-> exit 1
```

```text
$ scripts/orch/gr -e GM_MUTATE_BIPARTITE_3D_NODES=1 cargo run -q --release -p graph-cli -- hashgate --seeds 8
  DIVERGED layout.bipartite_3d 0:
    native run 1  layout.bipartite_3d 0 7c17cab35290cc1b5bc114891a185036ed63fe892937bbab384f504596f34de3
    native run 2  layout.bipartite_3d 0 7c17cab35290cc1b5bc114891a185036ed63fe892937bbab384f504596f34de3
    wasm32 run 1  layout.bipartite_3d 0 7a3c229ab5d1fa79f862b492fefeb8ad9150d77a8fa4fb1bf10b4d352f2880fc
    wasm32 run 2  layout.bipartite_3d 0 7a3c229ab5d1fa79f862b492fefeb8ad9150d77a8fa4fb1bf10b4d352f2880fc
-> exit 1
```

Each names **one** stage and no other, which is the property a per-stage control exists for:
the divergence is attributable. Note both are native-vs-wasm32 divergences of the *perturbed*
stage — that is the gate correctly reporting that a mutated stage does not agree across targets,
which is what makes it a control rather than a no-op.

Without either variable the gate is green, and **the stages' digests are unchanged** by this
job — a knob adds a control, it does not touch a layout:

```text
$ scripts/orch/gr cargo run -q --release -p graph-cli -- hashgate --seeds 8
  layout.basic3d.spiral: 4-way equal on 8/8 seeds
  layout.bipartite_3d: 4-way equal on 8/8 seeds
  4-way equal on 8/8 seeds
PASS
-> exit 0
```

Per-stage `equal` counts read from `target/gates/hashgate.json`, before and after the wiring:

| stage | before | after |
|---|---|---|
| `layout.basic3d.spiral` | 8/8 | 8/8 |
| `layout.bipartite_3d` | 8/8 | 8/8 |

55 stages in the record, `pass: true`, both before and after. `KNOB: ALL` went 45 → 47 and
`THREE_D_LAYOUT_STAGES` 5 → 7; no stage's bytes moved, because no layout's numeric code is in
the diff.

## The shared control is not a substitute, and this is why

`GM_MUTATE_REFERENCE_DEGREE=9` exits 1 with `FAIL: 8 of 8 seeds diverge`, and it is the right
control for "do the seeds bite at all". It is **not** a per-stage control: it perturbs the shared
reference model, so it moves all 55 stages at once and names none of them. Before this job,
`layout.basic3d.spiral` and `layout.bipartite_3d` had nothing else — which is exactly the gap
`coverage.rs` now makes impossible to reopen silently.

## Commands and exit codes

```text
scripts/orch/gr cargo test -p graph-cli --bin graph-cli coverage   (RED, before step 1)
    -> 1, names ["layout.basic3d.spiral", "layout.bipartite_3d"]
scripts/orch/gr cargo test -p graph-cli --bin graph-cli coverage   (GREEN, after step 1)
    -> part of the 332-passed run below
scripts/orch/gr cargo test -p graph-cli --bin graph-cli
    -> 0, 332 passed, 0 failed
scripts/orch/gr cargo run -q --release -p graph-cli -- hashgate --seeds 8
    -> 0, PASS, layout.basic3d.spiral 4-way equal on 8/8 seeds
scripts/orch/gr -e GM_MUTATE_BASIC3D_SPIRAL_NODES=1 ... hashgate --seeds 8
    -> 1, DIVERGED layout.basic3d.spiral
scripts/orch/gr -e GM_MUTATE_BIPARTITE_3D_NODES=1 ... hashgate --seeds 8
    -> 1, DIVERGED layout.bipartite_3d
scripts/orch/gr -e GM_MUTATE_REFERENCE_DEGREE=9 ... hashgate --seeds 8
    -> 1, FAIL: 8 of 8 seeds diverge
```

## Left behind

**The 18 `Gap::NoControl` ids are still un-controlled**, and that is pre-existing debt this job
did not take on: `bipartite`, `circular.circo`, `circular.ring`, `dag.sugiyama`,
`force.barnes_hut`, `force.fdp`, `force.neato`, `force.sfdp`, `force.spring`, `force.yifan_hu`,
`forceatlas2`, `forceatlas2.barnes_hut`, `grid`, `mds.pivot`, `packing.circle`, `random`,
`spectral`, `spiral` (the planar one). They are now **enumerated and reasoned in the test**
rather than invisible, which is the state the coverage test can hold and a later job can work
from. `layout.force.spring` is the interesting one: `Knob::SpringIterations` moves it, but it
moves `layout.force.spring3d` too, so neither knob names it alone — it wants a *node* control
like the two added here.