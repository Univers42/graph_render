"""The cause classifier, driven by the rows that decide it.

`sc_propose.py` is the one file every `sg-*` job shares: it turns a run's `metrics.json` and
`motor.jsonl` into the `(tier, cause)` pair the baseline table and the matrix are pinned from.
Nothing else in the harness has a test, so this one holds the classifier to **measured rows**
rather than to hand-written dicts: the numbers below are transcribed from a run of
`scripts/scigraphs-conformance.sh` (`target/scigraphs-conformance/metrics.json`, the run quoted
in `docs/measurements/sg-spring-seed.md` and `docs/measurements/scigraphs-conformance.md`).

Run it in the oracle image the harness itself runs in:

    docker run --rm --user 0:0 -v "$PWD:/w" -w /w ge-python-oracle \\
        python3 harness/scigraphs-conformance/test_sc_propose.py -v

The three cases the widened rule has to separate are all here: `SPRING`, whose residual lands
on one fixture out of twenty-four and is therefore `arithmetic`; `GRAPHVIZ_TWOPI`, whose whole
90.0-unit step lands on all twenty-four and is therefore a `convention`; and `GRID` as it
measured before its scale fix — a disparity of 5e-32 with a gap of 4.0 — which must not become
`arithmetic` either, because its shape being exact is precisely what is not evidence.
"""

import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import sc_propose

#: `SPRING`'s 24 fixtures as `(name, max_gap, procrustes)`, transcribed from `metrics.json`.
#: `gate-00` carries no fit (`procrustes` is `None`), which `sc_metrics` records as one
#: unmeasured fixture of 24 — the discriminator reads the median and the two named fixtures,
#: so the missing fit is not what the case turns on.
SPRING_FIXTURES = (
    ("lesmis", 2.3736557427289084e-03, 1.663916971293011e-08),
    ("tree-balanced", 9.054289717980168e-08, 2.3562483399955027e-16),
    ("dag-diamond", 1.4206554421747342e-07, 2.954715407083551e-16),
    ("bipartite", 2.2464913485009674e-07, 6.87290889271226e-16),
    ("gate-00", 3.5496672268209295e-10, None),
    ("gate-01", 2.3519656977555314e-07, 1.9014693023490326e-16),
    ("gate-02", 1.183390221370928e-07, 1.4287734675220276e-16),
    ("gate-03", 2.074842608834615e-07, 1.5653199980532926e-16),
    ("gate-04", 5.7336085212966736e-08, 1.3449454150321726e-16),
    ("gate-05", 1.4408825688150273e-07, 2.917593868863284e-16),
    ("gate-06", 2.2818315503769782e-07, 4.621219212605969e-16),
    ("gate-07", 1.4366284073474844e-07, 5.023800487930867e-16),
    ("gate-08", 1.7451261502543503e-07, 4.7004805990622967e-16),
    ("gate-09", 2.1326282340083935e-07, 5.3510007279936625e-16),
    ("gate-10", 2.078849892228618e-07, 5.2320918301752144e-16),
    ("gate-11", 2.0215989859906358e-07, 5.29055184841196e-16),
    ("gate-12", 1.3334247128682364e-07, 3.7675322319180786e-16),
    ("gate-13", 2.2762824603006493e-07, 3.572619807668669e-16),
    ("gate-14", 2.3329651011039232e-07, 7.534956483883115e-16),
    ("gate-15", 1.0900650249112687e-07, 3.620449277965746e-16),
    ("gate-16", 1.2411400529543926e-07, 3.661096805036956e-16),
    ("gate-17", 1.1916949116397291e-07, 3.7366201156592526e-16),
    ("gate-18", 1.1862311666277492e-07, 3.8863417592285677e-16),
    ("gate-19", 1.5796004682044895e-07, 4.498249159436287e-16),
)

#: `SPRING`'s aggregate row, and the Procrustes median `sc_metrics.aggregate` computed from the
#: 23 fitted fixtures above (the median of the odd-length fitted list).
SPRING_MEDIAN = 3.7675322319180786e-16

#: `GRAPHVIZ_TWOPI`'s four named fixtures as measured. The other twenty measure the same
#: 90.0-unit step of a `twopi` drawing whose origin is one unit out, and are written once
#: below because the classifier counts them and never reads their names: a real scale gap is
#: a gap on *every* fixture, and that is the property this row is here for.
TWopi_NAMED = (
    ("lesmis", 270.5114783325195, 1.53597541608044e-10),
    ("tree-balanced", 226.56142048339848, 2.086725434395862e-10),
    ("dag-diamond", 90.0, 8.998559049065453e-34),
    ("bipartite", 128.82685676269531, 7.361199763923646e-10),
)
TWopi_GATES = tuple(
    ("gate-%02d" % index, 90.0, 2.590170315608238e-32)
    for index in range(20)
)
TWopi_MEDIAN = 2.0395983070005668e-10


def metrics(fixtures, median, coordinates=1020):
    """One `metrics.json` entry: the aggregate cells plus the per-fixture rows.

    `fixtures` is `(name, max_gap, procrustes)` per fixture and the aggregate `max_gap` is
    their maximum, which is what `sc_metrics.aggregate` computes and what `sc_propose` reads.
    The bitwise counts are zero, so the "the two lists are identical" branch of `classify`
    stays out of the way and every case here is decided by the shape and the gap.
    """
    return {
        "name": "case",
        "coordinates": coordinates,
        "fixtures": len(fixtures),
        "bitwise_f64": 0,
        "bitwise_f32": 0,
        "max_gap": max(gap for _, gap, _ in fixtures),
        "procrustes_median": median,
        "per_fixture": [
            {"fixture": name, "max_gap": gap, "procrustes": spread}
            for name, gap, spread in fixtures
        ],
    }


def motor_row(seeded=False):
    """A `motor.jsonl` line, with a `layout seed` gap when `seeded` asks for one."""
    gaps = [{"parameter": "layout seed", "note": "the arms start apart", "at": "start"}]
    return {"name": "case", "motor": "ok", "convention_gaps": gaps if seeded else []}


class ResidualOnOneFixture(unittest.TestCase):
    """`SPRING`: `f32`-identical on 21 of 24 fixtures and off by 2.37e-03 on `lesmis` alone.

    That is one fixture's chaos amplifying a fixed reduction difference, and no scale, centre
    or axis order accounts for it, so the cause is `arithmetic`.
    """

    def test_one_fixture_carrying_the_residual_is_arithmetic(self):
        tier, cause, _ = sc_propose.classify(
            metrics(SPRING_FIXTURES, SPRING_MEDIAN), motor_row()
        )
        self.assertEqual(cause, "arithmetic")
        self.assertEqual(tier, "tolerance")


class RowsThatStayConvention(unittest.TestCase):
    """A gap on every fixture is a convention, and an exact shape is not evidence against it."""

    def test_a_scale_gap_on_every_fixture_is_a_convention(self):
        fixtures = TWopi_NAMED + TWopi_GATES
        tier, cause, _ = sc_propose.classify(
            metrics(fixtures, TWopi_MEDIAN), motor_row()
        )
        self.assertEqual(cause, "convention")
        self.assertEqual(tier, "bitwise")

    def test_grid_before_its_scale_fix_is_a_convention(self):
        """Disparity 5e-32 and a gap of 4.0, on all 24 (`docs/measurements/sg-grid-scale.md`)."""
        fixtures = tuple(
            ("gate-%02d" % index, 4.0, 5e-32) for index in range(24)
        )
        tier, cause, _ = sc_propose.classify(metrics(fixtures, 5e-32), motor_row())
        self.assertEqual(cause, "convention")
        self.assertEqual(tier, "bitwise")

    def test_two_fixtures_carrying_the_residual_are_a_convention(self):
        """The boundary: one residual fixture is chaos, two is systematic."""
        widened = list(SPRING_FIXTURES)
        gate_19 = widened[-1]
        widened[-1] = (gate_19[0], 2.4e-03, gate_19[2])
        tier, cause, _ = sc_propose.classify(
            metrics(tuple(widened), SPRING_MEDIAN), motor_row()
        )
        self.assertEqual(cause, "convention")
        self.assertEqual(tier, "bitwise")


class WhatTheWideningMustNotTouch(unittest.TestCase):
    """The other two halves of the rule, which the fixture count does not weaken."""

    def test_a_different_shape_is_still_an_algorithm(self):
        """`CIRCLE_PACKING`'s shape verdict, with a residual on `lesmis` alone."""
        fixtures = (
            ("lesmis", 8.547e-01, 0.517),
            ("tree-balanced", 1.42e-07, 5.297068645513404e-16),
            ("bipartite", 1.18e-07, 6.87290889271226e-16),
        )
        tier, cause, _ = sc_propose.classify(
            metrics(fixtures, 5.297068645513404e-16), motor_row()
        )
        self.assertEqual(cause, "algorithm")
        self.assertEqual(tier, "shape")

    def test_a_seed_gap_is_still_the_rng(self):
        """A row whose arms never start alike is `rng`, however few fixtures carry a residual."""
        tier, cause, _ = sc_propose.classify(
            metrics(SPRING_FIXTURES, SPRING_MEDIAN), motor_row(seeded=True)
        )
        self.assertEqual(cause, "rng")
        self.assertEqual(tier, "bitwise")

    def test_an_uncountable_residual_is_not_arithmetic(self):
        """No per-fixture numbers means no count, and no count means no widened rule."""
        entry = metrics(SPRING_FIXTURES, SPRING_MEDIAN)
        entry.pop("per_fixture")
        tier, cause, _ = sc_propose.classify(entry, motor_row())
        self.assertEqual(cause, "convention")
        self.assertEqual(tier, "bitwise")


if __name__ == "__main__":
    unittest.main()