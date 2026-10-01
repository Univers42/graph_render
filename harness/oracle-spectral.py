"""Differential of layout.spectral, layout.mds.pivot and their two 3D arms
(layout.spectral.3d, layout.mds.pivot.3d) against SciGraphs' own networkx/scipy
implementations, run in the ge-python-oracle image (scipy 1.16.2):

  graph-cli emit-spectral-fixtures --seeds 1000
  docker run --rm -v $PWD:/w -v <SciGraphs>/core:/sg:ro -w /w ge-python-oracle \
      python3 harness/oracle-spectral.py /sg target/spectral-fixtures
  graph-cli oracle-spectral

Per connected component of at least 3 nodes, after centring away the packing offset:
  spectral   sine of the largest principal angle between our two coordinate columns and
             the reference's (eigenvectors are unique only up to sign and, in a
             degenerate eigenspace, rotation, so the spans are compared);
  pivot_mds  largest absolute difference of the peak-normalised, sign-pinned blocks (the
             algorithm is deterministic, so coordinates are compared).
A component whose eigenvalues leave the compared object undefined (a tie across the
2-column boundary for either layout; a tie inside the two columns for pivot_mds, whose
coordinates then compare as a span instead) is counted under "degenerate", not compared:
the reference itself returns an arbitrary rotation there. The result holds the worst
value per layout; graph-cli holds the ceilings.

Ponytail: sign ties (see column_gap) hide a true mirror in a column whose two largest
magnitudes tie. The reference seeds LOBPCG's extra start columns randomly, so the spectral
worst includes the reference's own run-to-run noise, and the comparison cannot see a
mirror or a rotation inside the 2D span. The gate model is one connected graph per seed,
so degenerate eigenspaces and disconnected components are not exercised here.
Ponytail (3D arms): the degeneracy thresholds are indexed by `dims` rather than fixed at
the 2D boundaries, so a component whose third eigenvalue ties is counted `degenerate`
rather than compared -- the reference itself returns an arbitrary rotation there. The
reference's `n < 4` random fallback is NOT emulated: our arms solve those components
instead, so a 3-node component is compared rather than skipped
(`docs/measurements/p12-t4a.md`).
"""
import hashlib, json, os, sys
import numpy as np

core, directory = sys.argv[1:3]
sys.path.insert(0, core)
import networkx as nx
import scipy
from scigraphs_core.mesh.layouts import networkx_layouts as ref

path = os.path.join(directory, "spectral.jsonl")
manifest = json.load(open(os.path.join(directory, "spectral-manifest.json")))
digest = hashlib.sha256(open(path, "rb").read()).hexdigest()
if digest != manifest["sha256"]["spectral.jsonl"]:
    sys.exit("spectral.jsonl does not match its manifest")


def centred(block):
    return block - block.mean(axis=0)


def worst_sine(a, b):
    qa, _ = np.linalg.qr(centred(a))
    qb, _ = np.linalg.qr(centred(b))
    cos = np.linalg.svd(qa.T @ qb, compute_uv=False)
    return float(np.sqrt(max(0.0, 1.0 - cos.min() ** 2)))


def peak_normalised(block):
    block = centred(block)
    return block / np.abs(block).max()


def column_gap(ours, theirs):
    """Largest column-wise coordinate difference. A column whose two largest magnitudes
    tie (symmetric graphs) has its sign fixed by rounding noise in either solver, so
    there either sign counts as agreement; every other column must match its sign."""
    worst = 0.0
    for j in range(theirs.shape[1]):
        a, b = ours[:, j], theirs[:, j]
        top = np.sort(np.abs(b))[-2:]
        same = float(np.abs(a - b).max())
        flipped = float(np.abs(a + b).max())
        worst = max(worst, min(same, flipped) if top[1] - top[0] <= 1e-9 * top[1] else same)
    return worst


GAP = 1e-6  # relative eigenvalue gap below which two eigenvalues count as tied


def tied(values, i):
    """Whether values[i] and values[i + 1] are within GAP of each other (values ascending)."""
    return values[i + 1] - values[i] <= GAP * max(abs(values).max(), 1e-12)


def laplacian_spectrum(graph, idx):
    sub = graph.subgraph([int(i) for i in idx])
    return np.linalg.eigvalsh(nx.laplacian_matrix(sub, nodelist=sorted(sub.nodes)).toarray().astype(float))


def pivot_spectrum(graph, idx, dims):
    """CtC's eigenvalues, ascending, captured from the reference's own eigh call."""
    seen = []
    real = np.linalg.eigh
    np.linalg.eigh = lambda a, *args, **kw: (seen.append(real(a, *args, **kw)[0]) or real(a, *args, **kw))
    try:
        nodes = sorted(int(i) for i in idx)
        ref._pivot_mds_coordinates(nx.adjacency_matrix(graph.subgraph(nodes), nodelist=nodes).astype(float).tocsr(), dims, ref._MDS_PIVOTS)
    finally:
        np.linalg.eigh = real
    return seen[0]


# key -> (ours column, the reference's own `dims` for it). The 3D arms are the reference's
# `_spectral_component_coordinates(G, 3)` and `_pivot_mds_component_coordinates(G, 3, _MDS_PIVOTS)`
# (networkx_layouts.py:260,286), which is the only thing that makes them a third column.
ARMS = {
    "spectral": ("spectral", 2),
    "pivot_mds": ("pivot_mds", 2),
    "spectral_3d": ("spectral_3d", 3),
    "pivot_mds_3d": ("pivot_mds_3d", 3),
}


def block(case, key):
    """A case's columns as an (n, d) float array: xy, or xyz when the layout is 3D."""
    ours = case[key]
    columns = [ours["x"], ours["y"]]
    if "z" in ours:
        columns.append(ours["z"])
    return np.column_stack(columns).astype(float)


layouts = {
    key: {"cases": 0, "degenerate": 0, "worst": 0.0} for key in ARMS
}
for text in open(path):
    case = json.loads(text)
    graph = nx.Graph()
    graph.add_nodes_from(range(case["n"]))
    graph.add_edges_from((s, t) for s, t in zip(case["source"], case["target"]) if s != t)
    for key, (_, dims) in ARMS.items():
        ours = block(case, key)
        assert ours.shape[1] == dims, f"{key}: expected {dims} columns, got {ours.shape[1]}"
        if key.startswith("spectral"):
            theirs, comps = ref._spectral_component_coordinates(graph, dims)
        else:
            theirs, comps = ref._pivot_mds_component_coordinates(graph, dims, ref._MDS_PIVOTS)
        for idx in comps:
            if len(idx) < 3:
                continue
            # The degeneracy index is the eigenvalue just PAST the last one kept. The
            # Laplacian spectrum carries the trivial 0 at index 0, so the `dims` kept
            # non-trivial pairs are values[1..=dims] and the boundary tie is
            # `tied(values, dims)`. At dims = 2 that is the 2D arm's own original
            # `len < 4 / tied(values, 2)`, unchanged; at dims = 3 it is `len < 5 /
            # tied(values, 3)`. Getting this wrong is silent: it admits a degenerate
            # component into the comparison, where the reference returns an arbitrary
            # rotation and the metric reads ~1.0 instead of failing.
            if key.startswith("spectral"):
                values = laplacian_spectrum(graph, idx)
                if len(values) < dims + 2 or tied(values, dims):
                    layouts[key]["degenerate"] += 1
                    continue
                metric = worst_sine(ours[idx], theirs[idx])
            else:
                values = pivot_spectrum(graph, idx, dims)[::-1]
                values = -values  # ascending on the negated spectrum: leading eigenvalues first
                # `values` here is the Gram spectrum with no trivial zero, so the kept
                # pairs are values[..dims] and the boundary is one earlier. At dims = 2
                # this is the 2D arm's own `len < 3 / tied(values, 1)` and `tied(values, 0)`.
                if len(values) < dims + 1 or tied(values, dims - 1):
                    layouts[key]["degenerate"] += 1
                    continue
                if tied(values, dims - 2):
                    metric = worst_sine(ours[idx], theirs[idx])
                else:
                    metric = column_gap(peak_normalised(ours[idx]), peak_normalised(theirs[idx]))
            layouts[key]["cases"] += 1
            layouts[key]["worst"] = max(layouts[key]["worst"], metric)

result = {
    "fingerprint": manifest["fingerprint"],
    "sha256": digest,
    "oracle": f"SciGraphs networkx_layouts on scipy {scipy.__version__}, numpy {np.__version__}",
    "layouts": layouts,
}
json.dump(result, open(os.path.join(directory, "spectral-result.json"), "w"), indent=1)
print(json.dumps(layouts))
