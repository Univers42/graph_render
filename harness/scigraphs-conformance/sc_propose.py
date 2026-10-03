"""The tier rule and the first-cause classifier, as code rather than as a claim.

`docs/measurements/scigraphs-coverage.md` has a tier per row, but it tiers a *tolerance*: a
differential that passed because two arms agree to `1e-6`. This file tiers something else — a
**row**, where the question is how close the two coordinate lists are in bytes and what stops
them being closer. So both the tier and the cause here are computed from the measured numbers
and the row's own declared gaps, and the doc reads them off rather than asserting them.

**The tier rule, in one line each.**

- `bitwise` — byte equality is reachable **without changing graph-core's algorithm**: a closed
  form, a scale or centring convention that can be undone in one place, or an RNG that can be
  ported away. This is the tier a repair job can move a row *up* to.
- `tolerance` — both arms are deterministic and run the same method, and the drawings agree in
  shape to a few parts per million; only the reduction order and the libm differ. Byte
  equality is reachable, but not at a sane cost: it is an f64 kernel, and the motor's columns
  are `f32`.
- `shape` — the two arms do not start from the same place, or do not run the same method, or
  there is no reference to compare against. Only the picture is comparable, and the picture is
  what the SVG panels are for.

**The cause rule, first cause wins, in this order.** `reference-absent`, then `arithmetic`,
then `convention`, then `rng`, then `algorithm`. The order matters and is the point of "first
cause": a row can have three of them, and the repair that matters is always the one that would
unlock the rest, which is the one nearest the top of this list.
"""

#: Below this disparity the two drawings are the same shape to within `scipy`'s own fit **and**
#: their raw coordinates differ by less than [`ARITHMETIC_GAP`] on all but
#: [`ARITHMETIC_RESIDUAL_FIXTURES`] of the row's fixtures, so there is nothing left over: the
#: same method, a different summation order or a different `libm`, and nothing but that.
#:
#: **Both halves are needed, and the gap is the half that matters.** `GRID` has a disparity of
#: 5e-32 and an absolute gap of 4.0: the same grid, in a different unit and a different axis
#: order. Calling that `arithmetic` because the shape is exact would send a repair job to chase
#: a summation order that is already right.
ARITHMETIC_DISPARITY = 1e-3
ARITHMETIC_GAP = 1e-6

#: **How many fixtures may carry a gap above [`ARITHMETIC_GAP`]**, which is the smallest rule
#: that separates the three measured cases from each other. `SPRING` is `f32`-identical on 21 of
#: its 24 fixtures and off by 2.37e-03 on `lesmis` alone; one fixture may carry the residual
#: because a reduction difference is a fixed thing and the fixture that shows it is the one with
#: the most nodes and the most chaotic iterations. Two may not: a second one makes the
#: difference systematic rather than amplified, which is exactly the measured shape of a unit or
#: an axis mismatch — `GRAPHVIZ_TWOPI` measures the whole 90.0-unit step on all 24, and `GRID`
#: measured 4.0 on all 24 before `sg-grid-scale` fixed its scale.
#:
#: **A fraction would separate nothing here.** Every row of this matrix carries its residual on
#: 24 of 24 fixtures or on 1 of 24; there is no row between, so `1` and "one half" draw the same
#: line on this data, and the count is the one that names what it counts.
#:
#: Caveat: a row whose residual is spread thinly over several *large* fixtures — say 4.0 on six
#: of twenty-four and 0.0 on the rest — reads as `convention` here. Such a row has not been
#: measured; if one appears, the number to raise is this one, and the direction is up.
ARITHMETIC_RESIDUAL_FIXTURES = 1

#: Below this disparity the drawings are the same shape up to a similarity, so what is left is
#: something a similarity cannot fix: the units, the centring, the axis order, the `z`.
#:
#: **1e-6, and it is measured rather than chosen.** Every row this classifies as `convention`
#: measures between 5e-32 (`GRID`) and 4e-10 (`GRAPHVIZ_PATCHWORK`); every row that measures
#: 0.08 to 0.7 does not, and its overlay says the same thing. An earlier 0.35 threshold here
#: called `SPECTRAL_3D` (0.333), `MDS_3D` (0.078), `GRAPHVIZ_CIRCO` (0.284) and
#: `CIRCLE_PACKING` (0.517 on lesmis) conventions, and every one of those four is a different
#: shape in the picture — the threshold was wrong, not the data.
CONVENTION_DISPARITY = 1e-6

#: The two fixtures the shape verdicts are read from. **They are in the discriminator, not
#: beside it**, because the median over 24 fixtures is dominated by the twenty that hold 2 to
#: 21 nodes: `CIRCLE_PACKING` measures 5e-16 on those and 0.517 on `lesmis`, and a cause
#: decided on the median alone is a cause about a graph nobody looks at.
NAMED_FIXTURES = ("lesmis", "tree-balanced")

TIERS = ("bitwise", "tolerance", "shape")
CAUSES = ("convention", "rng", "arithmetic", "algorithm", "reference-absent")


def classify(entry, row):
    """`(tier, cause, why)` for one row, from its measured numbers and its declared gaps.

    `entry` is one row of `metrics.json`; `row` is that row's `motor.jsonl` line, which is
    where the convention gaps live. A row that could not be measured has no numbers, and its
    cause is its own absence.
    """
    if "metrics" in entry:
        return "shape", "reference-absent", entry["metrics"]
    if row["motor"].startswith("not run"):
        return "shape", "algorithm", "not run: %s" % row["motor"]
    agrees = entry["bitwise_f64"] == entry["coordinates"] and entry["coordinates"] > 0
    if agrees:
        return "bitwise", "arithmetic", "the two coordinate lists are identical"
    worst = _worst_disparity(entry)
    if worst <= ARITHMETIC_DISPARITY and _same_method(entry, row):
        return "tolerance", "arithmetic", None
    # **`rng` before `convention`, and that order is the argument.** A row whose two arms
    # cannot start from the same position would still disagree after the scale and the
    # centring were undone, so the seed is the cause a repair has to clear first. A row with no
    # seed gap and an exact shape is a pure convention: undoing it makes the bytes agree.
    if _has_seed_gap(row):
        return "bitwise", "rng", None
    if worst <= CONVENTION_DISPARITY:
        return "bitwise", "convention", None
    return "shape", "algorithm", None


def _worst_disparity(entry):
    """The largest disparity over the median **and** the two named fixtures.

    A row that agrees to a part in a million on one graph and not at all on another is a row
    whose cause must be decided by the worse of the two, because one repair has to cover both.
    Taking only the median is how `CIRCLE_PACKING` came to be called a convention.
    """
    values = [entry.get("procrustes_median", 1.0)]
    per_fixture = {row["fixture"]: row for row in entry.get("per_fixture", [])
                   if "fixture" in row}
    for name in NAMED_FIXTURES:
        measured = per_fixture.get(name, {}).get("procrustes")
        if isinstance(measured, float):
            values.append(measured)
    return max(values)


def _same_method(entry, row):
    """Whether what is left over is one method's own rounding, rather than a unit or an axis.

    Two ways to answer yes. The narrow one, unchanged and still enough on its own: the largest
    gap over the row is within [`ARITHMETIC_GAP`]. The widened one: the shape agrees and **at
    most [`ARITHMETIC_RESIDUAL_FIXTURES`] fixtures** carry a gap above it, so one chaotic
    fixture no longer decides the cause on behalf of the other twenty-three.

    A row that declares a `layout seed` gap is excluded from the widened half. Its two arms never
    started alike, so a residual that lands on one fixture is the seed rather than the summation
    order, and `rng` is the cause a repair has to clear first.
    """
    if entry.get("max_gap", float("inf")) <= ARITHMETIC_GAP:
        return True
    if _has_seed_gap(row):
        return False
    residual = _residual_fixtures(entry)
    return residual is not None and residual <= ARITHMETIC_RESIDUAL_FIXTURES


def _residual_fixtures(entry):
    """How many fixtures carry a gap above [`ARITHMETIC_GAP`], or `None` when uncountable.

    `entry["max_gap"]` is the largest of those gaps, so it says *that* a residual exists and
    never *how many* carry it. A row with no per-fixture numbers cannot be counted, and an
    uncounted residual is `None` rather than `0`: only a row measured exact everywhere, or one
    whose aggregate gap is itself within `ARITHMETIC_GAP`, gets the widened rule.
    """
    gaps = [
        fixture.get("max_gap") for fixture in entry.get("per_fixture", [])
    ]
    gaps = [gap for gap in gaps if isinstance(gap, float)]
    if not gaps:
        return None
    return sum(1 for gap in gaps if gap > ARITHMETIC_GAP)


def _has_seed_gap(row):
    """Whether the row declares a `layout seed` gap, i.e. the two arms cannot start alike."""
    return any(
        gap["parameter"] == "layout seed" for gap in row.get("convention_gaps", [])
    )


#: The references that are **not reproducible between runs**, and therefore cannot have their
#: bytes pinned at all. Measured, not guessed: two consecutive runs of
#: `--graphviz` over the same `conformance.jsonl` and the same `-Gstart=981798123` were
#: compared file by file, and 31 of the 32 reference files were byte-identical while
#: `GRAPHVIZ_FDP.f64` differed. Graphviz's FDP draws its start from a C RNG that its `-Gstart`
#: does not seed (`lib/fdpgen/`), so the reference moves and the motor never does.
#:
#: This is a finding about the reference, not a licence: the row is still gated, on its motor
#: bytes and its measured shape, and `verdict::one_row` prints the reason beside the verdict.
NOT_REPRODUCIBLE = {
    "GRAPHVIZ_FDP": "the engine's own start is not seeded by -Gstart: two runs differ",
}


def propose_row(name, row, entry, motor_sha, reference_sha):
    """One line of `conformance/baseline.rs`, from this run's measurement of this row.

    A row that could not be measured proposes **empty** shas, which the judge refuses outright
    (`verdict::one_row`): an unpinned row is a row the matrix is waiting on, not a row that
    passed. The one exception is a row in [`NOT_REPRODUCIBLE`], whose reference sha is empty
    **with a reason**, and whose motor sha is still pinned.
    """
    tier, cause, why = classify(entry, row)
    note = NOT_REPRODUCIBLE.get(name, "")
    if note:
        reference_sha = ""
    if motor_sha and (reference_sha or note):
        ceiling = _ceiling(entry)
    else:
        ceiling = "f64::INFINITY"
    detail = " // %s" % why if why else ""
    return '    row("%s", "%s", "%s", "%s", %s, "%s", "%s"),%s' % (
        name, motor_sha, reference_sha, note, ceiling, tier, cause, detail,
    )


def _ceiling(entry):
    """The Procrustes ceiling a passing row is measured against.

    **The measured median, moved to the next power of ten above it** — the rule every other
    ceiling in this repo follows (`oracle_python/spring.rs`, `basic_3d.rs`). A ceiling sitting
    on the number it was measured from is not a ceiling: it would fail on the last bit of a
    `libm`.
    """
    import math

    median = entry.get("procrustes_median", 0.0)
    if median <= 0.0:
        return "f64::INFINITY"
    return "1e%d" % math.ceil(math.log10(median))
