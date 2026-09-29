"""Pins for the SciGraphs Les Miserables fixture's graph, layout and camera.

The exact values live in ``scigraphs_lesmis_pins``; this module drives them
against the pipeline, and the camera class recomputes the whole projection a
second time in numpy so the pins are a check rather than a snapshot.
Run in the oracle image; see ``harness/emit-scigraphs-lesmis.py``.
"""

import math
import os
import sys
import unittest

import numpy as np

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import scigraphs_lesmis_camera as cam
import scigraphs_lesmis_graph as gr
import scigraphs_lesmis_pins as pins


def setUpModule():
    gr.scigraphs_setup(pins.SCIGRAPHS_ROOT)


class GraphShape(unittest.TestCase):
    """The node relabelling and the edge canonicalisation."""

    def setUp(self):
        self.lesmis = gr.build_graph()

    def test_nodes_are_the_nx_generator_order_relabelled_from_zero(self):
        self.assertEqual(pins.NODE_COUNT, len(self.lesmis.labels))
        self.assertEqual(pins.NODE_COUNT, self.lesmis.graph.number_of_nodes())
        self.assertEqual(("Napoleon", "Myriel", "MlleBaptistine"), self.lesmis.labels[:3])
        self.assertEqual(("Child2", "Brujon", "MmeHucheloup"), self.lesmis.labels[-3:])
        self.assertEqual(list(range(pins.NODE_COUNT)), list(self.lesmis.graph.nodes()))

    def test_edges_are_min_max_first_occurrence_with_no_self_loops(self):
        edges = gr.canonical_edges(self.lesmis)
        self.assertEqual(pins.EDGE_COUNT, len(edges))
        self.assertEqual([[0, 1], [1, 2], [1, 3]], edges[:3])
        self.assertTrue(all(src < tgt for src, tgt in edges))
        self.assertEqual(pins.EDGE_COUNT, len({tuple(e) for e in edges}))
        self.assertEqual(2 * pins.EDGE_COUNT,
                         sum(dict(self.lesmis.graph.degree()).values()))

    def test_canonicalisation_drops_duplicates_reversals_and_loops(self):
        raw = [("Myriel", "Valjean"), ("Valjean", "Myriel"),
               ("Myriel", "Myriel"), ("Napoleon", "MmeBurgon")]
        self.assertEqual([[1, 10], [0, 46]], gr.canonical_edges(self.lesmis, raw))


class Layout(unittest.TestCase):
    """SPRING_3D through SciGraphs' own wrapper, seeded from determinism.py."""

    def test_layout_seed_is_derived_from_base_seed_42(self):
        self.assertEqual(pins.LAYOUT_SEED, gr.layout_seed())

    def test_positions_are_sciagraphs_spring_3d(self):
        world = gr.spring3d(gr.build_graph())
        self.assertEqual((pins.NODE_COUNT, 3), world.shape)
        self.assertTrue(np.isfinite(world).all())
        self.assertEqual(pins.WORLD_0, tuple(float(v) for v in world[0]))
        self.assertEqual(pins.WORLD_76, tuple(float(v) for v in world[76]))

    def test_layout_reads_the_same_through_a_list_or_an_array(self):
        world = gr.spring3d(gr.build_graph())
        as_list = [tuple(float(v) for v in row) for row in world.tolist()]
        self.assertEqual([pins.WORLD_0], [as_list[0]])
        self.assertEqual((pins.NODE_COUNT, 3), (len(as_list), 3))


class Metrics(unittest.TestCase):
    """Betweenness, and the RANK-normalised t derived from it."""

    def setUp(self):
        self.values = gr.node_betweenness(gr.build_graph())

    def test_betweenness_is_networkx_normalised(self):
        self.assertEqual(pins.NODE_COUNT, len(self.values))
        self.assertEqual(pins.BC_VALJEAN, self.values[pins.VALJEAN_ID])
        self.assertEqual(max(self.values), self.values[pins.VALJEAN_ID])
        self.assertTrue(all(0.0 <= v <= 1.0 for v in self.values))
        self.assertEqual(0.0, min(self.values))

    def test_rank_t_is_the_mid_rank_over_seventy_six(self):
        ts = gr.rank_t(self.values)
        expected = [rank / 76.0 for rank in pins.mid_ranks(self.values)]
        self.assertEqual(expected, ts)
        self.assertEqual(1.0, ts[pins.VALJEAN_ID])
        self.assertEqual(pins.T_NAPOLEON, ts[0])
        self.assertEqual(pins.ZERO_BETWEENNESS_NODES, self.values.count(0.0))
        self.assertEqual(pins.T_NAPOLEON, min(ts))
        self.assertEqual(1.0, max(ts))

    def test_rank_t_gives_ties_one_shared_mid_rank(self):
        self.assertEqual([2 / 3, 1 / 6, 1 / 6, 1.0], gr.rank_t([4.0, 1.0, 1.0, 9.0]))


class Camera(unittest.TestCase):
    """Framing, projection and the glyph radii derived from the bbox."""

    def setUp(self):
        self.world = gr.spring3d(gr.build_graph())
        self.view = cam.frame_camera(self.world)
        self.projected = cam.project(self.world, self.view)

    def test_bounds_and_radii_come_from_the_world_bbox(self):
        self.assertEqual(pins.CENTRE, tuple(self.view.centre))
        self.assertEqual(pins.R_RADIUS, self.view.radius)
        self.assertEqual(pins.NODE_RADIUS, pins.R_RADIUS * cam.NODE_RADIUS_REL)
        self.assertEqual(pins.EDGE_RADIUS, pins.R_RADIUS * cam.EDGE_RADIUS_REL)
        self.assertEqual(pins.NODE_RADIUS, self.view.node_radius)

    def test_basis_and_framing_distance(self):
        self.assertEqual(pins.FORWARD, self.view.forward)
        self.assertEqual(pins.RIGHT, self.view.right)
        self.assertEqual(pins.UP, self.view.up)
        self.assertEqual(pins.DISTANCE, self.view.distance)
        self.assertEqual(pins.LOCATION, self.view.location)
        self.assertEqual(pins.TAN_H, self.view.tan_h)
        self.assertEqual(pins.TAN_V, self.view.tan_v)

    def test_projection_puts_every_node_in_the_frame(self):
        self.assertEqual(pins.IN_FRAME, sum(self.projected.visible))
        self.assertEqual(pins.SCREEN_0, (self.projected.x[0], self.projected.y[0]))
        self.assertEqual(pins.SCREEN_76, (self.projected.x[76], self.projected.y[76]))
        self.assertEqual(pins.DEPTH_0, self.projected.depth[0])
        self.assertEqual(pins.DEPTH_76, self.projected.depth[76])

    def test_occlusion_uses_the_reference_tolerance_and_leaves_sixty_five(self):
        self.assertEqual(pins.TOLERANCE, self.view.occlusion_tolerance)
        self.assertEqual(pins.UNOCCLUDED, sum(not o for o in self.projected.occluded))

    def test_camera_matches_an_independent_recomputation(self):
        """The same formulas again, in numpy, vectorised over the whole cloud."""
        pts = np.asarray(self.world, dtype=float)
        lo, hi = pts.min(axis=0), pts.max(axis=0)
        centre = (lo + hi) * 0.5
        radius = 0.5 * math.sqrt(float(((hi - lo) ** 2).sum()))
        fwd = np.array(cam.DEFAULT_DIRECTION)
        fwd = fwd / np.linalg.norm(fwd)
        right = np.cross(np.array([0.0, 0.0, 1.0]), fwd)
        right = right / np.linalg.norm(right)
        up = np.cross(fwd, right)
        tan_h = math.tan(math.atan(36.0 * 0.5 / 60.0))
        tan_v = math.tan(math.atan(tan_h * 1080 / 1920))
        off = pts - centre
        depth = off @ fwd
        distance = max(float((depth + np.abs(off @ right) * 1.12 / tan_h).max()),
                       float((depth + np.abs(off @ up) * 1.12 / tan_v).max()),
                       radius * 1e-3)
        rel = pts - (centre + fwd * distance)
        d = -(rel @ fwd)
        px = ((rel @ right) * 60.0 / (d * 18.0) + 1.0) / 2.0 * 1920
        py = (1.0 - ((rel @ up) * 60.0 / (d * 18.0) * (1920 / 1080) + 1.0) / 2.0) * 1080
        self.assertLess(abs(radius - self.view.radius), 1e-12)
        self.assertLess(abs(distance - self.view.distance), 1e-12)
        for axis in range(3):
            self.assertLess(abs(centre[axis] - self.view.centre[axis]), 1e-12)
            self.assertLess(abs(fwd[axis] - self.view.forward[axis]), 1e-15)
            self.assertLess(abs(up[axis] - self.view.up[axis]), 1e-15)
        self.assertLess(float(np.abs(px - np.array(self.projected.x)).max()), 1e-9)
        self.assertLess(float(np.abs(py - np.array(self.projected.y)).max()), 1e-9)
        self.assertLess(float(np.abs(d - np.array(self.projected.depth)).max()), 1e-12)


if __name__ == "__main__":
    unittest.main()
