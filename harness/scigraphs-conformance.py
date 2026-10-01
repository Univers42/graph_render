#!/usr/bin/env python3
"""The SciGraphs conformance matrix, reference arm: every one of `apply_graph_layout`'s 32
names measured byte by byte against the motor layout it is supposed to be.

Three modes, one per image, because the two references do not live in the same place:

    # 1. the SciGraphs arm, in ge-python-oracle (numpy 2.3.3, scipy 1.16.2, networkx 3.6,
    #    igraph 0.11.9), with the SciGraphs/ submodule on the path:
    docker run --rm -v "$PWD:/w" -w /w ge-python-oracle \
        python3 harness/scigraphs-conformance.py --reference target/scigraphs-conformance

    # 2. the Graphviz arm, in ge-graphviz-oracle (pinned Graphviz 16.1.0):
    docker run --rm -v "$PWD:/w" -w /w ge-graphviz-oracle \
        python3 harness/scigraphs-conformance.py --graphviz target/scigraphs-conformance

    # 3. the metrics, the SVGs and the proposed baseline, back in ge-python-oracle:
    docker run --rm -v "$PWD:/w" -w /w ge-python-oracle \
        python3 harness/scigraphs-conformance.py --metrics target/scigraphs-conformance

Exit: 0 every row reached its reference · 1 a row could not · 2 this arm could not run.

**What each mode is allowed to change.** `--reference` and `--graphviz` write only into
`ref/`, and neither touches the fixture file or the motor's coordinates: the two arms are
independent of each other, and the fixture file is read-only to both. `--metrics` writes
`metrics.json`, `shapes/` and `conformance-baseline-proposed.rs`, and reads everything else.

**Why nine rows take the Graphviz arm rather than SciGraphs' own code.** SciGraphs reaches
Graphviz through `scigraphs_utils.graphviz_layout` (`yifan_hu.py:278-279`, `:366`), and
`scigraphs_utils` is not in either oracle image. The alternative — reimplementing the layer
here — would make the reference our guess about what SciGraphs does, which is the one thing a
conformance matrix must not be. So those nine rows are measured against the engine itself and
each of them carries a convention gap saying the `scale = 5.0` multiply and the spectral z are
not in these numbers.

Ponytail: the coordinates travel as raw little-endian `f64`, not as JSON. A decimal round trip
in the middle would be a rounding step between the two values whose equality is the question,
and `0.1` is not representable in binary to begin with.
"""

import argparse
import hashlib
import json
import os
import sys

sys.dont_write_bytecode = True

# The driver sits in `harness/` and its own modules in `harness/scigraphs-conformance/`, so
# both directories are on the path: the driver imports `sc_fixture` by name, and the child
# modules import each other by name.
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
sys.path.insert(0, os.path.join(HERE, "scigraphs-conformance"))

from sc_fixture import FixtureError, read_f64, read_fixtures, read_manifest  # noqa: E402
from sc_names import ALL_NAMES  # noqa: E402

DEFAULT_DIR = "target/scigraphs-conformance"


def parse_args(argv):
    parser = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    parser.add_argument("directory", nargs="?", default=DEFAULT_DIR)
    modes = parser.add_mutually_exclusive_group(required=True)
    modes.add_argument("--reference", action="store_true",
                       help="run apply_graph_layout itself (ge-python-oracle)")
    modes.add_argument("--graphviz", action="store_true",
                       help="run the Graphviz engines (ge-graphviz-oracle)")
    modes.add_argument("--metrics", action="store_true",
                       help="compare the two arms and draw the shapes (ge-python-oracle)")
    return parser.parse_args(argv)


def motor_rows(directory):
    """`motor.jsonl`, parsed: one row per SciGraphs name, in the emit's order."""
    path = os.path.join(directory, "motor.jsonl")
    with open(path) as handle:
        return [json.loads(line) for line in handle if line.strip()]


def main(argv):
    args = parse_args(argv[1:])
    directory = args.directory
    if not os.path.isdir(directory):
        print("scigraphs-conformance: %s is not a directory" % directory, file=sys.stderr)
        return 2
    try:
        fixtures = read_fixtures(directory)
        rows = motor_rows(directory)
        if args.reference:
            _reference(directory, fixtures, rows)
        elif args.graphviz:
            from sc_graphviz import check_fixtures, run

            check_fixtures(directory, fixtures)
            run(directory, fixtures)
        else:
            _metrics(directory, fixtures, rows)
        # **Both arms count the same way**: a row has reached its reference when a file
        # carrying its coordinates exists. Counting per mode would let one arm's nine rows and
        # the other's twenty-three be two different totals, and neither would be the matrix.
        reached = sum(
            1 for name in ALL_NAMES
            if os.path.exists(os.path.join(directory, "ref", "%s.f64" % name))
        )
        total = len(rows)
    except FixtureError as failure:
        print("scigraphs-conformance: could not run: %s" % failure, file=sys.stderr)
        return 2
    except (OSError, ValueError, ImportError) as failure:
        print("scigraphs-conformance: could not run: %s" % failure, file=sys.stderr)
        return 2
    print("scigraphs-conformance: %d/%d rows reached a reference" % (reached, total))
    return 0 if reached == total else 1


def _reference(directory, fixtures, rows):
    """Mode 1: `apply_graph_layout` for the 23 names this image hosts."""
    from sc_reference import check_names, run

    check_names(rows)
    print("scigraphs-conformance --reference: 23 names, SciGraphs' own dispatcher")
    return run(directory, fixtures)


def _metrics(directory, fixtures, rows):
    """Mode 3: the numbers, the SVGs and the proposed baseline."""
    from sc_shapes import draw

    manifest = read_manifest(directory)
    measured, refusals = {}, []
    for row in rows:
        name = row["name"]
        entry = _row_metrics(directory, fixtures, row)
        if "metrics" in entry:
            refusals.append("%s: %s" % (name, entry["metrics"]))
        measured[name] = entry

    shapes = draw(directory, fixtures, rows)
    _write_metrics(directory, manifest, fixtures, measured, shapes)
    _propose_baseline(directory, manifest, rows, measured)
    for line in refusals:
        print("  not run  %s" % line)
    return len(measured) - len(refusals)


def _row_metrics(directory, fixtures, row):
    """One name: the motor's coordinates beside the reference's, fixture by fixture.

    A fixture the reference did not reach is dropped rather than compared against nothing,
    and the row's cell count falls with it — so `coordinates` is always the number of
    coordinates that were **actually compared**, which is the only reading under which the
    bitwise counts mean anything.
    """
    from sc_metrics import aggregate, fixture_metrics

    name = row["name"]
    motor = os.path.join(directory, "motor", "%s.f64" % name)
    reference = os.path.join(directory, "ref", "%s.f64" % name)
    if not os.path.exists(reference):
        return {"metrics": "not run: the reference arm wrote no %s.f64" % name}
    if row["motor"].startswith("not run"):
        # **The motor's own reason, not this arm's guess.** A row with no motor layout is
        # unmeasurable because there is nothing to compare against, and "no fixture could be
        # fitted" would be the metrics arm inferring a cause from a symptom. The motor arm
        # already said which it was, so this repeats it rather than re-deriving it. The
        # `not run:` is stripped because this line adds its own.
        reason = row["motor"].removeprefix("not run: ")
        return {"metrics": "not run: %s" % reason}
    theirs = read_f64(reference)
    ours = read_f64(motor)
    at_motor = at_reference = 0
    per_fixture = []
    for fixture in fixtures:
        try:
            their_points, at_reference = _slice(theirs, fixture, at_reference, name)
            our_points, at_motor = _slice(ours, fixture, at_motor, name)
        except FixtureError as failure:
            per_fixture.append({
                "fixture": fixture.name,
                "metrics": "not run: %s" % failure,
            })
            continue
        entry = fixture_metrics(their_points, our_points)
        entry["fixture"] = fixture.name
        per_fixture.append(entry)
    summary = aggregate([e for e in per_fixture if "metrics" not in e])
    if not per_fixture:
        summary["metrics"] = "not run: no fixture held coordinates on both arms"
    summary["per_fixture"] = per_fixture
    summary["name"] = name
    return summary


def _slice(values, fixture, offset, name):
    """One fixture's coordinates out of a file, and the offset after them."""
    end = offset + 3 * fixture.n
    if end > len(values):
        raise FixtureError("%s holds %d coordinates, %s needs %d" % (
            name, len(values), fixture.name, end,
        ))
    return values[offset:end], end


def _write_metrics(directory, manifest, fixtures, measured, shapes):
    """`metrics.json`: every row's numbers, and the shas of the files they came from."""
    body = {
        "fingerprint": manifest["fingerprint"],
        "sha256": _sha256(directory, measured),
        "fixtures": [fixture.name for fixture in fixtures],
        "shapes": shapes,
        "rows": measured,
    }
    with open(os.path.join(directory, "metrics.json"), "w") as handle:
        json.dump(body, handle, indent=1, sort_keys=True)
        handle.write("\n")


def _sha256(directory, measured):
    """The digests the judge pins against: the fixture file and every `ref/*.f64`."""
    digests = {"conformance.jsonl": _digest(os.path.join(directory, "conformance.jsonl"))}
    for name in measured:
        path = os.path.join(directory, "ref", "%s.f64" % name)
        if os.path.exists(path):
            digests["ref/%s.f64" % name] = _digest(path)
    return digests


def _digest(path):
    with open(path, "rb") as handle:
        return hashlib.sha256(handle.read()).hexdigest()


def _propose_baseline(directory, manifest, rows, measured):
    """The pinned table this run measured, in the syntax `conformance/baseline.rs` takes.

    Written rather than printed because it is 32 lines of numbers that must be pasted into a
    Rust file without being retyped, and a retyped sha256 is a typo that would fail the gate in
    a way nobody could read.
    """
    from sc_propose import propose_row

    digests = _sha256(directory, measured)
    lines = ["// paste into conformance/baseline.rs, in place of the empty BASELINE:", "pub const BASELINE: &[Baseline] = &["]
    for row in rows:
        name = row["name"]
        lines.append(propose_row(
            name, row, measured.get(name, {}),
            manifest["sha256"].get("motor/%s.f64" % name, ""),
            digests.get("ref/%s.f64" % name, ""),
        ))
    lines.append("];")
    path = os.path.join(directory, "conformance-baseline-proposed.rs")
    with open(path, "w") as handle:
        handle.write("\n".join(lines) + "\n")
    print("  proposed baseline: %s" % path)


if __name__ == "__main__":
    sys.exit(main(sys.argv))
