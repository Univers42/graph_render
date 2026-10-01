"""The metrics: how close two coordinate lists are, and in what sense.

Five numbers per fixture and six per row, each measuring something the others cannot:

- **bitwise f64 / f32** — how many coordinates are the *same value*, unshifted. The motor is
  `f32` end to end (`graph_contract::canonical_json.schema:110`), so its `f64` file is an `f32`
  widened, and `bitwise_f64 == N` is reachable only where the reference's own value happens to
  be `f32`-representable. That is why both counts are reported: the `f32` one is the strongest
  statement this tree can make, and the `f64` one says whether even that was exact.
- **max ULP** — the same question in units of the last place, so a 1e-9 gap in a coordinate of
  1e6 and in a coordinate of 1e-3 are not reported as one number.
- **max absolute gap** — the plain distance, which is the one a reader can picture.
- **Procrustes disparity** — the gap after removing a translation, a uniform scale and a
  rotation (`scipy.spatial.procrustes`). Two layouts can differ by a factor of a thousand and
  have disparity 0; that is the point. It cannot see a **reflection**, so it is reported twice:
  once against the motor as given, and once against the negated motor, and a row whose second
  number is much the smaller is mirrored rather than merely rotated.
- **the dimension it was computed in** — a Procrustes fit needs at least as many points as
  columns, so a two-node fixture has no 3D fit. It gets the `xy` one and says so; the cell is
  never blank.
"""

import math

import numpy as np
from scipy.linalg import orthogonal_procrustes
from scipy.spatial import procrustes

# `orthogonal_procrustes` is the public helper `procrustes` itself calls
# (`scipy/linalg/_procrustes.py`), imported directly because the overlay needs the rotation and
# the scale on their own: `procrustes` hands back the aligned array in its own normalised frame
# and not the transform.

from sc_fixture import as_points


def ulp_distance(ours, theirs):
    """The distance between two doubles in units in the last place.

    Monotone on the integer encoding, so it crosses zero the way the numbers do and it treats
    `+0.0` and `-0.0` as adjacent rather than as opposite ends of the range.
    """
    difference = abs(_ordinal(float(ours)) - _ordinal(float(theirs)))
    return int(difference)


def _ordinal(value):
    """A double's bits, read as a signed integer that **rises with the value**.

    This is the monotone encoding the ULP distance needs, and it deliberately maps `-0.0` onto
    `0.0` — they are adjacent reals and one ULP apart is not a fact worth reporting. It is
    therefore **not** the comparison for byte equality: [`_bits`] is, because `0.0` and
    `-0.0` are two different eight-byte sequences and this matrix counts bytes.
    """
    return _monotone(_bits(np.float64(value)))


def _bits(value):
    """A float's IEEE-754 encoding, as the unsigned integer the bytes spell."""
    width = np.dtype(type(value)).itemsize
    raw = int.from_bytes(np.asarray(value, dtype=np.float64 if width == 8 else np.float32)
                         .tobytes(), "little", signed=False)
    return raw & ((1 << (8 * width)) - 1)


def _monotone(bits):
    """An unsigned bit pattern as a signed integer that rises with the float it encodes."""
    signed = bits - (1 << 64) if bits >> 63 else bits
    return signed if signed >= 0 else -(bits & 0x7FFFFFFFFFFFFFFF)


def bit_counts(ours, theirs):
    """`(bitwise f64, bitwise f32)`: the same **bits**, and the same bits after narrowing.

    `==` would be wrong here, and wrong in the way that only ever flatters: `+0.0 == -0.0` in
    every language, and zero is a third of this matrix's coordinates. The comparison is over
    the IEEE-754 encodings, which is what "byte by byte" means and what the `.f64` files carry.

    The `f32` count is the motor's own width — the motor has no `f64` coordinate to compare —
    so it is "the reference, narrowed, equals what we emit", which is the strongest equality
    this tree can state.
    """
    equal64 = sum(1 for a, b in zip(ours, theirs) if _bits(np.float64(a)) == _bits(np.float64(b)))
    equal32 = sum(
        1 for a, b in zip(ours, theirs)
        if _bits(np.float32(a)) == _bits(np.float32(b))
    )
    return equal64, equal32


def absolute_gap(ours, theirs):
    """The largest `|ours - theirs|` over the coordinates, or 0.0 over none."""
    if not ours:
        return 0.0
    return max(abs(a - b) for a, b in zip(ours, theirs))


def _finite(points):
    """The points as an `(M, N)` array, or `None` when one is not finite.

    A reference that wrote a `NaN` has not answered, and a disparity computed over it would be
    a number about nothing; the caller turns `None` into a `not run: …` cell instead.
    """
    if not points:
        return None
    array = np.asarray(points, dtype=float)
    if array.ndim != 2 or array.shape[0] < array.shape[1] or not np.isfinite(array).all():
        return None
    return array


def apply_similarity(theirs, ours):
    """The motor's points in the reference's frame, or `None` when the fit is not defined.

    **One fit in the pipeline, and it is scipy's.** The rotation and the scale come from
    `scipy.linalg.orthogonal_procrustes` — the public helper behind `scipy.spatial.procrustes`
    — so the overlay the SVGs are drawn from and the disparity the matrix reports are the same
    fit by construction rather than by agreement.

    Two earlier shapes of this function were wrong in ways no PNG would have shown.
    `scipy.spatial.procrustes`'s second return value is the already-aligned array **in scipy's
    own centred-and-unit-RMS frame**, not in the reference's, so drawing it directly collapsed
    every overlay to a single dot. And re-deriving the fit from an SVD of one's own got the
    reflection case wrong — the textbook Procrustes drops the flipped singular value out of the
    scale and `orthogonal_procrustes` does not — which shifted every 3D overlay. Both are gone:
    there is one fit and it is scipy's.
    """
    left, right = _finite(theirs), _finite(ours)
    if left is None or right is None or left.shape != right.shape:
        return None
    if left.shape[0] <= left.shape[1]:
        # No more points than columns: a Procrustes fit is not defined, and pretending
        # otherwise would draw an overlay from an arbitrary rotation.
        return None
    source = left - left.mean(axis=0)
    target = right - right.mean(axis=0)
    first, second = _norm(source), _norm(target)
    if first == 0 or second == 0:
        return None
    rotation, scale = orthogonal_procrustes(source / first, target / second)
    if scale == 0:
        return None
    # `procrustes` has `source / first ≈ ((target / second) @ rotation.T) * scale`, so the
    # motor back in the reference's own units is `(second / (first * scale))` times the
    # rotation of its centred cloud.
    gain = second / (first * scale)
    return [tuple(left.mean(axis=0) + gain * (rotation @ point)) for point in target]


def _norm(array):
    """A cloud's Frobenius norm, or 1.0 for a cloud of zero extent (an all-`0.0` column)."""
    total = float((array ** 2).sum())
    return total ** 0.5 if total else 1.0


def disparity(theirs, ours):
    """`(disparity, reflected)`: after translation, uniform scale and rotation.

    `reflected` is the best fit over both signs of `ours`, so a drawing that is the reference
    mirrored rather than merely rotated reports a small number **and** `True`, which is the
    only place in this matrix that a reflection can be seen.
    """
    first = _procrustes(theirs, ours)
    # The reflection test is a negation of the whole array, not of each point: negating a
    # point's three coordinates independently would be a point reflection through the origin,
    # which Procrustes already absorbs into its translation.
    second = _procrustes(theirs, [(-x, -y, -z) for x, y, z in ours])
    if first is None:
        return None, False
    if second is None:
        return first, False
    return min(first, second), second < first


def _procrustes(theirs, ours):
    """One `scipy.spatial.procrustes` disparity, or `None` when the fit is not defined."""
    left, right = _finite(theirs), _finite(ours)
    if left is None or right is None or left.shape != right.shape:
        return None
    try:
        _, _, disparity_value = procrustes(left, right)
    except ValueError:
        return None
    if not math.isfinite(disparity_value):
        return None
    return float(disparity_value)


def fixture_metrics(theirs, ours):
    """One fixture's numbers, in the cells the matrix prints.

    Both arguments are **flat coordinate lists**, the shape the `.f64` files carry, because the
    point-wise comparisons are over coordinates; the Procrustes fit needs points and is given
    them through [`as_points`].
    """
    theirs, ours = list(theirs), list(ours)
    coordinates = min(len(theirs), len(ours))
    if coordinates == 0:
        return {"metrics": "not run: neither arm produced coordinates"}
    theirs, ours = theirs[:coordinates], ours[:coordinates]
    equal64, equal32 = bit_counts(ours, theirs)
    gap = absolute_gap(ours, theirs)
    ulps = [ulp_distance(a, b) for a, b in zip(ours, theirs)] or [0]
    points_theirs, points_ours = as_points(theirs), as_points(ours)
    span = disparity(points_theirs, points_ours)
    row = {
        "coordinates": coordinates,
        "bitwise_f64": equal64,
        "bitwise_f32": equal32,
        "max_ulp": max(ulps),
        "max_gap": gap,
        "procrustes": "not run: the fit is not defined for this fixture",
        "reflected": False,
        # Whether the overlay could be drawn at all, and the same fit's disparity beside it:
        # a row whose overlay is absent is a row whose picture is missing, and the matrix says
        # so here rather than leaving an empty cell for a reader to discover in the PNG.
        "overlay": apply_similarity(points_theirs, points_ours) is not None,
    }
    if span is None:
        return row
    value, mirrored = span
    row["procrustes"] = value
    row["reflected"] = mirrored
    return row


def aggregate(rows):
    """A row's numbers over its fixtures: totals for the counts, median and max for the rest.

    **The median, not the mean, for the disparity.** Twenty-four fixtures where one is a
    two-node graph and one is a disconnected gate model are not twenty-four samples of a
    common quantity; the median is the one summary that a single outlier cannot move, and the
    max is printed beside it so the tail is visible rather than summarised away.
    """
    measured = [row for row in rows if isinstance(row.get("procrustes"), float)]
    if not measured:
        return {"metrics": "not run: no fixture could be fitted"}
    gaps = sorted(row["procrustes"] for row in measured)
    middle = len(gaps) // 2
    median = gaps[middle] if len(gaps) % 2 else (gaps[middle - 1] + gaps[middle]) / 2
    return {
        "coordinates": sum(row["coordinates"] for row in rows),
        "fixtures": len(rows),
        "measured_fixtures": len(measured),
        "bitwise_f64": sum(row["bitwise_f64"] for row in rows),
        "bitwise_f32": sum(row["bitwise_f32"] for row in rows),
        "max_ulp": max((row["max_ulp"] for row in rows), default=0),
        "max_gap": max((row["max_gap"] for row in rows), default=0.0),
        "procrustes_median": median,
        "procrustes_max": max(gaps),
        "reflected_fixtures": sum(1 for row in measured if row["reflected"]),
        "overlays": sum(1 for row in measured if row.get("overlay")),
    }
