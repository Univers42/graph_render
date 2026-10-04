"""The layout switch row: a layout the studio settles live still lands inside the viewport.

The dock's Layout list is clicked with a real mouse event, and the drawing is read back the
way the painter puts it on screen: `screen = world · scale + offset`, with the camera's offset
canvas-relative, so the box a centre has to be in is `0..width` by `0..height` and not the
viewport's own box. The row claims both halves at once — every centre inside that box, and a
camera that is already the fit of what is drawn, so pressing `f` afterwards moves nothing. A
camera fitted to the layout run while the live loop redraws the graph somewhere else is what
that second half is there to catch: on `hierarchy/tree-balanced.json`, switching to `force.drl`
left 0 of 15 centres inside and the canvas with no drawn pixel at all, and only the fit button
brought them back. On the studio's own graph develop dbab64cb leaves the camera at ×0.356 after
the switch, and `f` then moves it to ×0.328.

The camera is not compared with the one from before the switch: two layouts can draw the
same extent, whose fit is the camera it already had.

The wait is the one every other row uses, `settle_drawing`, which polls until the positions
stop moving (drive.py:92-96). It is only the right wait once the run has committed — a still
drawing is not a settled one — so the click is followed by a poll for the studio's own layout
id with nothing left running.

`expect_switch_stale` is the negative control: it inverts the camera half, so the row then
claims `f` still moves the camera after the switch, and a fixed build has to fail it.
"""
import json
import time

from gradientrows import click_control, open_section
from verdict import row

NAME = "nav-layout-switch"
# The button's own label is `force.drl`: the list shows shortName(id), the part after the dot.
TARGET = "force.drl"
LAYOUT = "layout.force.drl"
# A layout run on the studio's own graph takes seconds before it commits, so the poll is
# generous where a layout that never arrives is a failure, not a slow machine.
SWITCH_CAP_S = 20.0

HEAD = ("document.querySelector('graph-studio').shadowRoot"
        ".querySelector('#gs-dock-layout-head')")

LIST = ("[...document.querySelector('graph-studio').shadowRoot"
        ".querySelectorAll('[role=group][aria-label=%s] button')]")

# The section's head is a toggle and the dock opens Layout first, so clicking it blind would
# close the very section the row needs; null when the dock has no such head at all.
OPEN = """
(() => {
  const head = %s;
  return head === null ? null : head.getAttribute('aria-expanded') === 'true';
})()
"""

# The run has committed when nothing is running and the settings name its layout; the live
# settle that follows is not a run, so the busy list is already empty by then.
COMMITTED = """
(() => {
  const at = document.querySelector('graph-studio').studio.store.get();
  return at.busy.length === 0 ? at.settings.layout : null;
})()
"""

# One read: the studio's layout, and every node centre as the painter draws it.
READ = """
(() => {
  const host = document.querySelector('graph-studio');
  const view = host.view;
  const camera = view.camera();
  const nodes = view.frame().nodeCount;
  const box = host.shadowRoot.querySelector('canvas').getBoundingClientRect();
  let inside = 0;
  const span = [null, null, null, null];
  for (let i = 0; i < nodes; i += 1) {
    const world = view.position(i);
    const x = world.x * camera.scale + camera.x;
    const y = world.y * camera.scale + camera.y;
    span[0] = span[0] === null ? x : Math.min(span[0], x);
    span[1] = span[1] === null ? y : Math.min(span[1], y);
    span[2] = span[2] === null ? x : Math.max(span[2], x);
    span[3] = span[3] === null ? y : Math.max(span[3], y);
    if (x >= 0 && x <= box.width && y >= 0 && y <= box.height) inside += 1;
  }
  return {
    layout: host.studio.store.get().settings.layout,
    nodes: nodes,
    inside: inside,
    span: span.map((v) => (v === null ? null : Math.round(v * 10) / 10)),
    camera: camera,
    box: [box.left, box.top, box.width, box.height],
  };
})()
"""


def _cam(camera):
    return f"x {camera['x']:.1f} y {camera['y']:.1f} ×{camera['scale']:.3f}"


def _span(span):
    return "[none]" if span[0] is None else " ".join(f"{v:.1f}" for v in span)


def _claim(stale, nodes):
    """What the row holds itself to under the mode it was given, counts included."""
    if stale:
        return (f"switching the Layout to {TARGET} leaves all {nodes} node centres inside the "
                f"canvas and a camera that `f` still moves")
    return (f"switching the Layout to {TARGET} leaves all {nodes} node centres inside the canvas "
            f"and a camera already fitted to that drawing, which `f` does not move")


def _target():
    """The Layout list's own button for `TARGET`, in the shape the edge colour control uses."""
    return (LIST % json.dumps("Layout")
            + f".find((b) => b.textContent.trim() === {json.dumps(TARGET)} && !b.disabled)")


def await_switch(studio, cap=SWITCH_CAP_S):
    """Wait for the run the click started, and report the layout the studio settled on."""
    deadline = time.monotonic() + cap
    last = None
    while time.monotonic() < deadline:
        last = studio.page.evaluate(COMMITTED)
        if last == LAYOUT:
            return last
        time.sleep(0.1)
    return last


def row_layout_switch(studio):
    """Switch the layout from the dock, and claim the drawing and the camera that come with it.

    `expect_switch_stale` (the negative control) drives exactly the same click; only the second
    half of the claim is inverted, so the row then fails a build that does move the camera.
    """
    if studio.page.evaluate(OPEN % HEAD) is False and not open_section(studio, "Layout"):
        return row(NAME, "the Layout section opens so its list of layouts can be clicked",
                   "not found", False, "the dock has no openable Layout section on the page")
    at = click_control(studio, _target())
    if at is None:
        return row(NAME, f"the Layout list offers {TARGET} to click", "not found", False,
                   f"the Layout section has no enabled {TARGET} button on the page")
    before = studio.camera()
    studio.click(tuple(at))
    settled_on = await_switch(studio)
    if settled_on != LAYOUT:
        return row(NAME, f"clicking {TARGET} in the Layout list runs a {LAYOUT} layout",
                   f"the Layout was still {settled_on} after {SWITCH_CAP_S:.0f} s", False)
    studio.settle_drawing()
    read = studio.page.evaluate(READ)
    studio.key("f")
    return _verdict(studio, before, read, studio.settle())


def _verdict(studio, before, read, refit):
    """Both halves of the claim: every centre inside the canvas, and the camera that fits them."""
    stale = studio.expect_switch_stale
    nodes, inside = read["nodes"], read["inside"]
    after = read["camera"]
    fitted = (after != refit) if stale else (after == refit)
    passed = read["layout"] == LAYOUT and nodes > 0 and inside == nodes and fitted
    measured = (f"{inside} of {nodes} centres inside, span {_span(read['span'])}, "
               f"camera {_cam(before)} → {_cam(after)}, after f {_cam(refit)}")
    return row(NAME, _claim(stale, nodes), measured, passed)