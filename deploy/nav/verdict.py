"""One row's verdict: what it claimed, what it measured, and whether it passed.

Its own module so a row module can name it without importing the table that lists the rows.
A row that could not be measured at all is NOT-RUN and carries the reason; that is never
counted as a pass.
"""


def row(name, expectation, measured, passed, why=None):
    verdict = "PASS" if passed else ("NOT-RUN" if why is not None else "FAIL")
    return {"row": name, "expectation": expectation, "measured": measured,
            "verdict": verdict, "why": why}