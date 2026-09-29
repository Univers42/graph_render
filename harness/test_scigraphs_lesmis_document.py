"""Pins for the emitted ``fixtures/scigraphs/lesmis.json`` itself.

The document is the deliverable, so these are byte-level: the top-level keys,
every params entry, one node record in full, the edge list's shape, the label
set, and the guarantee that two builds agree. Run in the oracle image; see
``harness/emit-scigraphs-lesmis.py``.
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


def setUpModule():
    gr.scigraphs_setup(pins.SCIGRAPHS_ROOT)


class Document(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.text = fix.emit_document(pins.SCIGRAPHS_ROOT)
        cls.doc = json.loads(cls.text)

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
        scene = fix.make_scene(gr.build_graph())
        report = fix.overlap_report(scene)
        self.assertIn("77 projected, 77 in frame, 65 unoccluded, 18 kept", report)
        self.assertIn("overlap: 8 of 18", report)
        self.assertIn("gallery: 77 / 77 / 59 / 18", report)


if __name__ == "__main__":
    unittest.main()
