"""Differential of the three graph-free 3D placements — SPHERE, HELIX and CUBE — against
SciGraphs' own functions (SciGraphs/core/scigraphs_core/mesh/layouts/basic.py:22-103), run in
the ge-python-oracle image with the SciGraphs/ submodule mounted so the arm is the reference
function itself and not a restatement of it:

  graph-cli emit-basic-3d-fixtures --seeds 1000
  docker run --rm --user 0:0 -v $PWD:/w -w /w ge-python-oracle \
      python3 harness/oracle-basic-3d.py target/basic-3d-fixtures
  graph-cli oracle-basic-3d

ONE ARM FILE FOR THREE FUNCTIONS, because the three take the same two arguments and read no
graph at all: `_sphere_layout(num_nodes, scale)` (basic.py:22), `_helix_layout(num_nodes,
scale)` (:65) and `_cube_layout(num_nodes, scale)` (:83). Five near-identical arm files would
be the redundancy the library-first rule forbids, and a shared arm is what makes the
per-function `--function` selector below the honest place for the differences to live.

Each function is compared in its own way, and the difference is the whole point of the
selector:

  sphere  — coordinates, within a tolerance. Closed form, no RNG: `y = 1 - 2*(i+0.5)/n`,
            `radius = sqrt(1 - y*y)`, `theta = pi*(3 - sqrt(5))*i`. The `+0.5` band MIDPOINT
            and the golden angle are both load-bearing and both compared.
  helix   — coordinates, within the same tolerance, over node counts 1..=N inclusive so both
            sides of the `levels > 1` boundary are in the sweep: at n = 1 AND n = 2 the
            reference returns `t = 0.5` for every node, and that is the row a port gets
            wrong silently (basic.py:71-74).
  cube    — the corners EXACTLY and the interior STATISTICALLY, and the split is the point.
            The reference draws the interior from `_get_layout_rng()` (common.py:43-52), a
            module-level global numpy `RandomState`, so (a) no interior coordinate of ours
            can equal SciGraphs' for any seed, and (b) the reference's interior depends on
            every earlier layout in the process that drew from it, so there is no "the"
            interior to compare against. What IS compared exactly is the part that is
            actually determined: the eight corners in all eight slots (their ORDER is the
            layout — basic.py:91-94 is a literal array), the `min(n, 8)` split, the n == 1
            origin (basic.py:88-89), and the interior's distribution: uniform on
            `[-0.8*scale, 0.8*scale]` per axis, mean 0, variance `(1.6*scale)^2/12`, and
            strictly inside the shell.

**`worst` DOES NOT MEAN THE SAME THING IN ALL THREE ROWS**, and each row says which in a
`compared` field, because the judge's gate is `worst <= ceiling` and handing it a quantity
that is supposed to be large would gate the layout against its own correct behaviour. For
`cube` the gated `worst` is the CORNER gap; its interior gap is reported beside it as
`interior_gap_not_gated` and is expected to be large.

Metric: the largest absolute coordinate difference over the three axes, per function, for the
two that are coordinate comparisons; for `cube`, the worst corner gap (which must be 0 after
the f32 narrowing) plus the interior's per-axis mean and variance against the uniform's. The
result holds the worst per function and, beside it, how many seeds matched bit for bit after
the snapshot's own f32 narrowing, and graph-cli holds the ceiling.

Ponytail: the gate model is one connected random graph per seed of 2 to 601 nodes, and these
three read only its node COUNT, so the comparison sweeps node counts rather than graph
shapes — which is the right axis here, and it does mean the sweep never sees n = 0. The
empty case is held by graph-core's own tests. The reference prints no progress line (unlike
`oracle-circular-hierarchy.py`'s arm), so nothing is silenced.
"""
import hashlib
import json
import os
import sys

sys.path.insert(0, os.path.join("SciGraphs", "core"))
import numpy as np  # noqa: E402

from scigraphs_core.mesh.layouts.basic import (  # noqa: E402
    _cube_layout,
    _helix_layout,
    _sphere_layout,
)

directory = sys.argv[1]
name = "basic-3d"
path = os.path.join(directory, f"{name}.jsonl")
manifest = json.load(open(os.path.join(directory, f"{name}-manifest.json")))
digest = hashlib.sha256(open(path, "rb").read()).hexdigest()
if digest != manifest["sha256"][f"{name}.jsonl"]:
    sys.exit(f"{name}.jsonl does not match its manifest")

# The functions this arm covers, and the fixture key each is emitted under. The list is the
# arm's own table, and it is what `--function` on the graph-cli side selects against: one
# arm, three functions, one place the differences between them are written down.
ARMS = {
    "sphere": _sphere_layout,
    "helix": _helix_layout,
    "cube": _cube_layout,
}

# The eight corners of a unit cube of half-side 1, in SciGraphs' literal order
# (basic.py:91-94). Transcribed here rather than derived, for the same reason the port
# transcribes it: the ORDER decides which node sits at which corner, so a permutation is a
# regression rather than a refactor, and only a transcription can hold it.
CORNERS = np.array([
    [1.0, 1.0, 1.0], [1.0, -1.0, -1.0], [-1.0, 1.0, -1.0], [-1.0, -1.0, 1.0],
    [-1.0, -1.0, -1.0], [-1.0, 1.0, 1.0], [1.0, -1.0, 1.0], [1.0, 1.0, -1.0],
])


def theirs_of(function, n, scale):
    """The reference's own answer for `function` at (n, scale)."""
    return np.asarray(function(n, scale), dtype=float)


cases = 0
worst = {key: 0.0 for key in ARMS}
worst_seed = {key: None for key in ARMS}
exact = {key: 0 for key in ARMS}
# cube only: the worst corner gap and the interior's distribution against uniform's.
worst_corner = 0.0
corner_exact = 0
interior_mean = 0.0
interior_variance = 0.0

for text in open(path):
    case = json.loads(text)
    n, scale = case["n"], case["scale"]
    for key, function in ARMS.items():
        ours = np.column_stack(
            [case[key]["x"], case[key]["y"], case[key]["z"]]
        ).astype(float)
        theirs = theirs_of(function, n, scale)
        if ours.shape != theirs.shape:
            sys.exit(
                f"{key} seed {case['seed']}: shape {ours.shape} vs {theirs.shape}"
            )
        gap = float(np.abs(ours - theirs).max())
        if gap > worst[key]:
            worst[key], worst_seed[key] = gap, case["seed"]
        # Bit-for-bit after the snapshot's own f32 narrowing: ours arrives f32, so both
        # arms are narrowed. np.cos and libm::cos differ in the last bits, so this is a
        # count and not a gate — the gate is the tolerance above.
        if np.array_equal(ours.astype(np.float32), theirs.astype(np.float32)):
            exact[key] += 1
        if key == "cube":
            # The corners, exactly: min(n, 8) nodes, in the reference's order.
            held = min(n, 8)
            if held:
                gap_corner = float(
                    np.abs(ours[:held] - CORNERS[:held] * scale).max()
                )
                worst_corner = max(worst_corner, gap_corner)
                if gap_corner == 0.0:
                    corner_exact += 1
            # The interior, statistically: the only claim about it that both streams can
            # satisfy, since ours is not the reference's stream.
            interior = ours[held:]
            if interior.size:
                interior_mean = max(
                    interior_mean, float(np.abs(interior.mean(axis=0)).max())
                )
                # uniform(-r, r) has variance r^2/3, and r = scale*0.8.
                expected = (scale * 0.8) ** 2 / 3.0
                interior_variance = max(
                    interior_variance,
                    float(np.abs(interior.var(axis=0) / expected - 1.0).max()),
                )
    cases += 1

if cases == 0:
    sys.exit("no cases in the fixtures: a differential over nothing proves nothing")

layouts = {
    key: {
        "cases": cases,
        "worst": worst[key],
        "worst_seed": worst_seed[key],
        "f32_bit_exact": exact[key],
    }
    for key in sorted(ARMS)
}
# What each row's `worst` MEANS, because it is not the same quantity for all three and a
# reader who assumes it is will misread the table. The judge's gate is `worst <= ceiling`,
# so a row whose `worst` were its interior gap would be gated against a number that is
# supposed to be large — the gate has to be handed the quantity that IS compared.
layouts["sphere"]["compared"] = "all three columns, every node"
layouts["helix"]["compared"] = "all three columns, every node"
# cube's gate is its EIGHT CORNERS, which is exact (the reference's literal array times
# scale), and its `worst` is therefore the corner gap rather than the interior's. The
# interior's own gap is reported beside it and is NOT gated — it is supposed to be large,
# for the two reasons in this file's docstring.
layouts["cube"]["worst"] = worst_corner
layouts["cube"]["worst_seed"] = None
layouts["cube"]["compared"] = "the min(n, 8) corner nodes only; the interior is not compared"
layouts["cube"]["corners_exact"] = corner_exact
layouts["cube"]["interior_gap_not_gated"] = worst["cube"]
layouts["cube"]["interior_f32_bit_exact"] = exact["cube"]
layouts["cube"]["interior_mean_abs"] = interior_mean
layouts["cube"]["interior_variance_ratio"] = interior_variance
result = {
    "fingerprint": manifest["fingerprint"],
    "sha256": digest,
    "oracle": f"SciGraphs _sphere_layout / _helix_layout / _cube_layout on numpy "
    f"{np.__version__}",
    "layouts": layouts,
}
json.dump(result, open(os.path.join(directory, f"{name}-result.json"), "w"), indent=1)
print(json.dumps(layouts))