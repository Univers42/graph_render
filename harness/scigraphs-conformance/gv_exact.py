"""`gv_exact.engine_points`, a drop-in for `gv_plain.engine_points` that reads no rounding.

`gv_plain` asks the engine for `-Tplain` and takes the coordinates off that text. The text
is inches at five significant digits -- `printdouble` is `agxbprint(&buf, "%.5g", v)`
(`lib/common/output.c:66-71`) -- so every reference coordinate lands on a 7.2e-4-point grid
and `GRAPHVIZ_TWOPI`'s `max_gap` in the conformance matrix floors at 7.5e-5 rather than at
zero (`sc_graphviz.py:12-19`). That grid is the harness's doing, not SciGraphs':
`graphviz_layout(num_nodes, edges, engine=...)` (`yifan_hu.py:298-307`) is handed a node
count and an edge list and returns an array, so what the matrix should compare is the
layout rather than the rendering of it.

This module compiles `gv_exact.c`, which links libgvc and prints `ND_coord(n).x` and
`ND_coord(n).y` with `%a`, and `float.fromhex` reads that back. Same layout, same order,
same seed, same units, same y direction: the only thing that changes is the precision the
coordinates survive at, which is the entire point.

Nothing else has to be undone to make the swap. `-Tplain` would have reflected y about the
graph's bounding box had `Y_invert` been set (`yDir`, `lib/common/output.c:35-37`, over
`GD_bb(g).UR.y + GD_bb(g).LL.y` from `setYInvert`, `:88-95`), but it is a zero-initialised
global (`lib/common/globals.c` via `lib/common/globals.h:40-42`) that only the `-y` flag
ever sets (`lib/common/input.c:415-416`), and neither `gv_plain.run_engine` nor
`gv_exact` passes it -- so plain's second column is `ND_coord(n).y` as is, which is what
`_max_gap` below checks. Units match too: `ND_coord` is already in points, and
`parse_plain`'s `* 72` (`gv_plain.py:24`, `:95-96`) is only undoing `PS2INCH`
(`lib/common/geom.h:64`) to get back to them.

Everything else is `gv_plain`'s: `write_dot` writes the same undirected DOT, so the same
fixtures give the same graphs, and `ENGINE_BENIGN_STDERR` (`gv_plain.py:49-61`) still
decides which stderr lines are a build notice rather than a failure -- sfdp's
`remove_overlap` stub sets the error flag and exits 1 while stdout holds a finished layout.
"""

import os
import subprocess
import sys
import tempfile

# `harness/` is inside `FINGERPRINTED` (`crates/graph-cli/src/fingerprint.rs:21`), so
# importing a module by name would write `harness/__pycache__/*.pyc` and move the
# fingerprint for as long as it exists. Set before the child modules are imported.
sys.dont_write_bytecode = True

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from gv_plain import ENGINE_BENIGN_STDERR, START_SEED, write_dot  # noqa: E402
from gv_plain import engine_points as gv_engine_points  # noqa: E402

from sc_names import graphviz_version  # noqa: E402

#: The prefix `gv_exact.c`'s header comment documents; the image's own gcc, its headers and
#: its shared libraries, with rpath so the built binary finds them without `LD_LIBRARY_PATH`.
GRAPHVIZ_PREFIX = "/opt/graphviz"

SOURCE = os.path.join(os.path.dirname(os.path.abspath(__file__)), "gv_exact.c")

#: The build from an earlier call, keyed by nothing because `tmp` is the caller's and the
#: process is short-lived. `None` until `exact_binary` has compiled once.
_BUILT = None


def _compile(path):
    """The one gcc invocation, with `path` spliced in as the output. Its stderr is the whole
    diagnostic, so a build failure exits with it rather than with a bare code."""
    cmd = [
        "gcc", "-O2", "-std=c11", "-Wall", "-Wextra", SOURCE,
        "-I%s/include" % GRAPHVIZ_PREFIX,
        "-L%s/lib" % GRAPHVIZ_PREFIX,
        "-lgvc", "-lcgraph", "-lcdt",
        "-Wl,-rpath,%s/lib" % GRAPHVIZ_PREFIX,
        "-o", path,
    ]
    proc = subprocess.run(cmd, capture_output=True, text=True)
    if proc.returncode != 0:
        sys.exit("gv_exact.c did not compile (%d):\n%s" % (proc.returncode, proc.stderr))


def exact_binary(tmp):
    """The path of the `gv_exact` built into `tmp`, compiling it only if it is not there.

    The compile lands on a pid-suffixed name and is renamed into place, so a run sharing
    `tmp` with another one can never pick up a half-written executable.
    """
    global _BUILT
    if _BUILT is not None and os.path.exists(_BUILT):
        return _BUILT
    path = os.path.join(tmp, "gv_exact")
    if not os.path.exists(path):
        partial = "%s.%d" % (path, os.getpid())
        _compile(partial)
        os.replace(partial, path)
    _BUILT = path
    return path


def run_exact(binary, engine, dot_path, start):
    """`gv_exact <engine> <start> <dot>`'s stdout, or the engine's own complaint and no answer.

    The stderr rule is `gv_plain.run_engine`'s verbatim: a non-zero exit is a failure unless
    every line of stderr is that engine's measured build notice, which for this program means
    sfdp alone (`gv_plain.py:49-61`).
    """
    cmd = [binary, engine, str(start), dot_path]
    proc = subprocess.run(cmd, capture_output=True, text=True)
    if proc.returncode != 0:
        benign = ENGINE_BENIGN_STDERR.get(engine, ())
        noise = [ln for ln in proc.stderr.splitlines() if ln.strip() not in benign]
        if noise or not benign:
            sys.exit("%s failed on %s: %s" % (engine, dot_path, proc.stderr))
    return proc.stdout


def exact_points(engine, tmp, name, count, edges, start=START_SEED):
    """The engine's own node coordinates over one DOT graph, as dense-indexed points.

    The same return shape as `gv_plain.engine_points` (`gv_plain.py:105-110`) and the same
    DOT from the same `write_dot`, so the two are interchangeable on x and on y alike (see
    the module docstring for why no reflection has to be undone). Refuses a node count that
    is not the fixture's, which is the one way the engine could silently answer about
    another graph (`parse_plain`, `gv_plain.py:98-102`).
    """
    dot = os.path.join(tmp, "%s.dot" % name)
    write_dot(dot, count, [a for a, _ in edges], [b for _, b in edges])
    nodes = {}
    for line in run_exact(exact_binary(tmp), engine, dot, start).splitlines():
        parts = line.split()
        if len(parts) != 3:
            continue
        nodes[parts[0]] = (float.fromhex(parts[1]), float.fromhex(parts[2]))
    if len(nodes) != count:
        sys.exit("gv_exact printed %d nodes for %s, expected %d" % (len(nodes), dot, count))
    return [nodes["n%d" % i] for i in range(count)]


def _ring(count):
    """One undirected ring over `count` nodes: the 77-node fixture the check below runs on."""
    return [(i, (i + 1) % count) for i in range(count)]


def _max_gap(exact, plain):
    """The largest disagreement between the two readings of one layout, in points, as
    `(x, y)`. Both axes are compared outright, with no shift or reflection applied to
    either side, because `-Tplain` applies none (`Y_invert` off by default; see the module
    docstring). What is left on either axis is the `%.5g` each `-Tplain` value went
    through, which is the whole claim: this module reads the same layout the arm read
    before, with nothing of it thrown away."""
    dx = dy = 0.0
    for (ax, ay), (bx, by) in zip(exact, plain):
        dx = max(dx, abs(ax - bx))
        dy = max(dy, abs(ay - by))
    return dx, dy


def _report(tmp, fixtures, engines):
    """One block per fixture, one line per engine: how far apart the two readings are."""
    for name, count, edges in fixtures:
        print("%s (%d nodes, %d edges)" % (name, count, len(edges)))
        for engine in engines:
            dx, dy = _max_gap(
                exact_points(engine, tmp, name, count, edges),
                gv_engine_points(engine, tmp, name, count, edges),
            )
            print("  %-10s dx %.3e pt   dy %.3e pt" % (engine, dx, dy))


#: The two fixtures the check reads: a triangle, where any two nodes can differ, and a
#: 77-node ring, which is wide enough that the two readings cannot agree by luck.
FIXTURES = (("triangle", 3, [(0, 1), (1, 2), (2, 0)]), ("ring", 77, _ring(77)))

#: The eight engines SciGraphs reaches through `scigraphs_utils` (`sc_graphviz.py:1`), all
#: of which are layout plugin names as well as executables, so `gv_plain`'s
#`engine -Tplain` and `gv_exact`'s `gvLayout(gvc, g, engine)` ask for the same layout.
ENGINES = ("twopi", "patchwork", "dot", "neato", "fdp", "sfdp", "circo", "osage")


def main():
    """Compile the C once, then measure every engine against `-Tplain` on both fixtures."""
    with tempfile.TemporaryDirectory() as tmp:
        print("binary:  %s" % exact_binary(tmp))
        print("graphviz: %s" % graphviz_version("dot"))
        _report(tmp, FIXTURES, ENGINES)


if __name__ == "__main__":
    main()