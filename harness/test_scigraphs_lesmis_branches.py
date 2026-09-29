"""Branch coverage for the SciGraphs fixture's math, on synthetic inputs.

Each test drives one branch of ``scigraphs_lesmis_camera`` and
``scigraphs_lesmis_fixture`` that the real 77-node graph does not reach, so a
swapped operator or constant cannot hide behind the happy path.
"""

import dataclasses
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import scigraphs_lesmis_camera as cam
import scigraphs_lesmis_fixture as fix


def points(*triples):
    return [tuple(float(v) for v in t) for t in triples]


def cross(a, b):
    return (a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0])


class WorldBounds(unittest.TestCase):
    def test_centre_and_radius_are_half_the_bbox_diagonal(self):
        centre, radius, lo, hi = cam.world_bounds(points((0, 0, 0), (2, 4, 6)))
        self.assertEqual((1.0, 2.0, 3.0), centre)
        self.assertEqual((0.0, 0.0, 0.0), lo)
        self.assertEqual((2.0, 4.0, 6.0), hi)
        self.assertEqual(0.5 * (2.0 ** 2 + 4.0 ** 2 + 6.0 ** 2) ** 0.5, radius)

    def test_a_single_point_clamps_the_radius_to_its_floor(self):
        _centre, radius, _lo, _hi = cam.world_bounds(points((5, 5, 5)))
        self.assertEqual(cam.RADIUS_FLOOR, radius)

    def test_a_negative_extent_uses_its_own_diagonal(self):
        _centre, radius, _lo, _hi = cam.world_bounds(points((-1, 0, 0), (1, 0, 0)))
        self.assertEqual(1.0, radius)


class CameraBasis(unittest.TestCase):
    def test_the_three_axes_are_mutually_orthogonal(self):
        forward, right, up = cam.camera_basis(cam.DEFAULT_DIRECTION)
        for a, b in ((forward, up), (forward, right), (right, up)):
            self.assertAlmostEqual(0.0, sum(x * y for x, y in zip(a, b)), places=15)

    def test_up_is_the_forward_to_right_cross_and_right_is_unit(self):
        forward, right, up = cam.camera_basis(cam.DEFAULT_DIRECTION)
        self.assertEqual(up, cross(forward, right))
        self.assertAlmostEqual(1.0, sum(v * v for v in right) ** 0.5, places=15)
        self.assertAlmostEqual(1.0, sum(v * v for v in forward) ** 0.5, places=15)

    def test_a_forward_along_the_world_up_takes_the_180_degree_roll_fallback(self):
        self.assertEqual(((0.0, 0.0, 1.0), (-1.0, 0.0, 0.0), (0.0, -1.0, 0.0)),
                         cam.camera_basis(cam.WORLD_UP))

    def test_a_forward_against_the_world_up_takes_the_same_fallback(self):
        self.assertEqual(((0.0, 0.0, -1.0), (1.0, 0.0, 0.0), (0.0, -1.0, 0.0)),
                         cam.camera_basis(tuple(-v for v in cam.WORLD_UP)))


class FrameTangents(unittest.TestCase):
    def test_landscape_uses_the_horizontal_sensor(self):
        self.assertEqual((0.3, 0.16875), cam.frame_tangents(60.0, 36.0, 1920, 1080))

    def test_portrait_widens_the_vertical_tangent(self):
        tan_h, tan_v = cam.frame_tangents(60.0, 36.0, 1080, 1920)
        self.assertEqual(0.3, tan_h)
        self.assertEqual(0.5333333333333333, tan_v)

    def test_an_ultra_wide_lens_is_floored_not_divided_by_zero(self):
        tan_h, tan_v = cam.frame_tangents(1e9, 36.0, 1920, 1080)
        self.assertEqual(cam.TANGENT_FLOOR, tan_h)
        self.assertEqual(cam.TANGENT_FLOOR, tan_v)


class FramingDistance(unittest.TestCase):
    def test_the_distance_covers_the_widest_offset_after_the_margin(self):
        view = cam.frame_camera(points((-5, 0, 0), (5, 0, 0)), direction=(0.0, -1.0, 0.0))
        self.assertAlmostEqual(5.0 * cam.MARGIN / view.tan_h, view.distance, places=9)
        self.assertEqual(5.0, view.radius)

    def test_the_camera_sits_on_the_centre_plus_forward_times_distance(self):
        view = cam.frame_camera(points((-5, 0, 0), (5, 0, 0)), direction=(0.0, -1.0, 0.0))
        expected = tuple(c + f * view.distance
                         for c, f in zip(view.centre, view.forward))
        self.assertEqual(expected, view.location)

    def test_a_collapsed_graph_still_gets_the_epsilon_floor(self):
        view = cam.frame_camera(points((1, 1, 1), (1, 1, 1)), direction=(0.0, 0.0, 1.0))
        self.assertEqual(cam.RADIUS_FLOOR * 1e-3, view.distance)


class Projection(unittest.TestCase):
    def setUp(self):
        self.view = cam.frame_camera(points((-1, -1, 0), (1, 1, 0)),
                                     direction=(0.0, -1.0, 0.0))
        self.centre = cam.project([self.view.centre], self.view)

    def test_the_centre_lands_in_the_middle_of_the_frame(self):
        self.assertAlmostEqual(960.0, self.centre.x[0], places=9)
        self.assertAlmostEqual(540.0, self.centre.y[0], places=9)
        self.assertEqual(self.view.distance, self.centre.depth[0])
        self.assertTrue(self.centre.visible[0])

    def test_y_is_flipped_for_image_coordinates(self):
        above = cam.project([(0.0, 0.0, 1.0)], self.view)
        below = cam.project([(0.0, 0.0, -1.0)], self.view)
        self.assertLess(above.y[0], below.y[0])
        self.assertAlmostEqual(960.0, above.x[0], places=9)

    def test_a_point_behind_the_camera_is_invisible(self):
        behind = [tuple(c + 2.0 * self.view.distance * f
                        for c, f in zip(self.view.centre, self.view.forward))]
        self.assertFalse(cam.project(behind, self.view).visible[0])

    def test_a_point_far_off_axis_is_outside_the_frame(self):
        self.assertFalse(cam.project([(1e4, 0.0, 0.0)], self.view).visible[0])

    def test_depth_is_the_view_axis_component_not_the_euclidean_one(self):
        near = cam.project([(1.0, -1.0, 0.0)], self.view)
        far = cam.project([(1.0, 1.0, 0.0)], self.view)
        self.assertLess(near.depth[0], far.depth[0])
        self.assertLess(near.distance[0], far.distance[0])
        self.assertGreater(near.distance[0] - near.depth[0], 0.0)
        self.assertAlmostEqual(3.733333333333333, near.depth[0], places=12)


class Occlusion(unittest.TestCase):
    def setUp(self):
        self.view = cam.frame_camera(points((-5, 0, 0), (5, 0, 0)),
                                     direction=(0.0, 1.0, 0.0))
        self.near = (0.0, 15.0, 0.0)
        self.far = (0.0, 10.0, 0.0)

    def test_a_ray_that_misses_every_glyph_reports_no_hit(self):
        self.assertIsNone(cam.ray_hits((0.0, 0.0, 0.0), (0.0, 0.0, 1.0),
                                       [(5.0, 5.0, 5.0)], 1.0))

    def test_a_ray_reports_the_near_surface_of_the_glyph_it_hits(self):
        hit = cam.ray_hits((0.0, 0.0, 0.0), (0.0, 0.0, 1.0), [(0.0, 0.0, 10.0)], 2.0)
        self.assertEqual(8.0, hit)

    def test_a_glyph_behind_the_ray_origin_does_not_count(self):
        self.assertIsNone(cam.ray_hits((0.0, 0.0, 0.0), (0.0, 0.0, 1.0),
                                       [(0.0, 0.0, -10.0)], 2.0))

    def test_a_node_alone_in_front_of_the_camera_is_not_self_occluded(self):
        self.assertFalse(self._occluded([self.view.centre])[0])

    def test_a_glyph_on_the_ray_occludes_the_node_behind_it(self):
        verdicts = self._occluded([self.far, self.near])
        self.assertTrue(verdicts[0])
        self.assertFalse(verdicts[1])

    def test_a_glyph_further_off_the_ray_than_its_radius_does_not_occlude(self):
        target = (0.0, 10.0, 0.0)
        clear = [target, (0.0, 15.0, 1.0)]
        self.assertFalse(self._occluded(clear)[0])

    def test_a_glyph_inside_its_own_radius_of_the_ray_does_occlude(self):
        target = (0.0, 10.0, 0.0)
        grazing = cam.NODE_RADIUS_REL * self.view.radius * 0.5
        self.assertTrue(self._occluded([target, (0.0, 15.0, grazing)])[0])

    def _occluded(self, pts):
        return cam.occluded_mask(cam.project(pts, self.view), pts, self.view).occluded

    def test_an_invisible_node_is_occluded_without_a_raycast(self):
        behind = [tuple(c + 2.0 * self.view.distance * f
                        for c, f in zip(self.view.centre, self.view.forward))]
        self.assertTrue(self._occluded(behind)[0])

    def test_the_tolerance_floors_at_a_tenth(self):
        small = cam.frame_camera(points((-1, 0, 0), (1, 0, 0)), direction=(0.0, 1.0, 0.0))
        self.assertEqual(cam.OCCLUSION_TOLERANCE_FLOOR, small.occlusion_tolerance)

    def test_the_tolerance_scales_with_the_glyph_above_the_floor(self):
        grown = dataclasses.replace(self.view, radius=100.0)
        self.assertEqual(100.0 * cam.NODE_RADIUS_REL * cam.OCCLUSION_TOLERANCE_FACTOR,
                         grown.occlusion_tolerance)


class LabelSelection(unittest.TestCase):
    def _candidates(self):
        return [fix.Candidate(0, "A", 100.0, 100.0), fix.Candidate(1, "B", 400.0, 100.0)]

    def test_declutter_keeps_two_boxes_that_do_not_overlap(self):
        self.assertEqual([0, 1], [c.index for c in fix.declutter(self._candidates(), 26)])

    def test_declutter_drops_a_box_that_overlaps_an_accepted_one(self):
        near = [fix.Candidate(0, "A", 100.0, 100.0), fix.Candidate(1, "B", 110.0, 100.0)]
        self.assertEqual([0], [c.index for c in fix.declutter(near, 26)])

    def test_declutter_drops_a_box_that_overlaps_vertically_only(self):
        near = [fix.Candidate(0, "A", 100.0, 100.0), fix.Candidate(1, "B", 100.0, 120.0)]
        self.assertEqual([0], [c.index for c in fix.declutter(near, 26)])

    def test_declutter_preserves_the_order_it_is_given(self):
        self.assertEqual([1, 0],
                         [c.index for c in fix.declutter(list(reversed(self._candidates())), 26)])

    def test_declutter_half_extents_follow_the_reference_constants(self):
        self.assertEqual(0.30 * 26 * 1, 0.30 * 26 * max(len("A"), 1))
        overlap = [fix.Candidate(0, "A" * 4, 0.0, 0.0), fix.Candidate(1, "B" * 4, 40.0, 0.0)]
        self.assertEqual([0], [c.index for c in fix.declutter(overlap, 26)])

    def test_max_count_truncates_and_zero_means_no_limit(self):
        cands = [fix.Candidate(i, "N%d" % i, float(i) * 1000.0, 100.0) for i in range(5)]
        self.assertEqual([0, 1], [c.index for c in fix.keep_labels(cands, 2)])
        self.assertEqual([0, 1, 2, 3, 4], [c.index for c in fix.keep_labels(cands, 0)])

    def test_candidates_rank_by_value_descending_and_break_ties_by_index(self):
        projected = _FakeProjection(visible=[True, True, True, False])
        ranked = fix.label_candidates(("A", "B", "C", "D"), (1.0, 3.0, 3.0, 9.0), projected)
        self.assertEqual([1, 2, 0], [c.index for c in ranked])

    def test_candidates_skip_occluded_and_invisible_nodes(self):
        projected = _FakeProjection(visible=[True, False], occluded=[True, False])
        ranked = fix.label_candidates(("A", "B"), (5.0, 9.0), projected)
        self.assertEqual([], [c.index for c in ranked])

    def test_fig6_overlap_partitions_both_name_sets(self):
        overlap, only_ours, only_fig6 = fix.fig6_overlap(("A", "B"), [0, 1],
                                                         fig6=("A", "C"))
        self.assertEqual({"A"}, overlap)
        self.assertEqual({"B"}, only_ours)
        self.assertEqual({"C"}, only_fig6)


class _FakeProjection:
    """The four fields ``label_candidates`` reads, nothing else."""

    def __init__(self, visible, occluded=None):
        self.visible = visible
        self.occluded = occluded if occluded is not None else [False] * len(visible)
        self.x = [0.0] * len(visible)
        self.y = [0.0] * len(visible)


if __name__ == "__main__":
    unittest.main()
