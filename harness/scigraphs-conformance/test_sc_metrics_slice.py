"""The coordinate slicing in `_row_metrics`, held to the alignment it claims.

`harness/scigraphs-conformance.py` cuts `ref/<NAME>.f64` and `motor/<NAME>.f64` into one
fixture each by running offset, and the two files are **not** the same length whenever an arm
skipped a fixture: an arm writes only what it produced, with no placeholder for what it did
not. Slicing both by the fixture's own index therefore compares the motor against the *next*
fixture's coordinates for every fixture after the first skip.

Measured 2026-10-04 on `sg-igraph-3d` after `scripts/scigraphs-conformance.sh`: the reference
defect `G_KK_NON_FINITE` left `ref/IGRAPH_KK.json` fixture 5 (`gate-01`, 3 nodes) marked
`"not run"`, and `ref/IGRAPH_KK.f64` held 1011 coordinates where the motor held 1020. From
`gate-01` on, every `IGRAPH_KK` fixture was compared against its successor's reference
coordinates, and the drift only became visible at the last fixture, where it surfaced as
`not run: IGRAPH_KK holds 1011 coordinates, gate-19 needs 1020`. The function's own docstring
says a fixture the reference did not reach is dropped; the code did not drop it.

So the contract under test is that **an arm's offset advances only for a fixture that arm
wrote**, and a fixture either arm skipped is reported rather than compared. Symmetric on
purpose: `motor.jsonl` records a skip as a fixture *name* in `skipped` (`conformance/motor.rs`
builds it by filtering `run.is_err()` against the names), while `ref/<NAME>.json` records a
per-fixture `status` and `detail` — the two arms do not record a skip the same way, and the fix
has to hold for either one.

Run it in the oracle image the harness itself runs in, which is where `sc_metrics` gets its
numpy and scipy:

    scripts/orch/drun --rm --user 0:0 --pull never -v "$PWD:/w" -w /w ge-python-oracle \\
        python3 harness/scigraphs-conformance/test_sc_metrics_slice.py -v
"""

import importlib.util
import json
import os
import sys
import tempfile
import unittest

sys.dont_write_bytecode = True

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import sc_fixture

#: The driver is `harness/scigraphs-conformance.py` — a hyphen is not an importable name — so it
#: is loaded by path. Inserting the sibling directory first is what lets it find `sc_fixture`.
DRIVER_PATH = os.path.join(
    os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "scigraphs-conformance.py"
)
_spec = importlib.util.spec_from_file_location("sc_conformance", DRIVER_PATH)
driver = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(driver)

#: The row name; one row of the matrix, which is what `ref/<NAME>.*` and `motor/<NAME>.*` key on.
NAME = "IGRAPH_KK"

#: Three fixtures, three nodes each, nine coordinates apiece. The base is per-fixture and far
#: apart, so comparing a fixture against its successor cannot look like a comparison against
#: itself: every coordinate would differ, and `bitwise_f64` would be 0 rather than 9.
BASES = {"gate-a": 100.0, "gate-b": 200.0, "gate-c": 300.0}
NODES = 3
WIDTH = 3 * NODES


def fixture_record(name):
    """The `conformance.jsonl` line `sc_fixture.Fixture` accepts for a three-node fixture."""
    nodes = ["n%d" % index for index in range(NODES)]
    return {
        "name": name,
        "n": NODES,
        "nodes": nodes,
        "source": [0, 1],
        "target": [1, 2],
        "mapping": [[index, node] for index, node in enumerate(nodes)],
    }


def coordinates(name, drift=0.0):
    """One fixture's nine coordinates, identifiable by the fixture's own base value."""
    base = BASES[name] + drift
    return [base + step for step in range(WIDTH)]


def motor_row(skipped):
    """One `motor.jsonl` row, with the fields `_row_metrics` and `sc_propose` read."""
    detail = "partial: no coordinates on %s" % ", ".join(skipped) if skipped else "ok"
    return {"name": NAME, "motor": detail, "skipped": list(skipped), "coordinates": 0}


class ArmSkipsAFixture(unittest.TestCase):
    """Three fixtures on disk; one of the two arms did not write the middle one."""

    def setUp(self):
        self.dir = tempfile.mkdtemp(prefix="sc-metrics-slice-")
        self.names = list(BASES)
        with open(os.path.join(self.dir, "conformance.jsonl"), "w") as handle:
            for name in self.names:
                handle.write(json.dumps(fixture_record(name)) + "\n")
        self.fixtures = sc_fixture.read_fixtures(self.dir)

    def tearDown(self):
        for root, _, files in os.walk(self.dir, topdown=False):
            for entry in files:
                os.remove(os.path.join(root, entry))
            os.rmdir(root)

    def write_values(self, arm, written, drift=0.0):
        """One arm's `.f64`, carrying exactly the fixtures named and nothing else."""
        values = [value for name in written for value in coordinates(name, drift)]
        os.makedirs(os.path.join(self.dir, arm), exist_ok=True)
        sc_fixture.write_f64(os.path.join(self.dir, arm, "%s.f64" % NAME), values)

    def write_report(self, written):
        """`ref/<NAME>.json`: a `status` and a `detail` per fixture, in fixture order."""
        report = []
        for name in self.names:
            if name in written:
                report.append({"fixture": name, "status": "ok"})
            else:
                report.append({
                    "fixture": name,
                    "status": "not run",
                    "detail": "apply_graph_layout returned False",
                })
        path = os.path.join(self.dir, "ref", "%s.json" % NAME)
        with open(path, "w") as handle:
            json.dump({"name": NAME, "status": "ok", "fixtures": report}, handle)

    def metrics(self, skipped=()):
        return driver._row_metrics(self.dir, self.fixtures, motor_row(skipped))

    def assert_neighbours_measured_against_themselves(self, summary):
        """The claim the defect broke: the fixtures beside a skipped one keep their own numbers."""
        by_name = {entry["fixture"]: entry for entry in summary["per_fixture"]}
        for name in ("gate-a", "gate-c"):
            entry = by_name[name]
            self.assertEqual(entry["coordinates"], WIDTH, name)
            self.assertEqual(entry["bitwise_f64"], WIDTH, name)
            self.assertEqual(entry["max_gap"], 0.0, name)


class ReferenceSkippedTheMiddleFixture(ArmSkipsAFixture):
    """The measured defect: `ref/<NAME>.json` marks `gate-b` `"not run"`."""

    def setUp(self):
        super().setUp()
        self.write_values("ref", ("gate-a", "gate-c"))
        self.write_report(("gate-a", "gate-c"))
        self.write_values("motor", self.names)

    def test_the_skipped_fixture_is_reported_with_the_references_own_detail(self):
        summary = self.metrics()
        middle = summary["per_fixture"][1]
        self.assertEqual(middle["fixture"], "gate-b")
        self.assertEqual(
            middle["metrics"],
            "not run: reference not run: apply_graph_layout returned False",
        )
        self.assertNotIn("bitwise_f64", middle)

    def test_the_fixtures_after_the_skip_keep_their_own_coordinates(self):
        self.assert_neighbours_measured_against_themselves(self.metrics())

    def test_only_the_fixture_both_arms_wrote_is_the_one_dropped(self):
        """The row's totals, and *which* fixture they exclude.

        Before the fix these totals were already right by accident — the misaligned `gate-b` was
        measured in place of the skipped one, so two fixtures still counted — which is why the
        excluded name is asserted and not just the counts.
        """
        summary = self.metrics()
        self.assertEqual(summary["coordinates"], 2 * WIDTH)
        self.assertEqual(summary["fixtures"], 2)
        self.assertEqual(len(summary["per_fixture"]), 3)
        dropped = [e["fixture"] for e in summary["per_fixture"] if "metrics" in e]
        self.assertEqual(dropped, ["gate-b"])


class MotorSkippedTheMiddleFixture(ArmSkipsAFixture):
    """The symmetric arm: `motor.jsonl` lists `gate-b` in `skipped` and holds no coordinates."""

    def setUp(self):
        super().setUp()
        self.write_values("ref", self.names)
        self.write_report(self.names)
        self.write_values("motor", ("gate-a", "gate-c"))

    def test_the_skipped_fixture_is_reported_from_the_motor_rows_own_record(self):
        summary = self.metrics(["gate-b"])
        middle = summary["per_fixture"][1]
        self.assertEqual(middle["fixture"], "gate-b")
        self.assertEqual(
            middle["metrics"],
            "not run: motor skipped: partial: no coordinates on gate-b",
        )
        self.assertNotIn("bitwise_f64", middle)

    def test_the_fixtures_after_the_skip_keep_their_own_coordinates(self):
        self.assert_neighbours_measured_against_themselves(self.metrics(["gate-b"]))

    def test_both_arms_skipping_the_same_fixture_reports_one_of_them(self):
        """Two refusals, one cell: the fixture is `not run` whichever arm is named."""
        self.write_values("motor", ("gate-a",))
        summary = self.metrics(["gate-b", "gate-c"])
        self.assertIn("not run:", summary["per_fixture"][1]["metrics"])
        self.assertIn("not run:", summary["per_fixture"][2]["metrics"])


class DriftsAreStillMeasured(ArmSkipsAFixture):
    """No fixture is skipped, so this guards the fix against comparing stubs.

    Every fixture's numbers have to come from the coordinates actually compared. If a fix
    reported a fixture it never sliced, or sliced a neighbour's run, `gate-a` would measure a
    gap of 100.0 or 200.0 rather than the 1.0 written here.
    """

    def test_a_drifted_fixture_is_seen_as_drifted_by_its_own_amount(self):
        self.write_values("ref", self.names)
        self.write_report(self.names)
        self.write_values("motor", self.names, drift=1.0)
        summary = self.metrics()
        gaps = {e["fixture"]: e["max_gap"] for e in summary["per_fixture"]}
        self.assertEqual(gaps, {"gate-a": 1.0, "gate-b": 1.0, "gate-c": 1.0})

    def test_an_exact_fixture_is_bitwise(self):
        self.write_values("ref", self.names)
        self.write_report(self.names)
        self.write_values("motor", self.names)
        summary = self.metrics()
        self.assertEqual(summary["bitwise_f64"], 3 * WIDTH)
        self.assertEqual(summary["coordinates"], 3 * WIDTH)


class AFileShorterThanItsReport(ArmSkipsAFixture):
    """The report says every fixture ran, and the `.f64` says otherwise.

    Nothing in the harness writes this state, and that is the point: it is what a fixture set
    that grew since the reference arm ran would look like, and the cell has to name **the arm
    that ran out** rather than the arm that happened to be sliced first.
    """

    def test_the_shortfall_names_the_arm_whose_file_ran_out(self):
        self.write_values("ref", ("gate-a", "gate-b"))
        self.write_report(self.names)
        self.write_values("motor", self.names)
        summary = self.metrics()
        last = summary["per_fixture"][2]
        self.assertEqual(last["fixture"], "gate-c")
        self.assertEqual(
            last["metrics"],
            "not run: reference: the file holds 18 coordinates, gate-c needs 27",
        )


if __name__ == "__main__":
    unittest.main()
