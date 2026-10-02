"""The 32 names, which arm holds each one's reference, and the call that gets it.

Two arms, and the split is not ours: SciGraphs reaches Graphviz through
`scigraphs_utils.graphviz_layout` (`yifan_hu.py:278-279`, `:366`), and `scigraphs_utils` is
not in the `ge-python-oracle` image — so those nine names take the engine's own `-Tplain`
points from `ge-graphviz-oracle` instead, exactly the arm `harness/oracle-graphviz.py`
already drives. Every row says which arm it used, so a reader never has to guess which image
produced a number.

**`YIFAN_HU` is in the Graphviz arm, and that is a finding rather than a placement.** SciGraphs
runs it through the same sfdp call as `GRAPHVIZ_SFDP` (`yifan_hu.py:344`, `:366`), so the two
rows compare two different motor layouts against one reference. What SciGraphs adds on top
in mode `2Z` — a spectral z at `0.3 * scale` (`yifan_hu.py:344-353`) — the plain points do not
carry, and the matrix records that as the row's convention gap.
"""

# `apply_graph_layout`'s own defaults (`dispatcher.py:14`).
ITERATIONS = 50
SCALE = 5.0

# `get_layout_seed()` with no pipeline seed: `derive_seed(42, "layout")`
# (`repro/determinism.py:124-129`, `:56-62`) is 981798123. Passed as the Graphviz arm's
# `-Gstart`, which is the one seed a Graphviz engine takes from us.
LAYOUT_SEED = 981798123

#: The 23 names `ge-python-oracle` runs through `apply_graph_layout` itself, in the
#: dispatcher's own order.
SCIGRAPHS_NAMES = [
    "RANDOM",
    "GRID",
    "SPRING",
    "SPRING_3D",
    "CIRCLE_PACKING",
    "FORCEATLAS2",
    "IGRAPH_FR",
    "IGRAPH_KK",
    "IGRAPH_DRL",
    "IGRAPH_DRL_2D",
    "IGRAPH_LGL",
    "SPHERE",
    "SPECTRAL_3D",
    "SPIRAL_3D",
    "HELIX",
    "CUBE",
    "HIERARCHICAL_3D",
    "BIPARTITE_3D",
    "IGRAPH_DH",
    "IGRAPH_GRAPHOPT",
    "MDS_3D",
    "SUGIYAMA",
    "CIRCULAR_HIERARCHY",
]

#: The nine names the Graphviz arm reaches, as `(name, engine)`. The engines are the ones
#: `GRAPHVIZ_ENGINES` maps each name to (`yifan_hu.py:7-16`).
GRAPHVIZ_ROWS = [
    ("YIFAN_HU", "sfdp"),
    ("GRAPHVIZ_DOT", "dot"),
    ("GRAPHVIZ_NEATO", "neato"),
    ("GRAPHVIZ_FDP", "fdp"),
    ("GRAPHVIZ_SFDP", "sfdp"),
    ("GRAPHVIZ_TWOPI", "twopi"),
    ("GRAPHVIZ_CIRCO", "circo"),
    ("GRAPHVIZ_OSAGE", "osage"),
    ("GRAPHVIZ_PATCHWORK", "patchwork"),
]

#: Every name, in the dispatcher's order, so a caller can walk the matrix in one pass.
ALL_NAMES = [
    "RANDOM", "GRID", "SPRING", "SPRING_3D", "CIRCLE_PACKING", "FORCEATLAS2",
    "IGRAPH_FR", "IGRAPH_KK", "IGRAPH_DRL", "IGRAPH_DRL_2D", "IGRAPH_LGL", "SPHERE",
    "SPECTRAL_3D", "SPIRAL_3D", "HELIX", "CUBE", "HIERARCHICAL_3D", "BIPARTITE_3D",
    "IGRAPH_DH", "IGRAPH_GRAPHOPT", "MDS_3D", "YIFAN_HU", "GRAPHVIZ_DOT",
    "GRAPHVIZ_NEATO", "GRAPHVIZ_FDP", "GRAPHVIZ_SFDP", "GRAPHVIZ_TWOPI",
    "GRAPHVIZ_CIRCO", "GRAPHVIZ_OSAGE", "GRAPHVIZ_PATCHWORK", "SUGIYAMA",
    "CIRCULAR_HIERARCHY",
]

#: The arm each name's reference came from, for `ref/<NAME>.json` and for the matrix.
ARMS = {name: "ge-python-oracle:scigraphs" for name in SCIGRAPHS_NAMES}
ARMS.update({name: "ge-graphviz-oracle:%s" % engine for name, engine in GRAPHVIZ_ROWS})


def libraries():
    """The versions the reference ran on, recorded per row.

    The point of recording them is that agreement here is against **one** pinned build: a
    different scipy would move the last digit of a spectral eigenvector and this file would
    quietly be measuring something else.
    """
    import networkx
    import numpy
    import scipy

    versions = {
        "numpy": numpy.__version__,
        "scipy": scipy.__version__,
        "networkx": networkx.__version__,
    }
    try:
        import igraph

        versions["igraph"] = igraph.__version__
    except ImportError:
        versions["igraph"] = "absent"
    return versions


def graphviz_version(engine):
    """The engine's own `-V`, which is the version the reference points came from."""
    import subprocess

    proc = subprocess.run([engine, "-V"], capture_output=True, text=True)
    return (proc.stdout + proc.stderr).strip().splitlines()[0] if (
        proc.stdout or proc.stderr
    ) else "unknown"
