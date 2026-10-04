"""The two arms of the conformance matrix, cut into one fixture each, and who refused.

`harness/scigraphs-conformance.py` reads `ref/<NAME>.f64` and `motor/<NAME>.f64` as flat
concatenations of `f64` with no header and no index, so the only way to recover a fixture's own
coordinates is a running offset — and **an offset is only honest if it advances for exactly the
fixtures an arm wrote.** An arm writes what it produced and writes nothing at all for a fixture
it skipped, so the two files are different lengths whenever either arm refused one.

Measured 2026-10-04 on `sg-igraph-3d`: the reference defect `G_KK_NON_FINITE` left
`ref/IGRAPH_KK.json` fixture 5 (`gate-01`, 3 nodes) marked `"not run"`, so `ref/IGRAPH_KK.f64`
held 1011 coordinates where the motor held 1020. The metrics arm advanced both offsets
regardless, and every fixture from `gate-01` on was compared against its **successor's**
reference coordinates. No cell said so — they were numbers, not nonsense, and each was a real
coordinate compared to a real coordinate — so the drift stayed invisible until the last fixture,
where it surfaced as `not run: IGRAPH_KK holds 1011 coordinates, gate-19 needs 1020`. The
driver's own docstring claimed such a fixture was dropped; the code did not drop it.

So the rule this module holds: **a fixture either arm refused is reported in that arm's own
words, and only the arm that wrote it advances.**
"""

import json
import os

from sc_fixture import FixtureError, slice_run

#: The two arm labels, written verbatim into every cell, so one grep finds every refusal and
#: every shortfall. They match the keys `ref/<NAME>.json` and `motor.jsonl` are read under.
REFERENCE = "reference"
MOTOR = "motor"


class Arm:
    """One side of the comparison: what it is called, and where its next fixture starts."""

    def __init__(self, label, name, values):
        self.label = label
        self.name = name
        self.values = values
        self.at = 0

    def take(self, fixture):
        """This arm's coordinates for the fixture, advancing the offset past them.

        Raises [`FixtureError`] from [`sc_fixture.slice_run`] when the file is shorter than the
        fixture set asks for, which is this arm's own shortfall.
        """
        points, self.at = slice_run(self.values, fixture, self.at)
        return points


def refusals(directory, row, fixtures):
    """Per fixture, the `(reference, motor)` refusal that arm recorded — `None` where it wrote.

    **The arm's own record, never a length re-derived here.** The reference arm writes a
    `status` and a `detail` per fixture into `ref/<NAME>.json`, and the SciGraphs and the
    Graphviz arms both write that file, so the rule holds for either. The motor arm records
    only the *names* of the fixtures it skipped (`conformance/motor.rs` builds `skipped` by
    filtering `run.is_err()` against them) and keeps its reason at row level, so a motor cell
    quotes the row. Dropping a fixture on the arm's word rather than on a file length is what
    makes the two arms symmetric, and it is what keeps the offsets in step.
    """
    report = _report(directory, row["name"])
    skipped = set(row.get("skipped") or ())
    pairs = []
    for index, fixture in enumerate(fixtures):
        status, detail = report[index] if index < len(report) else ("ok", "")
        pairs.append((
            None if status == "ok" else "not run: %s %s: %s" % (REFERENCE, status, detail),
            None if fixture.name not in skipped else "not run: %s skipped: %s" % (
                MOTOR, row["motor"]),
        ))
    return pairs


def _report(directory, name):
    """`ref/<NAME>.json`'s per-fixture `(status, detail)` pairs, in fixture order.

    Ponytail (a missing `.json` reads as "every fixture ran"): this is the status quo the fix
    started from, kept so a `.f64` with no report beside it still measures. It cannot silently
    misalign, because a file short of coordinates still raises out of `Arm.take` and names the
    arm that ran out; the only thing it can miss is a fixture the reference skipped *and* did
    not write down. Escape hatch: write the `.json`, which both reference arms already do.
    """
    path = os.path.join(directory, "ref", "%s.json" % name)
    try:
        with open(path) as handle:
            entries = json.load(handle).get("fixtures") or []
    except (OSError, ValueError):
        return []
    return [(entry.get("status"), entry.get("detail") or "") for entry in entries]


def per_fixture(reference, motor, fixtures, pairs):
    """One entry per fixture, in fixture order: measured, or the refusal that replaced it."""
    entries = []
    for fixture, pair in zip(fixtures, pairs):
        entry = _entry(fixture, reference, motor, pair)
        entry["fixture"] = fixture.name
        entries.append(entry)
    return entries


def _entry(fixture, reference, motor, pair):
    """One fixture: its numbers from the arms that wrote it, or the refusal that replaced it.

    **Only an arm that wrote the fixture advances.** The arm that did not contributes no
    coordinates to skip past, so leaving its offset alone is what puts the *next* fixture's
    slice on its own data. The arm that did write it still advances — dropping its coordinates
    too would slide every later fixture the other way, which is the same defect with the sign
    flipped, and it is the case this module's own first draft got wrong.
    """
    from sc_metrics import fixture_metrics

    their_refusal, our_refusal = pair
    try:
        their_points = None if their_refusal else reference.take(fixture)
    except FixtureError as failure:
        return {"metrics": "not run: %s: %s" % (reference.label, failure)}
    try:
        our_points = None if our_refusal else motor.take(fixture)
    except FixtureError as failure:
        return {"metrics": "not run: %s: %s" % (motor.label, failure)}
    if their_points is None or our_points is None:
        return {"metrics": their_refusal or our_refusal}
    return fixture_metrics(their_points, our_points)
