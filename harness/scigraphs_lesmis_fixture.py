"""Assembly of ``fixtures/scigraphs/lesmis.json`` from the SciGraphs numbers.

The document is the reference data the studio needs to draw SciGraphs' own
Les Miserables gallery figure (``docs/assets/gallery/fig6_labels_lesmiserables.png``)
without running Blender: the generator's topology, SciGraphs' SPRING_3D
positions, its betweenness and the RANK-normalised colour coordinate, the
fig6 camera's 1920x1080 screen coordinates, the glyph radii, and the label set
its text overlay would have kept.

Label selection is ``executor.py:477-509`` (``_rank_labels``) in order:
in-frame, then not hidden behind geometry, then rank by betweenness descending
with ties left in node order, then ``declutter_labels``
(``text_overlay.py:302-326``) greedily in that order, then ``max_count``.
"""

import dataclasses
import json

import scigraphs_lesmis_camera as cam
import scigraphs_lesmis_graph as gr

FONT_SIZE = 26
MAX_COUNT = 18
RANK_BY = "betweenness"
BOX_HALF_HEIGHT_FACTOR = 0.62
BOX_HALF_WIDTH_FACTOR = 0.30

FIG6_NAMES = (
    "MmeBurgon", "Mabeuf", "Anzelma", "Bossuet", "Marius", "Fauchelevent",
    "Javert", "Cosette", "MmePontmercy", "Valjean", "Labarre", "Marguerite",
    "Gervais", "MlleBaptistine", "Myriel", "Geborand", "Champtercier",
    "Napoleon",
)

SOURCES = (
    "SciGraphs/SciGraphs/core/mesh/geometry.py:259-287",
    "SciGraphs/SciGraphs/core/repro/executor.py:285-307",
    "SciGraphs/SciGraphs/core/repro/executor.py:388-509",
    "SciGraphs/SciGraphs/core/repro/executor.py:606-650",
    "SciGraphs/SciGraphs/core/repro/executor.py:652-744",
    "SciGraphs/core/scigraphs_core/algorithms/analysis.py:149-153",
    "SciGraphs/core/scigraphs_core/algorithms/analysis.py:62-76",
    "SciGraphs/core/scigraphs_core/coloring/colormaps.py:462-469",
    "SciGraphs/core/scigraphs_core/coloring/colormaps.py:527-530",
    "SciGraphs/core/scigraphs_core/mesh/layouts/dispatcher.py:57-58",
    "SciGraphs/core/scigraphs_core/mesh/layouts/networkx_layouts.py:26-34",
    "SciGraphs/core/scigraphs_core/repro/determinism.py:56-62",
    "SciGraphs/SciGraphs/core/visualization/text_overlay.py:139-247",
    "SciGraphs/SciGraphs/core/visualization/text_overlay.py:250-299",
    "SciGraphs/SciGraphs/core/visualization/text_overlay.py:302-326",
    "SciGraphs/docs/examples/05-reproducible-pipeline.qmd:591-680",
)

PONYTAIL = (
    "The gallery figure's dataset is a GEXF that is in neither tree, so the node "
    "order here is nx.les_miserables_graph()'s and the layout starts from the "
    "seeded random positions rather than any coordinates the GEXF carried.",
    "Occluders are node glyph spheres of radius 0.022R. Blender's ray_cast also "
    "hits icosphere facets and the bezier edge curves; neither is modelled here.",
    "R is half the diagonal of the node point cloud's bbox. Blender measures the "
    "evaluated mesh, which also encloses the instanced glyphs and the edges, so "
    "its R is larger and its centre can be one glyph radius away.",
    "to_track_quat is a Blender binding; the aim frame is rebuilt around world up "
    "(0,0,1), the degenerate case being the 180-degree roll the reference notes.",
    "python-igraph is absent from the oracle image, so betweenness is "
    "nx.betweenness_centrality on SciGraphs' normalisation, not _igraph_betweenness.",
)


@dataclasses.dataclass(frozen=True)
class Candidate:
    """One node that survived in-frame and occlusion, with its screen position."""

    index: int
    name: str
    x: float
    y: float


@dataclasses.dataclass(frozen=True)
class Scene:
    """Every intermediate the document is assembled from, kept for the report."""

    lesmis: gr.Lesmis
    edges: tuple
    world: object
    view: cam.View
    projected: cam.Projected
    betweenness: tuple
    t: tuple
    candidates: tuple
    labels: tuple


def make_scene(lesmis):
    """Run the whole reference chain once, in SciGraphs' own order."""
    world = gr.spring3d(lesmis)
    view = cam.frame_camera(world)
    projected = cam.project(world, view)
    betweenness = gr.node_betweenness(lesmis)
    t = gr.rank_t(betweenness)
    candidates = label_candidates(lesmis.labels, betweenness, projected)
    kept = select_labels(candidates, FONT_SIZE, MAX_COUNT)
    return Scene(lesmis, tuple(gr.canonical_edges(lesmis)), world, view, projected,
                 tuple(betweenness), tuple(t), tuple(candidates),
                 tuple(c.index for c in kept))


def label_candidates(labels, values, projected):
    """In-frame, unoccluded nodes ranked by betweenness, ties left in node order."""
    survivors = [Candidate(index, labels[index], projected.x[index], projected.y[index])
                 for index in range(len(labels))
                 if projected.visible[index] and not projected.occluded[index]]
    survivors.sort(key=lambda c: -values[c.index])
    return survivors


def declutter(candidates, font_size=FONT_SIZE):
    """``declutter_labels``: greedy in the given order, estimated boxes."""
    half_h = max(font_size, 1) * BOX_HALF_HEIGHT_FACTOR
    accepted, boxes = [], []
    for candidate in candidates:
        half_w = BOX_HALF_WIDTH_FACTOR * font_size * max(len(candidate.name), 1)
        box = (candidate.x - half_w, candidate.y - half_h,
               candidate.x + half_w, candidate.y + half_h)
        if _overlaps(box, boxes):
            continue
        boxes.append(box)
        accepted.append(candidate)
    return accepted


def _overlaps(box, boxes):
    return any(box[0] < other[2] and other[0] < box[2]
               and box[1] < other[3] and other[1] < box[3] for other in boxes)


def keep_labels(candidates, max_count=MAX_COUNT):
    """``max_count``; zero or None means no limit, as the reference treats it."""
    if not max_count or max_count <= 0:
        return list(candidates)
    return list(candidates[:int(max_count)])


def select_labels(candidates, font_size=FONT_SIZE, max_count=MAX_COUNT):
    """The two steps the reference runs in this order, in one call."""
    return keep_labels(declutter(candidates, font_size), max_count)


def fig6_overlap(labels, label_ids, fig6=FIG6_NAMES):
    """Compare against the eighteen names legible in the gallery PNG."""
    ours = {labels[i] for i in label_ids}
    theirs = set(fig6)
    return ours & theirs, ours - theirs, theirs - ours


def build_document(scene):
    """The JSON-ready mapping. Floats stay ``float`` so ``repr`` round-trips."""
    return {
        "provenance": _provenance(),
        "params": _params(scene),
        "nodes": _nodes(scene),
        "edges": [list(pair) for pair in scene.edges],
        "labels": list(scene.labels),
        "radii": {"R": scene.view.radius, "node": scene.view.node_radius,
                  "edge": scene.view.edge_radius},
    }


def _provenance():
    networkx_version, numpy_version = gr.library_versions()
    return {"networkx": networkx_version, "numpy": numpy_version,
            "seed": gr.BASE_SEED, "layout_seed": gr.layout_seed(),
            "sources": sorted(SOURCES),
            "source_digests": _source_digests(),
            "scigraphs_revision": gr.scigraphs_revision(),
            "ponytail": sorted(PONYTAIL)}


def _source_digests():
    """``file:line`` -> the sha256 of the file that line range lives in.

    A ``file:line`` names a place, not a revision: it resolves to different code
    after any edit, and the line numbers here are not re-checked when SciGraphs
    moves, so the string alone cannot say which tree produced the numbers. The
    digest is the same file read now, so any later edit that moves the cited
    code shows up as a changed value. Sorted, so the key order is fixed rather
    than whatever the table happens to iterate in (D2).
    """
    return {source: gr.digest_of(source.split(":", 1)[0])
            for source in sorted(SOURCES)}


def _params(scene):
    _centre, _radius, lo, hi = cam.world_bounds([tuple(p) for p in scene.world])
    return {
        "graph": gr.GRAPH_GENERATOR,
        "betweenness": gr.BETWEENNESS_METHOD,
        "color_norm": gr.COLOR_NORM,
        "layout_algorithm": gr.LAYOUT_ALGORITHM,
        "layout_dim": gr.LAYOUT_DIM,
        "layout_iterations": gr.LAYOUT_ITERATIONS,
        "layout_scale": gr.LAYOUT_SCALE,
        "bbox_lo": list(lo), "bbox_hi": list(hi),
        "camera_type": "PERSP",
        "camera_direction": list(cam.DEFAULT_DIRECTION),
        "camera_forward": list(scene.view.forward),
        "camera_distance": scene.view.distance,
        "camera_lens_mm": cam.LENS_MM,
        "camera_sensor_mm": cam.SENSOR_MM,
        "camera_margin": cam.MARGIN,
        "world_up": list(cam.WORLD_UP),
        "resolution": [cam.RES_X, cam.RES_Y],
        "node_radius_rel": cam.NODE_RADIUS_REL,
        "edge_radius_rel": cam.EDGE_RADIUS_REL,
        "label_rank_by": RANK_BY,
        "label_font_size": FONT_SIZE,
        "label_max_count": MAX_COUNT,
        "label_occlusion": True,
        "label_declutter": True,
    }


def _nodes(scene):
    nodes = []
    for index, label in enumerate(scene.lesmis.labels):
        nodes.append({
            "id": index,
            "label": label,
            "world": [float(v) for v in scene.world[index]],
            "screen": [scene.projected.x[index], scene.projected.y[index]],
            "depth": scene.projected.depth[index],
            "betweenness": scene.betweenness[index],
            "t": scene.t[index],
        })
    return nodes


def dumps(document):
    """Sorted keys, two-space indent, repr-exact floats, one trailing newline.

    ``allow_nan=False`` is D9's Python face: a non-finite coordinate raises
    instead of emitting ``NaN``, which is not JSON.
    """
    return json.dumps(document, sort_keys=True, indent=2, allow_nan=False) + "\n"


def emit_document(root=gr.SCIGRAPHS_ROOT):
    """The whole chain, as the exact bytes written to the fixture file."""
    gr.scigraphs_setup(root)
    return dumps(build_document(make_scene(gr.build_graph())))


def overlap_report(scene):
    """The gallery comparison and the four stage counts, for stderr; never tuned."""
    overlap, only_ours, only_fig6 = fig6_overlap(scene.lesmis.labels, scene.labels)
    total = len(scene.lesmis.labels)
    in_frame = sum(1 for flag in scene.projected.visible if flag)
    return (
        "labels: %d projected, %d in frame, %d unoccluded, %d kept "
        "(gallery: 77 / 77 / 59 / 18)\n"
        "fig6 label overlap: %d of %d; only ours: %s; only fig6: %s\n"
        % (total, in_frame, len(scene.candidates), len(scene.labels), len(overlap),
           len(FIG6_NAMES), " ".join(sorted(only_ours)), " ".join(sorted(only_fig6))))
