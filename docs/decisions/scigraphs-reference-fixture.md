# The SciGraphs reference fixture

`fixtures/scigraphs/lesmis.json` is the reference data the studio draws its
Les Miserables view from. It exists so the gallery figure
(`SciGraphs/docs/assets/gallery/fig6_labels_lesmiserables.png`) can be
reproduced **from SciGraphs' own numbers**, without Blender and without
re-deriving a layout the reference already defines.

## Why this fixture comes from Python, not from `graph-cli`

The project's rule is that `graph-cli` emits fixtures. This one is the
exception, and the exception is the point.

`graph-cli` can only emit what `graph-core` computes. Every other fixture here
is a *differential* artefact: the Rust motor produces a number and a second
implementation is compared against it. That works when the algorithm is the
thing under test. This fixture is the opposite — the algorithm is
**SciGraphs'**, already written, already reviewed, and unreachable from Rust
without re-implementing `nx.spring_layout`, the RANK colour normalisation, an
exact betweenness and a Blender camera projection. A Rust port of any of those
would be a *claim* about SciGraphs; running SciGraphs' own code is the reference
itself.

The cost is stated plainly: the file is produced by a Docker command, not by
`cargo run`. `harness/emit-scigraphs-lesmis.py` is the generator, and running
it twice gives byte-identical output — the same run-to-run property the hash
gate demands of the Rust side, checked here with `cmp` and by
`test_two_builds_are_byte_identical`. The numbers are Python's numbers, and the
JSON records the exact `networkx` and `numpy` versions that produced them under
`provenance`, so a fixture that stops matching is visibly a version problem
rather than a silent drift.

One consequence worth knowing: this file is **reference data, not a gate
input**. Nothing in `graph-cli` reads it, and it is not listed by the hash
gate. If the Rust motor ever has to match it, that is a new task with a new
gate, not something this file does silently.

## What is identical to the gallery

The parameters come from the gallery's own specification,
`SciGraphs/docs/examples/05-reproducible-pipeline.qmd:591-680`, read out of the
download link that page embeds. So these are SciGraphs' values, not ours:

| | |
| --- | --- |
| graph | the 77-node Les Miserables network, 254 edges |
| layout | `SPRING_3D`, 150 iterations, scale 5.0, dim 3, base seed 42 |
| colour transform | `RANK` over betweenness |
| camera | 60 mm lens, 36 mm sensor, margin 1.12, 1920×1080, perspective |
| radii | node 0.022 R, edge 0.0035 R |
| labels | rank by betweenness, occlusion on, declutter on, max 18, font 26 |

And these are SciGraphs' *functions*, imported rather than reimplemented
(`harness/scigraphs_lesmis_graph.py` names each one):

- the layout, via `core/scigraphs_core/mesh/layouts/networkx_layouts.py:26-34`
  — the same `nx.spring_layout` call the `SPRING_3D` branch reaches;
- the seed, via `core/scigraphs_core/repro/determinism.py:56-62` —
  `derive_seed(42, "layout")` = `981798123`;
- the colour coordinate, via
  `core/scigraphs_core/coloring/colormaps.py:462-469` and `:534-536`;
- the edge canonicalisation, ported from
  `SciGraphs/core/mesh/geometry.py:259-287` because it is eleven lines of numpy
  inside a `bpy` function.

The camera and the label pipeline are ports, not imports: they live in
`bpy`-dependent modules. `harness/scigraphs_lesmis_camera.py` follows
`SciGraphs/core/repro/executor.py:606-744` and
`SciGraphs/core/visualization/text_overlay.py:139-326` line for line, and
`harness/scigraphs_lesmis_fixture.py` follows
`SciGraphs/core/repro/executor.py:477-509` for the four label stages in order.

`test_camera_matches_an_independent_recomputation` recomputes the whole camera a
second time, in numpy, inside the test module. That is what makes the pinned
`repr` values a check rather than a snapshot.

## How the generator is split

`harness/emit-scigraphs-lesmis.py` is the command; the work is in three modules
beside it, because the house limits (≤300 lines a file, ≤40 lines a function,
≤4 parameters) will not fit in one script:

- `harness/scigraphs_lesmis_graph.py` — everything imported from SciGraphs:
  the graph, the layout, the seed, betweenness, the colour coordinate.
- `harness/scigraphs_lesmis_camera.py` — everything ported from `bpy`:
  the frame, the projection, the radii, the occlusion test.
- `harness/scigraphs_lesmis_fixture.py` — the four label stages and the JSON.

## What is not identical, and why

**The node order is networkx's, not the gallery's.** The gallery's dataset is
`//data/lesmiserables.gexf`, and that file is in neither tree. The spec says
`auto_layout: false` with a layout block, and the GEXF almost certainly carried
its own `viz:` coordinates — which `create_graph_object` would have used as the
layout's *starting positions*. Ours start from the seeded random positions
instead. Same algorithm, same seed, different starting point, therefore a
different point cloud. This is recorded as a Ponytail in the fixture's own
`provenance`, not buried here.

**Betweenness is networkx's, not igraph's.** `python-igraph` is not in the
oracle image, so `calculate_centrality` takes its documented fallback
(`analysis.py:149-153`) rather than `_igraph_betweenness` (`:62-76`). SciGraphs'
own comment puts the two within 7e-18 of each other on this graph, and the
normalisation is the same, so this is a difference in the last few digits, not
in the ordering. It is still a deviation and it is still in the Ponytail list.

**`R` is measured over the nodes, not over the evaluated mesh.**
`_graph_bounds` measures `evaluated.bound_box`, which also encloses the
instanced glyphs and the edge curves. Ours is the node point cloud's box, so our
`R` is smaller and our centre can sit up to one glyph radius away from theirs.
That moves every screen coordinate slightly.

**The occlusion raycast is against spheres, not the real geometry.** Blender
casts against icosphere facets and bezier edge curves. We cast against one sphere
of radius `0.022 R` per node, keeping the reference's ray origin offset
(`0.01`), its nearest-hit rule and its `max(0.1, 1.5 r)` tolerance. The
predicate is the reference's; only the occluder set is smaller, so we can only
under-count occlusions, never invent them.

**`to_track_quat` is rebuilt.** It is a `mathutils` binding with no pure-Python
equivalent in the tree. We rebuild the frame around world up `(0, 0, 1)`. The
degenerate case — forward parallel to world up — is taken to be the 180-degree
roll, which is what the reference tree's own comment says Blender produces
(`SciGraphs/api/render.py:677`).

## The label set, compared with the figure

The reference pipeline reports its own losses in the log; so does ours.

| stage | gallery | this fixture |
| --- | --- | --- |
| projected | 77 | 77 |
| inside the frame | 77 | 77 |
| not hidden behind geometry | 59 | 65 |
| kept | 18 | 18 |

Eighteen names are legible in the PNG. **Eight of them are in the fixture's
label set** — `Fauchelevent`, `Javert`, `Mabeuf`, `Marius`, `MmeBurgon`,
`MmePontmercy`, `Myriel`, `Napoleon`. Ten we label are not in the figure
(`Combeferre`, `Enjolras`, `Fantine`, `Gillenormand`, `Gueulemer`, `Joly`,
`Magnon`, `MlleGillenormand`, `Simplice`, `Tholomyes`) and ten the figure
labels are not in ours (`Anzelma`, `Bossuet`, `Champtercier`, `Cosette`,
`Geborand`, `Gervais`, `Labarre`, `Marguerite`, `MlleBaptistine`, `Valjean`).

**Nothing was tuned to close that gap.** Both sets are the honest output of the
same four stages on two different point clouds, and the divergence is the
consequence recorded above: a different layout means a different screen
projection, a different occlusion set, and a different greedy declutter result.
Forcing agreement would mean tuning the camera or the tolerance until the answer
looked right, which would make the fixture a description of the PNG rather than
of SciGraphs. The comparison is printed to **stderr** on every emit so it is
visible but never contaminates the JSON on stdout, and it is pinned in
`test_fig6_overlap_is_reported_not_forced`.

## Reproducing it

```sh
docker run --rm -v "$PWD:/w" \
  -v /sgoinfre/students/dlesieur/graph_render/SciGraphs:/sg:ro -w /w \
  ge-python-oracle python3 harness/emit-scigraphs-lesmis.py /sg \
  > fixtures/scigraphs/lesmis.json
```

The tests run in the same image:

```sh
docker run --rm -v "$PWD:/w" \
  -v /sgoinfre/students/dlesieur/graph_render/SciGraphs:/sg:ro -w /w \
  ge-python-oracle python3 -m unittest discover -s harness \
  -p "test_scigraphs_lesmis*.py"
```

`harness/test_scigraphs_lesmis.py` pins the graph, the layout, the metrics and
the camera; `harness/test_scigraphs_lesmis_document.py` pins the emitted bytes;
`harness/test_scigraphs_lesmis_branches.py` drives the branches the 77-node
graph never reaches — a direction parallel to world up, a portrait frame, a
node behind the camera, a glyph just inside and just outside its own radius, a
declutter rejection, a `max_count` truncation and a betweenness tie. The pinned
values live in `harness/scigraphs_lesmis_pins.py`, so all three read one
source.
