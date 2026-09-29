"""The SciGraphs fig1/fig6 camera, ported from bpy-dependent source.

Sources, all read-only references under the SciGraphs tree:

* ``SciGraphs/core/repro/executor.py:606-650`` — ``_graph_bounds``: the world
  centre and the radius ``R`` (half the bounding-box diagonal).
* ``SciGraphs/core/repro/executor.py:652-744`` — ``_frame_camera``: the aim
  basis, the fit distance with its margin, and the perspective tangents.
* ``SciGraphs/core/visualization/text_overlay.py:139-247`` —
  ``project_nodes_to_screen``: the sensor-plane projection to pixels.
* ``SciGraphs/core/visualization/text_overlay.py:250-299`` —
  ``test_depth_occlusion``: the ray, the origin offset and the tolerance.
* ``SciGraphs/core/repro/executor.py:285-307`` — the ``*_radius_rel``
  multipliers that turn ``R`` into the node and edge glyph radii.

PONYTAIL: ``bpy`` and ``mathutils`` are not importable here, so two things are
rebuilt rather than called. The bounding box is taken over the node point cloud
alone; Blender measures the *evaluated* mesh, which also encloses the instanced
glyphs and the edge curves, so its ``R`` is larger and its centre can sit up to
one glyph radius away from ours. And ``Vector.to_track_quat('Z', 'Y')`` is
rebuilt as an explicit orthonormal frame around world up ``+Z``; the reference
tree's own comment on that call ("track_quat for +Z rolls 180 degrees", see
``SciGraphs/api/render.py:677``) is the evidence for the degenerate case, which
is taken to be the 180-degree roll.
"""

import dataclasses
import math

DEFAULT_DIRECTION = (0.48, -0.72, 0.50)
WORLD_UP = (0.0, 0.0, 1.0)
RES_X, RES_Y = 1920, 1080
LENS_MM, SENSOR_MM, MARGIN = 60.0, 36.0, 1.12
NODE_RADIUS_REL, EDGE_RADIUS_REL = 0.022, 0.0035
OCCLUSION_TOLERANCE_FLOOR = 0.1
OCCLUSION_TOLERANCE_FACTOR = 1.5
RAY_ORIGIN_OFFSET = 0.01
RADIUS_FLOOR = 1e-6
RADIUS_DISTANCE_EPS = 1e-3
TANGENT_FLOOR = 1e-6
PARALLEL_EPS = 1e-12


@dataclasses.dataclass(frozen=True)
class View:
    """Everything the projection needs, resolved once by ``frame_camera``."""

    centre: tuple
    forward: tuple
    right: tuple
    up: tuple
    tan_h: float
    tan_v: float
    lens: float
    sensor: float
    res_x: int
    res_y: int
    margin: float
    distance: float
    location: tuple
    radius: float

    @property
    def node_radius(self):
        return self.radius * NODE_RADIUS_REL

    @property
    def edge_radius(self):
        return self.radius * EDGE_RADIUS_REL

    @property
    def occlusion_tolerance(self):
        return max(OCCLUSION_TOLERANCE_FLOOR,
                   self.node_radius * OCCLUSION_TOLERANCE_FACTOR)


@dataclasses.dataclass(frozen=True)
class Projected:
    """Per-node screen coordinates and the two label filters' verdicts."""

    x: tuple
    y: tuple
    depth: tuple
    distance: tuple
    visible: tuple
    occluded: tuple


def _dot(a, b):
    return sum(x * y for x, y in zip(a, b))


def _cross(a, b):
    return (a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0])


def _sub(a, b):
    return tuple(x - y for x, y in zip(a, b))


def world_bounds(points):
    """``_graph_bounds``' centre, radius and box, over the node positions."""
    axes = list(zip(*points))
    lo = tuple(float(min(axis)) for axis in axes)
    hi = tuple(float(max(axis)) for axis in axes)
    centre = tuple((a + b) * 0.5 for a, b in zip(lo, hi))
    diagonal = math.sqrt(_dot(_sub(hi, lo), _sub(hi, lo)))
    return centre, max(0.5 * diagonal, RADIUS_FLOOR), lo, hi


def camera_basis(direction):
    """``to_track_quat('Z', 'Y')`` as an explicit (forward, right, up) frame."""
    forward = tuple(float(v) for v in direction)
    length = math.sqrt(_dot(forward, forward))
    forward = tuple(v / length for v in forward)
    side = _cross(WORLD_UP, forward)
    if math.sqrt(_dot(side, side)) < PARALLEL_EPS:
        right = (-1.0, 0.0, 0.0) if forward[2] > 0.0 else (1.0, 0.0, 0.0)
    else:
        side_len = math.sqrt(_dot(side, side))
        right = tuple(v / side_len for v in side)
    return forward, right, _cross(forward, right)


def frame_tangents(lens, sensor, res_x, res_y):
    """The two half-frame tangents, floored as ``_frame_camera`` floors them."""
    half_h = math.atan((sensor * 0.5) / max(lens, 1e-6))
    half_v = math.atan(math.tan(half_h) * res_y / res_x)
    return (max(math.tan(half_h), TANGENT_FLOOR),
            max(math.tan(half_v), TANGENT_FLOOR))


def frame_camera(points, direction=DEFAULT_DIRECTION):
    """Centre, basis and the fit distance that puts the graph in the frame."""
    centre, radius, _lo, _hi = world_bounds(points)
    forward, right, up = camera_basis(direction)
    tan_h, tan_v = frame_tangents(LENS_MM, SENSOR_MM, RES_X, RES_Y)
    view = View(centre=centre, forward=forward, right=right, up=up, tan_h=tan_h,
                tan_v=tan_v, lens=LENS_MM, sensor=SENSOR_MM, res_x=RES_X,
                res_y=RES_Y, margin=MARGIN, distance=0.0, location=centre,
                radius=radius)
    distance = _fit_distance(points, view)
    location = tuple(float(c + f * distance) for c, f in zip(centre, forward))
    return dataclasses.replace(view, distance=float(distance), location=location)


def _fit_distance(points, view):
    """The largest ``depth + margin * |offset| / tan`` over the vertex cloud."""
    distance = 0.0
    for point in points:
        offset = _sub(point, view.centre)
        depth = _dot(offset, view.forward)
        need_h = depth + abs(_dot(offset, view.right)) * view.margin / view.tan_h
        need_v = depth + abs(_dot(offset, view.up)) * view.margin / view.tan_v
        distance = max(distance, need_h, need_v)
    return max(distance, view.radius * RADIUS_DISTANCE_EPS)


def project(points, view):
    """Sensor-plane projection to pixels, then the depth-occlusion test."""
    return occluded_mask(_project_points(points, view), points, view)


def _project_points(points, view):
    half_sensor = view.sensor / 2.0
    aspect = view.res_x / view.res_y
    xs, ys, depths, distances, visible = [], [], [], [], []
    for point in points:
        offset = _sub(tuple(float(v) for v in point), view.location)
        distance = math.sqrt(_dot(offset, offset))
        cam_z = _dot(offset, view.forward)
        if cam_z >= 0.0:
            xs.append(0.0)
            ys.append(0.0)
            depths.append(0.0)
            distances.append(distance)
            visible.append(False)
            continue
        depth = -cam_z
        proj_x = _dot(offset, view.right) * view.lens / (depth * half_sensor)
        proj_y = _dot(offset, view.up) * view.lens / (depth * half_sensor) * aspect
        norm_x = (proj_x + 1.0) / 2.0
        norm_y = (proj_y + 1.0) / 2.0
        xs.append(norm_x * view.res_x)
        ys.append((1.0 - norm_y) * view.res_y)
        depths.append(depth)
        distances.append(distance)
        visible.append(0.0 <= norm_x <= 1.0 and 0.0 <= norm_y <= 1.0)
    return Projected(tuple(xs), tuple(ys), tuple(depths), tuple(distances),
                     tuple(visible), tuple(False for _ in points))


def ray_hits(origin, direction, points, radius):
    """Distance to the nearest glyph sphere on the ray, or None if it misses."""
    best = None
    for point in points:
        offset = _sub(tuple(float(v) for v in point), origin)
        along = _dot(offset, direction)
        disc = along * along - (_dot(offset, offset) - radius * radius)
        if disc < 0.0:
            continue
        near = along - math.sqrt(disc)
        if near <= 0.0:
            continue
        if best is None or near < best:
            best = near
    return best


def occluded_mask(projected, points, view):
    """``test_depth_occlusion``: a nearer hit than the node, less tolerance."""
    tolerance = view.occlusion_tolerance
    verdicts = []
    for index, point in enumerate(points):
        if not projected.visible[index]:
            verdicts.append(True)
            continue
        aim = _sub(tuple(float(v) for v in point), view.location)
        length = math.sqrt(_dot(aim, aim))
        direction = tuple(v / length for v in aim)
        origin = tuple(o + d * RAY_ORIGIN_OFFSET
                       for o, d in zip(view.location, direction))
        hit = ray_hits(origin, direction, points, view.node_radius)
        verdicts.append(hit is not None and hit < (projected.distance[index] - tolerance))
    return dataclasses.replace(projected, occluded=tuple(verdicts))
