"""The graph, the layout and the metrics, imported from SciGraphs itself.

Nothing here re-implements a SciGraphs algorithm. ``scigraphs_core`` is pure
Python over numpy and networkx, so the fixture calls the same functions the
Blender add-on calls:

* ``core/scigraphs_core/mesh/layouts/networkx_layouts.py:26-34`` —
  ``_spring_layout_3d``, the SPRING_3D branch the dispatcher reaches
  (``core/scigraphs_core/mesh/layouts/dispatcher.py:57-58``).
* ``core/scigraphs_core/repro/determinism.py:56-62`` — ``get_layout_seed``,
  i.e. ``derive_seed(base, "layout")``, and ``set_pipeline_seed``.
* ``core/scigraphs_core/algorithms/analysis.py:149-153`` — the betweenness
  branch of ``calculate_centrality``.
* ``core/scigraphs_core/coloring/colormaps.py:462-469`` and ``:527-530`` —
  ``_average_ranks`` and the RANK arm of ``normalize_values``.
* ``SciGraphs/SciGraphs/core/mesh/geometry.py:259-287`` — the (min, max)
  canonicalisation, reproduced below because it is eleven lines of numpy inside
  a bpy function.

PONYTAIL: ``python-igraph`` is not in the oracle image, so
``calculate_centrality`` takes its ``nx.betweenness_centrality`` fallback rather
than ``_igraph_betweenness`` (``analysis.py:62-76``). The two agree to 7e-18 on
this graph by the fallback's own comment; SciGraphs' default normalisation is
``normalized=True, weight=None, endpoints=False``, which is what networkx uses.
"""

import contextlib
import dataclasses
import hashlib
import os
import re
import sys

import numpy as np

SCIGRAPHS_ROOT = os.environ.get("SCIGRAPHS_ROOT", "/sg")
BASE_SEED = 42
GRAPH_GENERATOR = "networkx.les_miserables_graph"
LAYOUT_ALGORITHM = "SPRING_3D"
LAYOUT_DIM = 3
LAYOUT_ITERATIONS = 150
LAYOUT_SCALE = 5.0
COLOR_NORM = "RANK"
BETWEENNESS_METHOD = "nx.betweenness_centrality(normalized=True)"

_STATE = {}


@dataclasses.dataclass(frozen=True)
class Lesmis:
    """The generator's nodes, relabelled to dense indices in its own order."""

    labels: tuple
    order: dict
    graph: object
    raw_edges: tuple


def scigraphs_setup(root=SCIGRAPHS_ROOT):
    """Import ``scigraphs_core`` from *root* and seed Python, numpy and layouts.

    SciGraphs' import-time warnings go to stdout (``common.py:92``), which would
    corrupt the JSON this harness writes there, so they are rerouted to stderr.
    """
    if _STATE:
        return
    if root + "/core" not in sys.path:
        sys.path.insert(0, root + "/core")
    with contextlib.redirect_stdout(sys.stderr):
        import networkx as nx
        from scigraphs_core.repro import determinism
        from scigraphs_core.coloring.colormaps import normalize_values
        from scigraphs_core.mesh.layouts import common as layouts_common
        from scigraphs_core.mesh.layouts import networkx_layouts
    _STATE.update(root=root, nx=nx, determinism=determinism,
                  normalize_values=normalize_values, common=layouts_common,
                  layouts=networkx_layouts)
    determinism.set_pipeline_seed(BASE_SEED)


def library_versions():
    """The two versions the numbers were produced with."""
    return _STATE["nx"].__version__, np.__version__


def source_digest(root, relative_path):
    """sha256 of the cited file under *root*, hex.

    *relative_path* is the repo-root spelling the fixture's ``SOURCES`` table
    uses; the mount the harness imports from is that path minus its leading
    ``SciGraphs/``, so the two trees are joined here rather than in the table.
    A path that does not resolve raises ``FileNotFoundError``: a digest that
    could not be read is not provenance, and a silently missing one is exactly
    the gap these digests close.
    """
    prefix, _, tail = relative_path.partition("/")
    if prefix == "SciGraphs":
        relative_path = tail
    with open(os.path.join(root, relative_path), "rb") as stream:
        return hashlib.sha256(stream.read()).hexdigest()


def digest_of(relative_path):
    """``source_digest`` against whichever root ``scigraphs_setup`` was given."""
    return source_digest(_STATE["root"], relative_path)


#: (path under the SciGraphs root, human name) of the checked-in version
#: declarations. There is no VERSION file; these two are the version metadata
#: that ships next to the code they version, so reading them needs neither the
#: network nor git.
VERSION_FILES = (
    ("blender_manifest.toml", "add-on"),
    ("core/pyproject.toml", "core"),
)
_VERSION_LINE = re.compile(r"^version\s*=\s*\"([^\"]+)\"", re.MULTILINE)


def scigraphs_revision(root=None):
    """Declared package versions of the tree the numbers came from.

    *root* defaults to the root ``scigraphs_setup`` installed. The version is
    read with one anchored regex rather than ``tomllib`` because the two files
    put it at two different depths (``version`` at the top level of the add-on
    manifest, ``[project].version`` in the core metadata); anchoring to the
    start of a line also keeps ``schema_version`` from matching. A file that is
    absent or unparsable is reported as ``None`` rather than raising, so a
    relocation of the metadata cannot stop the fixture being emitted.
    """
    base = _STATE["root"] if root is None else root
    found = {}
    for relative_path, name in VERSION_FILES:
        match = _VERSION_LINE.search(_read_text(os.path.join(base, relative_path)))
        found[name] = match.group(1) if match else None
    return found


def _read_text(path):
    try:
        with open(path, encoding="utf-8") as stream:
            return stream.read()
    except OSError:
        return ""


def layout_seed():
    """``derive_seed(42, "layout")``, the seed SPRING_3D is handed."""
    return int(_STATE["determinism"].get_layout_seed())


def build_graph():
    """``nx.les_miserables_graph()`` relabelled to 0..n-1 in the generator's order."""
    nx = _STATE["nx"]
    source = nx.les_miserables_graph()
    labels = tuple(source.nodes())
    order = {name: index for index, name in enumerate(labels)}
    graph = nx.Graph()
    graph.add_nodes_from(range(len(labels)))
    graph.add_edges_from((order[u], order[v]) for u, v in source.edges())
    return Lesmis(labels, order, graph, tuple(source.edges()))


def canonical_edges(lesmis, raw=None):
    """``geometry.py:275-284``: (min, max), self-loops dropped, first occurrence.

    *raw* defaults to the generator's own edge list; the test passes its own to
    exercise the duplicate, reversal and self-loop branches.
    """
    pairs = lesmis.raw_edges if raw is None else tuple(raw)
    size = np.int64(len(lesmis.labels))
    src = np.fromiter((lesmis.order[u] for u, _ in pairs), dtype=np.int64, count=len(pairs))
    tgt = np.fromiter((lesmis.order[v] for _, v in pairs), dtype=np.int64, count=len(pairs))
    known = (src >= 0) & (tgt >= 0)
    keep = known & (src != tgt)
    lo = np.minimum(src[keep], tgt[keep])
    hi = np.maximum(src[keep], tgt[keep])
    _unique, first = np.unique(lo * size + hi, return_index=True)
    first.sort()
    return [[int(lo[i]), int(hi[i])] for i in first]


def spring3d(lesmis, iterations=LAYOUT_ITERATIONS, scale=LAYOUT_SCALE):
    """SPRING_3D through SciGraphs' own wrapper, after its own RNG reset."""
    _STATE["common"]._reset_layout_rng()
    positions = _STATE["layouts"]._spring_layout_3d(lesmis.graph, iterations, scale)
    return np.asarray(positions, dtype=np.float64)


def node_betweenness(lesmis):
    """One normalised betweenness per dense index, in node order."""
    centrality = _STATE["nx"].betweenness_centrality(lesmis.graph)
    return [float(centrality[i]) for i in range(len(lesmis.labels))]


def rank_t(values):
    """The RANK-normalised colour coordinate: mid-ranks over ``n - 1``."""
    norm, _finite, _plan = _STATE["normalize_values"](values, mode=COLOR_NORM)
    return [float(v) for v in norm]
