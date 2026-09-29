"""The filtering rows: what each one claims, how it is driven, and how it is judged.

Every row is a claim about what the studio hides, colours and fits, and every claim is
judged against something the probe works out for itself: the query grammar is parsed and
evaluated here, in Python, and so is the look library's inferno ramp. A row that compares
the product against the product proves only that the product agrees with itself, so the two
oracles live in this file and the studio is never asked what it thinks the answer is.

The input goes in through the studio's own registry (`dispatch(id, raw)` and the console
line `run(line)`), because a filter is not a gesture: it is what a dock control and a typed
line both do. A row whose control is not in the registry yet is reported NOT-RUN with the
reason, never PASS.

`--break` is the negative control and it changes exactly one thing: this file's own
evaluator reads `NOT x` as `x`. The flag is a parameter threaded from `--break` into the
evaluator, so a red row under the break is a red row about the probe's arithmetic and about
nothing else.
"""
from pathlib import Path


# A filter redraws; the view answers on the next frame, so a read taken straight after the
# dispatch would be a read of the drawing that was there before.
SETTLE_S = 0.35
# The motor's own budget for the document load and the analysis run behind a dispatch.
BUSY_S = 90.0

# The table the metric colouring is checked against, read out of the committed one. The
# container mounts the repository at /w and runs from /w/deploy/nav, so the root is /w.
ROOT = Path(__file__).resolve().parents[2]
TABLES = ROOT / "packages/graph-render/src/colour/tables.ts"
COLORMAP = "inferno"
STOPS = 32
MISSING = (0.3, 0.3, 0.3)


class Missing(Exception):
    """The product surface this row needs is not there: NOT-RUN, with the reason."""


class Unjudgeable(Exception):
    """The probe read something it will not guess about: FAIL, never a silent pass."""


def row(name, expectation, measured, passed, why=None):
    verdict = "PASS" if passed else ("NOT-RUN" if why is not None else "FAIL")
    return {"row": name, "expectation": expectation, "measured": measured,
            "verdict": verdict, "why": why}


def guarded(name, expectation, body):
    """A row can fail and a row can be left unrun, but neither of them is a traceback."""
    try:
        return body()
    except Missing as gap:
        return row(name, expectation, "nothing was driven", False, str(gap))
    except Exception as failure:  # a probe bug is a red row, not a crash
        return row(name, expectation, f"the probe could not judge it: {failure}", False)
