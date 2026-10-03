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
            layout — basic.py:91-94 is a literal array), the `min(n, 8)` split, and the n == 1
            origin (basic.py:88-89), which is compared on its own branch because the
            reference returns the ORIGIN for a one-node graph and not `CORNERS[0] * scale`.
            The interior is MEASURED, not gated: `interior_mean_abs` and
            `interior_variance_ratio` are recorded beside the gated `worst` and NOTHING
            reads them — `judge()` in `crates/graph-cli/src/oracle_python.rs:238-243` reads
            only `cases` and `worst`, and the gate would have to be a new
            `(cube::ID, "interior_variance_ratio", ...)` entry in the `ceilings` slice at
            `crates/graph-cli/src/oracle_python/basic_3d.rs:34-41` before either number
            decided a row. The distribution this arm holds them to is uniform on
            `[-0.8*scale, 0.8*scale]` per axis, mean 0, variance `(1.6*scale)^2/12`.

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
`oracle-circular-hierarchy.py`'s arm), so nothing is silenced. The refusals this arm owes
its result — the digest, the seed count, a layout that compared no case, and a non-finite
gap — come from `harness/oracle_common.py` rather than from a fourth copy of them.
"""
import json
import os
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import numpy as np  # noqa: E402

from oracle_common import (  # noqa: E402
    finite,
    read_manifest,
    require_cases,
    require_seeds,
)

sys.path.insert(0, os.path.join("SciGraphs", "core"))
from scigraphs_core.mesh.layouts.basic import (  # noqa: E402
    _cube_layout,
    _helix_layout,
    _sphere_layout,
)

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

# What a one-node cube is: the ORIGIN (basic.py:86-89 returns `np.zeros((1, 3))` before the
# corner array is built at all), and so not `CORNERS[0] * scale` = (5, 5, 5).
ORIGIN = np.zeros((1, 3))


def theirs_of(function, n, scale):
    """The reference's own answer for `function` at (n, scale)."""
    return np.asarray(function(n, scale), dtype=float)


def keep_worst(worst, worst_seed, gap, seed):
    """`(worst, worst_seed)` with `gap` folded in, refusing a non-finite one. `max` and `>`
    are both false for a NaN, so without this a NaN coordinate leaves `worst` at its 0.0
    initialiser and the case reports as a perfect match."""
    if finite(gap, "gap") > worst:
        return gap, seed
    return worst, worst_seed


def fresh_tally():
    """Every accumulator at its initialiser, so a fold that never runs cannot report 0.0."""
    tally = {key: {"worst": 0.0, "worst_seed": None, "exact": 0} for key in sorted(ARMS)}
    tally["cube"].update(
        worst_corner=0.0,
        corner_exact=0,
        interior_mean=0.0,
        interior_variance=0.0,
    )
    return tally


def fold_cube(ours, case, tally):
    """One seed into `cube`'s two halves: the corners exactly, the interior statistically."""
    n, scale = case["n"], case["scale"]
    # n == 1 is the reference's ORIGIN and nothing else (basic.py:88-89), so it is compared
    # on its own branch and the `min(n, 8)` corner path below skips it entirely.
    if n == 1:
        held, want = 1, ORIGIN
    else:
        held = min(n, 8)
        want = CORNERS[:held] * scale
    if held:
        gap_corner = float(np.abs(ours[:held] - want).max())
        tally["worst_corner"], _ = keep_worst(
            tally["worst_corner"], None, gap_corner, case["seed"]
        )
        if gap_corner == 0.0:
            tally["corner_exact"] += 1
    # The interior, statistically: the only claim about it that both streams can satisfy,
    # since ours is not the reference's stream. Recorded, never gated — see the docstring.
    interior = ours[held:]
    if interior.size:
        tally["interior_mean"] = max(
            tally["interior_mean"],
            finite(float(np.abs(interior.mean(axis=0)).max()), "interior mean"),
        )
        # uniform(-r, r) has variance r^2/3, and r = scale*0.8.
        expected = (scale * 0.8) ** 2 / 3.0
        tally["interior_variance"] = max(
            tally["interior_variance"],
            finite(
                float(np.abs(interior.var(axis=0) / expected - 1.0).max()),
                "interior variance",
            ),
        )


def fold(case, tally):
    """One seed's row for every function in `ARMS`, in the arm's own order."""
    n, scale = case["n"], case["scale"]
    for key, function in sorted(ARMS.items()):
        ours = np.column_stack(
            [case[key]["x"], case[key]["y"], case[key]["z"]]
        ).astype(float)
        theirs = theirs_of(function, n, scale)
        if ours.shape != theirs.shape:
            sys.exit(
                f"{key} seed {case['seed']}: shape {ours.shape} vs {theirs.shape}"
            )
        gap = float(np.abs(ours - theirs).max())
        tally[key]["worst"], tally[key]["worst_seed"] = keep_worst(
            tally[key]["worst"], tally[key]["worst_seed"], gap, case["seed"]
        )
        # Bit-for-bit after the snapshot's own f32 narrowing: ours arrives f32, so both
        # arms are narrowed. np.cos and libm::cos differ in the last bits, so this is a
        # count and not a gate — the gate is the tolerance above.
        if np.array_equal(ours.astype(np.float32), theirs.astype(np.float32)):
            tally[key]["exact"] += 1
        if key == "cube":
            fold_cube(ours, case, tally["cube"])


def rows_of(tally, cases):
    """The result's `layouts` table, and what each row's `worst` MEANS — it is not the same
    quantity for all three, and a reader who assumes it is will misread the table."""
    layouts = {
        key: {
            "cases": cases,
            "worst": tally[key]["worst"],
            "worst_seed": tally[key]["worst_seed"],
            "f32_bit_exact": tally[key]["exact"],
        }
        for key in sorted(ARMS)
    }
    layouts["sphere"]["compared"] = "all three columns, every node"
    layouts["helix"]["compared"] = "all three columns, every node"
    # cube's gate is its EIGHT CORNERS, which is exact (the reference's literal array times
    # scale, and its origin at n == 1), and its `worst` is therefore the corner gap rather
    # than the interior's. The interior's own gap is reported beside it and is NOT gated — it
    # is supposed to be large, for the two reasons in this file's docstring.
    layouts["cube"].update({
        "worst": tally["cube"]["worst_corner"],
        "worst_seed": None,
        "compared": (
            "the min(n, 8) corner nodes only, and the origin at n == 1; "
            "the interior is not compared"
        ),
        "corners_exact": tally["cube"]["corner_exact"],
        "interior_gap_not_gated": tally["cube"]["worst"],
        "interior_f32_bit_exact": tally["cube"]["exact"],
    })
    # Recorded, NOT gated: `judge()` reads only `cases` and `worst`, so neither of these
    # decides a row today. The field that would have to read them is a new `ceilings` entry
    # in `crates/graph-cli/src/oracle_python/basic_3d.rs:34-41`; the keys stay as they are
    # because the Rust side and docs/measurements cite them.
    layouts["cube"]["interior_mean_abs"] = tally["cube"]["interior_mean"]
    layouts["cube"]["interior_variance_ratio"] = tally["cube"]["interior_variance"]
    return layouts


def main():
    directory = sys.argv[1]
    name = "basic-3d"
    manifest, digest = read_manifest(directory, name)
    with open(os.path.join(directory, f"{name}.jsonl")) as handle:
        lines = handle.readlines()
    require_seeds(manifest, lines, name)

    tally = fresh_tally()
    for text in lines:
        fold(json.loads(text), tally)

    layouts = rows_of(tally, len(lines))
    require_cases(layouts, sorted(ARMS), name)
    result = {
        "fingerprint": manifest["fingerprint"],
        "sha256": digest,
        "oracle": f"SciGraphs _sphere_layout / _helix_layout / _cube_layout on numpy "
        f"{np.__version__}",
        "layouts": layouts,
    }
    with open(os.path.join(directory, f"{name}-result.json"), "w") as out:
        json.dump(result, out, indent=1)
    print(json.dumps(layouts))


main()