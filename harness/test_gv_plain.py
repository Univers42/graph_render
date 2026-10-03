"""`harness/gv_plain.py`'s refusals, one test each, against stub engines.

Run in the `ge-graphviz-oracle` image, because `graphviz_version` asks the real `dot`.
**Pass `-B`**, or CPython writes `harness/__pycache__/test_gv_plain.*.pyc` while it
compiles this very file — before the `sys.dont_write_bytecode` below can run — and
`harness/` is inside the gate's fingerprinted set (`crates/graph-cli/src/fingerprint.rs:27`),
so the bytecode moves the fingerprint for as long as it exists:

  docker run --rm --user 0:0 -v $PWD:/w -w /w ge-python-oracle \
      python3 -B -m unittest discover -s harness -p 'test_gv_plain.py' -v

The stub engines are the point: M26's reproducer is an `sfdp` that exits non-zero having
written nothing to stderr, which no real Graphviz does, so it has to be written down.
"""

import os
import stat
import sys
import tempfile
import unittest

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import gv_plain  # noqa: E402
from gv_plain import (  # noqa: E402
    ENGINE_BENIGN_STDERR,
    PINNED_GRAPHVIZ,
    graph_of,
    graphviz_version,
    printed_nodes,
    run_engine,
)

# The three stub engines, as (what they print on stdout, what they print on stderr, exit).
DRAWING = "graph 1 1 72 36\nnode n0 0 0 1 1 1 1\nnode n1 0 36 1 1 1 1\nstop\n"
NOTICE = "Error: remove_overlap: Graphviz not built with triangulation library\n"


def stub(name, stdout, stderr, code):
    """Put a one-line executable `name` at the front of PATH, returning its directory."""
    return script(name, f"printf '%s' '{stdout}'\nprintf '%s' '{stderr}' >&2\nexit {code}\n")


def echo_stub(name):
    """A stub that prints its own argv, so a test can read the command that was run."""
    return script(name, "printf '%s\\n' \"$@\"\nexit 0\n")


def script(name, body):
    directory = tempfile.mkdtemp()
    path = os.path.join(directory, name)
    with open(path, "w") as handle:
        handle.write(f"#!/bin/sh\n{body}")
    os.chmod(path, os.stat(path).st_mode | stat.S_IEXEC | stat.S_IXGRP | stat.S_IXOTH)
    return directory


class on_path:
    """PATH with `directory` in front, restored on the way out."""

    def __init__(self, directory):
        self.directory = directory

    def __enter__(self):
        self.prior = os.environ["PATH"]
        os.environ["PATH"] = self.directory + os.pathsep + self.prior
        return self

    def __exit__(self, *stop):
        os.environ["PATH"] = self.prior


def refused(callable_, *args, **kwargs):
    try:
        callable_(*args, **kwargs)
    except SystemExit as stop:
        return str(stop)
    raise AssertionError("expected a refusal, got a return")


class RunEngineStderr(unittest.TestCase):
    """M26: the sfdp carve-out accepted a non-zero exit that wrote nothing to stderr."""

    def test_sfdps_own_notice_is_the_one_drawing_it_exits_one_for(self):
        # The real sfdp: notice on stderr, complete drawing on stdout, exit 1.
        with on_path(stub("sfdp", DRAWING, NOTICE, 1)):
            self.assertEqual(run_engine("sfdp", "g.dot"), DRAWING)

    def test_a_failing_engine_that_wrote_nothing_to_stderr_is_refused(self):
        # M26's reproducer: `benign` is truthy for sfdp and `noise` is empty, so the old
        # `if noise or not benign` accepted this drawing as a success.
        with on_path(stub("sfdp", DRAWING, "", 1)):
            message = refused(run_engine, "sfdp", "g.dot")
        self.assertIn("sfdp failed on g.dot", message)

    def test_an_engine_with_no_carve_out_is_refused_on_its_exit_alone(self):
        with on_path(stub("twopi", DRAWING, "", 3)):
            self.assertIn("twopi failed", refused(run_engine, "twopi", "g.dot"))

    def test_a_carve_out_does_not_swallow_other_stderr(self):
        with on_path(stub("sfdp", DRAWING, NOTICE + "Error: something else\n", 1)):
            self.assertIn("sfdp failed", refused(run_engine, "sfdp", "g.dot"))

    def test_the_notice_the_carve_out_names_is_the_measured_one(self):
        self.assertEqual(
            ENGINE_BENIGN_STDERR["sfdp"],
            ("Error: remove_overlap: Graphviz not built with triangulation library",),
        )


class RunEngineSeed(unittest.TestCase):
    """m63: `start=START_SEED` bound the seed at `def` time, so a rebind never arrived."""

    def test_the_seed_is_read_when_the_call_is_made(self):
        with on_path(echo_stub("twopi")):
            prior = gv_plain.START_SEED
            try:
                gv_plain.START_SEED = 99
                # A `def`-time default would still run the old seed here.
                self.assertIn("-Gstart=99", run_engine("twopi", "g.dot"))
            finally:
                gv_plain.START_SEED = prior

    def test_an_explicit_seed_still_wins(self):
        with on_path(echo_stub("twopi")):
            self.assertIn("-Gstart=7", run_engine("twopi", "g.dot", 7))


class PrintedNodes(unittest.TestCase):
    """m64: no graph line and no node count, so a truncated drawing was a `KeyError`."""

    graph = graph_of(2, [(0, 1)])

    def printed(self, body):
        """`printed_nodes` over `body`, with the DOT written into a scratch directory."""
        with on_path(stub("twopi", body, "", 0)):
            with tempfile.TemporaryDirectory() as scratch:
                return printed_nodes(
                    "twopi", os.path.join(scratch, "g.dot"), self.graph
                )

    def test_a_complete_drawing_gives_its_own_strings(self):
        self.assertEqual(self.printed(DRAWING), [("0", "0"), ("0", "36")])

    def test_a_drawing_with_no_graph_line_is_refused(self):
        body = "node n0 0 0 1 1 1 1\nnode n1 0 36 1 1 1 1\n"
        self.assertIn("printed no graph line", refused(self.printed, body))

    def test_a_truncated_drawing_is_refused_by_node_count(self):
        body = "graph 1 1 72 36\nnode n0 0 0 1 1 1 1\n"
        self.assertIn("printed 1 nodes", refused(self.printed, body))

    def test_an_empty_drawing_is_refused_not_an_index_error(self):
        self.assertIn("no graph line", refused(self.printed, ""))


class GraphvizVersion(unittest.TestCase):
    """M18: `dot -V` was never consulted, so any image claimed 16.1.0."""

    def test_the_real_dot_reports_the_pinned_version(self):
        self.assertEqual(graphviz_version(), PINNED_GRAPHVIZ)

    def test_a_dot_that_reports_nothing_is_refused(self):
        with on_path(stub("dot", "", "", 0)):
            self.assertIn("reported no version", refused(graphviz_version))

    def test_another_version_is_refused(self):
        with on_path(stub("dot", "", "dot - graphviz version 99.9.9 (x)\n", 0)):
            self.assertIn("want 16.1.0", refused(graphviz_version))


if __name__ == "__main__":
    unittest.main()