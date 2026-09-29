"""The navigation rows: what each one claims, and how it is judged.

Every row is measured by the camera the served app is drawing with, or by the pixels the
served app has drawn. Nothing is dispatched through the studio's own API: the input goes in
as a mouse, a wheel or a key. A row that cannot be driven with CDP is reported NOT-RUN with
the reason, and `--break` makes the drag row expect a move the app does not make.
"""
import time

from drive import (
    ANCHOR_TOLERANCE, CENTRE, CLAMP_PRESSES, DRAG_TOLERANCE, OFF_CENTRE, PINCH_NOTCH, SETTLE_S,
    WHEEL_NOTCH, apart, screen_to_world, world_to_screen,
)


def row(name, expectation, measured, passed, why=None):
    verdict = "PASS" if passed else ("NOT-RUN" if why is not None else "FAIL")
    return {"row": name, "expectation": expectation, "measured": measured,
            "verdict": verdict, "why": why}


def _cam(camera):
    return f"x {camera['x']:.1f} y {camera['y']:.1f} ×{camera['scale']:.3f}"


# ------------------------------------------------------------------------------- dragging


def row_drag(studio):
    """A 200 px drag on the background moves the camera offset 200 px, and the scale not at all."""
    before = studio.camera()
    studio.drag(CENTRE, 200, 0)
    after = studio.settle()
    moved = after["x"] - before["x"]
    expected = studio.expect_drag
    scale_kept = after["scale"] == before["scale"]
    passed = abs(moved - expected) <= DRAG_TOLERANCE and scale_kept
    return row("nav-drag", f"a {expected} px drag moves the offset {expected} px ±0.5",
               f"{moved:+.2f} px, scale {before['scale']} → {after['scale']}", passed)


def row_vertical_drag(studio):
    """The y of a drag is the y of the camera: panning is not a horizontal-only affordance."""
    before = studio.camera()
    studio.drag(CENTRE, 0, -140)
    after = studio.settle()
    moved = after["y"] - before["y"]
    return row("nav-drag-y", "a 140 px drag up moves the offset 140 px up",
               f"{moved:+.2f} px", abs(moved + 140) <= DRAG_TOLERANCE)


def row_space_drag(studio):
    """Space held, then a drag: the camera pans, as it does for the middle button."""
    before = studio.camera()
    studio.hold_space()
    studio.drag(CENTRE, -120, 0)
    studio.release_space()
    after = studio.settle()
    moved = after["x"] - before["x"]
    return row("nav-space-drag", "space+drag pans 120 px left",
               f"{moved:+.2f} px", abs(moved + 120) <= DRAG_TOLERANCE)


def row_middle_drag(studio):
    """The middle button pans: no modifier held, and no autoscroll either."""
    before = studio.camera()
    studio.drag(CENTRE, 0, -120, button="middle", buttons=4)
    after = studio.settle()
    moved = after["y"] - before["y"]
    return row("nav-middle-drag", "middle-drag pans 120 px up",
               f"{moved:+.2f} px", abs(moved + 120) <= DRAG_TOLERANCE)


# ------------------------------------------------------------------------------- zooming


def _anchor_row(studio, name, notch, ctrl):
    before = studio.camera()
    at = CENTRE if not ctrl else OFF_CENTRE
    world = screen_to_world(before, at)
    studio.wheel(at, notch, ctrl=ctrl)
    after = studio.settle()
    landed = world_to_screen(after, world)
    drift = apart(landed, {"x": at[0], "y": at[1]})
    zoomed = after["scale"] != before["scale"]
    zoomed_in = (after["scale"] > before["scale"]) == (notch < 0)
    passed = drift <= ANCHOR_TOLERANCE and zoomed and zoomed_in
    return row(name, f"a wheel notch at {at} moves the world point under it ≤0.5 px",
               f"drift {drift:.3f} px, scale {before['scale']:.4f} → {after['scale']:.4f}", passed)


def row_wheel(studio):
    """A wheel notch at (x,y): the world point under the cursor stays where it was."""
    return _anchor_row(studio, "nav-wheel", WHEEL_NOTCH, ctrl=False)


def row_pinch(studio):
    """A ctrlKey wheel is a pinch, and anchors at the cursor the same way."""
    return _anchor_row(studio, "nav-pinch", PINCH_NOTCH, ctrl=True)


def row_double_click(studio):
    """A double-click on the background zooms in by two, at the cursor."""
    before = studio.camera()
    at = OFF_CENTRE
    world = screen_to_world(before, at)
    studio.double_click(at)
    after = studio.settle()
    ratio = after["scale"] / before["scale"]
    drift = apart(world_to_screen(after, world), {"x": at[0], "y": at[1]})
    passed = abs(ratio - 2) < 1e-9 and drift <= ANCHOR_TOLERANCE
    return row("nav-double-click", "a double-click on the background zooms ×2 at the cursor",
               f"×{ratio:.4f}, drift {drift:.3f} px", passed)


def row_clamp(studio):
    """The scale never leaves the view's own limits, whichever way it is pushed."""
    limits = studio.read()["limits"]
    floor, ceiling = limits["min"], limits["max"]
    zoomed_in = _zoom_until(studio, "+")
    zoomed_out = _zoom_until(studio, "-")
    inside = floor <= zoomed_out["scale"] and zoomed_in["scale"] <= ceiling
    at_ceiling = abs(zoomed_in["scale"] - ceiling) < 1e-9
    at_floor = abs(zoomed_out["scale"] - floor) < 1e-9
    return row("nav-zoom-clamp", f"the scale stops at {ceiling} going in and at {floor} going out",
               f"in: {zoomed_in['scale']:.4f}, out: {zoomed_out['scale']:.4f}",
               inside and at_ceiling and at_floor)


def _zoom_until(studio, key):
    for _ in range(CLAMP_PRESSES):
        studio.key(key)
    return studio.settle()


# --------------------------------------------------------------------------------- keys


def _key_row(studio, name, key, check, expectation):
    before = studio.camera()
    studio.key(key)
    after = studio.settle()
    passed, measured = check(before, after)
    return row(name, expectation, measured, passed)


def _reset_check(before, after):
    return after["scale"] == 1, f"scale {after['scale']}"


def _zoom_in_check(before, after):
    ratio = after["scale"] / before["scale"]
    return abs(ratio - 2) < 1e-9, f"×{ratio:.4f}"


def _zoom_out_check(before, after):
    ratio = after["scale"] / before["scale"]
    return abs(ratio - 0.5) < 1e-9, f"×{ratio:.4f}"


def _pan_check(dx, dy):
    def check(before, after):
        moved_x = after["x"] - before["x"]
        moved_y = after["y"] - before["y"]
        passed = abs(moved_x - dx) < 1e-9 and abs(moved_y - dy) < 1e-9
        return passed, f"offset moved ({moved_x:+.1f}, {moved_y:+.1f})"
    return check


KEY_ROWS = [
    ("nav-key-reset", "0", _reset_check, "0 resets: the scale is 1"),
    ("nav-key-zoom-in", "+", _zoom_in_check, "+ zooms in: the scale doubles"),
    ("nav-key-zoom-out", "-", _zoom_out_check, "- zooms out: the scale halves"),
    ("nav-key-pan-left", "ArrowLeft", _pan_check(-50, 0), "ArrowLeft pans 50 px left"),
    ("nav-key-pan-right", "ArrowRight", _pan_check(50, 0), "ArrowRight pans 50 px right"),
    ("nav-key-pan-up", "ArrowUp", _pan_check(0, -50), "ArrowUp pans 50 px up"),
    ("nav-key-pan-down", "ArrowDown", _pan_check(0, 50), "ArrowDown pans 50 px down"),
]


def row_fit(studio):
    """`f` fits: it moves the camera off a zoomed, panned one, it is the same camera every
    time (a fit is a function of the drawing and the viewport), and afterwards nothing is
    drawn in the canvas' outer band — a fit that cropped a node would not be one."""
    studio.key("+")
    studio.key("+")
    studio.key("ArrowRight")
    time.sleep(SETTLE_S)
    away = studio.camera()
    cut_before = studio.border_drawn()
    studio.key("f")
    first = studio.settle()
    studio.drag(CENTRE, 60, 0)
    studio.key("f")
    second = studio.settle()
    cut_after = studio.border_drawn()
    moved = first != away
    return row("nav-key-fit", "f fits: the same fitted camera each time, nothing drawn at the edge",
               f"{_cam(away)} → {_cam(first)}, again {_cam(second)}, "
               f"edge pixels {cut_before} → {cut_after}",
               moved and first == second and cut_before > 0 and cut_after == 0)


def row_escape(studio):
    """Escape clears a selection that is really there, and leaves the camera where it was."""
    at = studio.find_node()
    if at is None:
        return row("nav-key-escape", "Escape clears the selection and does not move the camera",
                   "no node was hit by the probe's own hit-test", False,
                   "the probe found no node to click, so it could not select one first")
    studio.click(at)
    time.sleep(SETTLE_S)
    selected = studio.read()
    studio.key("Escape")
    time.sleep(SETTLE_S)
    after = studio.read()
    cleared = selected["selected"] >= 0 and after["selected"] == -1
    kept = after["camera"] == selected["camera"]
    return row("nav-key-escape", "Escape clears the selection and does not move the camera",
               f"selected {selected['selected']} at {at} → {after['selected']}", cleared and kept)


def run_rows(studio):
    studio.focus_page()
    return [
        row_drag(studio), row_vertical_drag(studio), row_wheel(studio), row_pinch(studio),
        row_double_click(studio), row_fit(studio),
        *[_key_row(studio, name, key, check, text) for name, key, check, text in KEY_ROWS],
        row_clamp(studio), row_escape(studio), row_space_drag(studio), row_middle_drag(studio),
    ]
