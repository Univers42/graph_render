"""Pins for the emitted ``fixtures/scigraphs/lesmis.json`` itself.

The document is the deliverable, so these are byte-level: the top-level keys,
every params entry, one node record in full, the edge list's shape, the label
set, and the guarantee that two builds agree.

The committed file is the artifact; the twelve structural tests read a freshly
emitted document, and ``test_the_committed_fixture_is_what_this_emitter_produces``
binds the two by opening the committed bytes. Without it this arm compares the
emitter against itself and a tampered artifact passes. Negative control: set
``nodes[0]["screen"]`` to ``[0.0, 0.0]`` in ``fixtures/scigraphs/lesmis.json``
and only that test fails. Regenerate with ``harness/emit-scigraphs-lesmis.py``;
see that file for the write.

Run it with:

    docker run --rm --pull never --user 0:0 -v "$PWD:/w" \\
      -v "$PWD/SciGraphs:/sg:ro" -w /w ge-python-oracle \\
      python3 -m unittest discover -s harness -p 'test_scigraphs_lesmis_document.py'
"""

import json
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import scigraphs_lesmis_camera as cam
import scigraphs_lesmis_fixture as fix
import scigraphs_lesmis_graph as gr
import scigraphs_lesmis_pins as pins

FIXTURE_PATH = os.path.normpath(os.path.join(
    os.path.dirname(os.path.abspath(__file__)),
    os.pardir, "fixtures", "scigraphs", "lesmis.json"))

# The gallery figure's own four stage counts, transcribed from the reference
# pipeline's reported losses as recorded in
# docs/decisions/scigraphs-reference-fixture.md:130-135. Three of the four are
# also our own pins; the unoccluded one is not (59 there, pins.UNOCCLUDED here)
# because the two point clouds differ, and nothing is tuned to close that gap.
GALLERY_UNOCCLUDED = 59


def setUpModule():
    gr.scigraphs_setup(pins.SCIGRAPHS_ROOT)


class Document(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.fixture_path = FIXTURE_PATH
        if not os.path.isfile(cls.fixture_path):
            raise AssertionError(
                "committed fixture is missing: %s" % cls.fixture_path)
        with open(cls.fixture_path, "rb") as handle:
            cls.committed = handle.read().decode("utf-8")
        cls.text = fix.emit_document(pins.SCIGRAPHS_ROOT)
        cls.doc = json.loads(cls.text)

    def test_the_committed_fixture_is_what_this_emitter_produces(self):
        self.assertEqual(self.committed, self.text)

    def test_top_level_keys(self):
        self.assertEqual(
            ["edges", "labels", "nodes", "params", "provenance", "radii"],
            sorted(self.doc),
        )

    def test_provenance_records_versions_seed_and_sources(self):
        prov = self.doc["provenance"]
        self.assertEqual("3.6", prov["networkx"])
        self.assertEqual(42, prov["seed"])
        self.assertEqual(pins.LAYOUT_SEED, prov["layout_seed"])
        self.assertTrue(all(":" in src for src in prov["sources"]))
        self.assertEqual(sorted(prov["sources"]), prov["sources"])
        self.assertTrue(prov["ponytail"])
        self.assertEqual(sorted(prov["ponytail"]), prov["ponytail"])

    def test_params_pin_the_figure_settings(self):
        params = self.doc["params"]
        self.assertEqual("SPRING_3D", params["layout_algorithm"])
        self.assertEqual(150, params["layout_iterations"])
        self.assertEqual(5.0, params["layout_scale"])
        self.assertEqual(3, params["layout_dim"])
        self.assertEqual("RANK", params["color_norm"])
        self.assertEqual([1920, 1080], params["resolution"])
        self.assertEqual([0.48, -0.72, 0.5], params["camera_direction"])
        self.assertEqual(pins.DISTANCE, params["camera_distance"])
        self.assertEqual(60.0, params["camera_lens_mm"])
        self.assertEqual(36.0, params["camera_sensor_mm"])
        self.assertEqual(1.12, params["camera_margin"])
        self.assertEqual([0, 0, 1], params["world_up"])
        self.assertEqual(0.022, params["node_radius_rel"])
        self.assertEqual(0.0035, params["edge_radius_rel"])
        self.assertEqual(pins.KEPT, params["label_max_count"])
        self.assertEqual(26, params["label_font_size"])
        self.assertEqual("betweenness", params["label_rank_by"])

    def test_radii(self):
        self.assertEqual({"R": pins.R_RADIUS, "edge": pins.EDGE_RADIUS,
                          "node": pins.NODE_RADIUS}, self.doc["radii"])

    def test_node_records_carry_every_field(self):
        node = self.doc["nodes"][0]
        self.assertEqual(["betweenness", "depth", "id", "label", "screen", "t", "world"],
                         sorted(node))
        self.assertEqual(0, node["id"])
        self.assertEqual("Napoleon", node["label"])
        self.assertEqual(list(pins.WORLD_0), node["world"])
        self.assertEqual(list(pins.SCREEN_0), node["screen"])
        self.assertEqual(pins.DEPTH_0, node["depth"])
        self.assertEqual(0.0, node["betweenness"])
        self.assertEqual(pins.T_NAPOLEON, node["t"])
        self.assertEqual(pins.NODE_COUNT, len(self.doc["nodes"]))

    def test_valjean_is_the_most_central_node(self):
        valjean = self.doc["nodes"][pins.VALJEAN_ID]
        self.assertEqual("Valjean", valjean["label"])
        self.assertEqual(pins.BC_VALJEAN, valjean["betweenness"])
        self.assertEqual(1.0, valjean["t"])
        self.assertEqual("Labarre", self.doc["nodes"][11]["label"])

    def test_edges_and_labels(self):
        self.assertEqual(pins.EDGE_COUNT, len(self.doc["edges"]))
        self.assertEqual([0, 1], self.doc["edges"][0])
        self.assertEqual(pins.LABEL_IDS, self.doc["labels"])
        by_id = {n["id"]: n["label"] for n in self.doc["nodes"]}
        self.assertEqual(pins.LABEL_NAMES, [by_id[i] for i in pins.LABEL_IDS])
        self.assertEqual(pins.KEPT, len(self.doc["labels"]))
        scores = [n["betweenness"] for n in self.doc["nodes"]]
        self.assertEqual(sorted((scores[i] for i in pins.LABEL_IDS), reverse=True),
                         [scores[i] for i in pins.LABEL_IDS])

    def test_every_label_is_in_frame_and_unoccluded(self):
        lesmis = gr.build_graph()
        world = gr.spring3d(lesmis)
        view = cam.frame_camera(world)
        projected = cam.project(world, view)
        candidates = fix.label_candidates(lesmis.labels, gr.node_betweenness(lesmis), projected)
        self.assertEqual(pins.UNOCCLUDED, len(candidates))
        for node_id in pins.LABEL_IDS:
            self.assertTrue(projected.visible[node_id])
            self.assertFalse(projected.occluded[node_id])
        self.assertEqual(pins.LABEL_IDS,
                         [c.index for c in fix.select_labels(candidates, 26, pins.KEPT)])

    def test_floats_are_written_repr_exact(self):
        self.assertIn("0.8670820583447432", self.text)
        self.assertIn("6.5874350307280265", self.text)
        self.assertNotIn("0.8670820583447,", self.text)
        self.assertNotIn("NaN", self.text)
        self.assertNotIn("Infinity", self.text)
        self.assertTrue(self.text.endswith("\n"))

    def test_two_builds_are_byte_identical(self):
        self.assertEqual(self.text, fix.emit_document(pins.SCIGRAPHS_ROOT))

    def test_fig6_overlap_is_reported_not_forced(self):
        names = gr.build_graph().labels
        overlap, only_ours, only_fig6 = fix.fig6_overlap(names, pins.LABEL_IDS)
        self.assertEqual(pins.OVERLAP, sorted(overlap))
        self.assertEqual(pins.ONLY_OURS, sorted(only_ours))
        self.assertEqual(pins.ONLY_FIG6, sorted(only_fig6))
        self.assertEqual(18, len(overlap) + len(only_fig6))
        self.assertEqual(pins.KEPT, len(overlap) + len(only_ours))

    def test_the_overlap_report_names_the_four_stages(self):
        """The report prints four counts, so build it from the pins, not literals.

        Our projected / in-frame / unoccluded / kept counts are
        ``pins.NODE_COUNT``, ``pins.IN_FRAME``, ``pins.UNOCCLUDED`` and
        ``pins.KEPT``, each already asserted from a recomputation elsewhere in
        this module, and the overlap total is ``len(pins.OVERLAP)`` out of the
        ``len(fix.FIG6_NAMES)`` names legible in the PNG. The gallery quadruple
        in the parenthetical is the reference figure's own log; three of its
        numbers coincide with our pins and are built from them, but its
        unoccluded count (``GALLERY_UNOCCLUDED``) has no origin inside this
        harness — it is a transcription, so this assertion pins the format and
        the three shared counts, not that one number.
        """
        scene = fix.make_scene(gr.build_graph())
        report = fix.overlap_report(scene)
        self.assertIn("labels: %d projected, %d in frame, %d unoccluded, %d kept"
                      % (pins.NODE_COUNT, pins.IN_FRAME, pins.UNOCCLUDED,
                         pins.KEPT), report)
        self.assertIn("overlap: %d of %d"
                      % (len(pins.OVERLAP), len(fix.FIG6_NAMES)), report)
        self.assertIn("gallery: %d / %d / %d / %d"
                      % (pins.NODE_COUNT, pins.IN_FRAME, GALLERY_UNOCCLUDED,
                         pins.KEPT), report)


if __name__ == "__main__":
    unittest.main()
