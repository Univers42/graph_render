"""The rows of the 3D GL gate: does the WebGL2 3D layer draw what the Canvas2D 3D painter draws.

The 3D layer (`packages/graph-render/src/webgl2/*3d.ts`) is a second painter for a 3D frame, and
the Canvas2D painter (`three/paint3d.ts`) is both its fallback and its parity reference. These
rows name the failures a second painter has: it draws a different picture, it draws nothing, it
uploads on every drag instead of moving one uniform, it throws, and it gives up dishonestly when
its context is lost or never there.

Every claim is read from the page as a user meets it: the backend and its failure from the view's
own `stats()`, the orbit from `view.orbit()`, the pixels from the canvas' own bitmap, and the GL
calls from a counter the gate wraps around the context's prototype before the page loads.
"""
import gpu
import smokerows
import verdict

LAYOUT = "layout.basic3d.sphere"
# The drag every load is given before its screenshot: real CDP input, so the orbit is turned
# and the depth test has overlapping nodes to order.
DRAG = (160, 60)

# The largest channel difference (0-255) one pixel may carry and still count as the same pixel:
# the backend gate's own (deploy/nav/backendrows.py), for one meaning of "the same pixel".
GAP = 32
# The share of pixels the two 3D painters may differ on. Measured on this host on 2026-10-03 at
# 0.0367% (463 of 1 260 000 pixels; 1.81%, 0.295%, 0.0367% and 0.0102% past a gap of 8, 16, 32
# and 64) and set to the next round value above it; docs/measurements/perf-3d-gl.md has the run.
# What separates the two is antialiasing at node rims and edges, a sub-1.75 px node drawn as a
# disc where Canvas2D fills a square, and an edge bundle each edge of which GL blends on its own
# (`webgl2/shaders3d.ts` says which and why).
CEILING = 0.001
# The `auto` row's graph: 59 996 elements with the perf driver's two links per node, past the 3D
# layer's SPACE_THRESHOLD (`webgl2/hook3d.ts`), so only the renderer's name keeps it off GL.
AUTO_NODES = 20000
# A canvas with less than this share of its pixels off its own background drew nothing.
BLANK_FLOOR = 0.01
# Two orbits this far apart in any field are the same orbit for a pixel row.
ORBIT_TOLERANCE = 1e-9


def _same_orbit(one, other):
    if one is None or other is None:
        return False
    fields = ("yaw", "pitch", "distance", "fov")
    return all(abs(one[key] - other[key]) <= ORBIT_TOLERANCE for key in fields)


def _orbit(orbit):
    if orbit is None:
        return "no orbit"
    return f"yaw {orbit['yaw']:.4f} pitch {orbit['pitch']:.4f} distance {orbit['distance']:.1f}"


def row_parity(parity, ceiling):
    expectation = (f"`{LAYOUT}` turned by one drag, drawn by canvas2d and by webgl2 from the same "
                   f"orbit, differs on at most {ceiling:.1%} of its pixels")
    left, right = parity["canvas2d"]["orbit"], parity["webgl2"]["orbit"]
    measured = (f"{parity['share']:.4%} of the pixels differ by more than {GAP} of 255 in a channel; "
                f"canvas2d at {_orbit(left)}, webgl2 at {_orbit(right)}")
    passed = parity["share"] <= ceiling and _same_orbit(left, right)
    return verdict.row("gl3d-parity", expectation, measured, passed)


def row_drawn(parity, floor):
    left, right = parity["canvas2d"]["offBackground"], parity["webgl2"]["offBackground"]
    expectation = f"both canvas screenshots have at least {floor:.0%} of their pixels off the background"
    measured = f"canvas2d {left:.4%} off it, webgl2 {right:.4%}"
    return verdict.row("gl3d-drawn", expectation, measured, min(left, right) >= floor)


def row_backend(parity):
    """Each page names the painter it was asked for: otherwise parity compares Canvas2D with itself."""
    left, right = parity["canvas2d"]["stats"], parity["webgl2"]["stats"]
    expectation = ("?backend=canvas2d is drawn by canvas2d and ?backend=webgl2 by webgl2, every node "
                   "of the 3D frame counted")
    measured = (f"canvas2d page: {left['backend']!r}, {left['drawnNodes']} nodes; webgl2 page: "
                f"{right['backend']!r}, {right['drawnNodes']} of {right['nodes']} nodes, failure "
                f"{right['backendFailure']!r}")
    passed = (left["backend"] == "canvas2d" and right["backend"] == "webgl2"
              and right["drawnNodes"] == right["nodes"] > 0)
    return verdict.row("gl3d-backend", expectation, measured, passed)


def row_uniform_only(calls):
    """A drag is the camera and nothing else: no buffer and no texture is sent while it moves."""
    expectation = ("an orbit drag on the webgl2 page sends no buffer and no texture; it sets the "
                   "camera uniform and draws")
    measured = (f"bufferData {calls['bufferData']}, texImage2D {calls['texImage2D']}, uniformMatrix4fv "
                f"{calls['uniformMatrix4fv']}, drawArraysInstanced {calls['drawArraysInstanced']}")
    passed = (calls["bufferData"] == 0 and calls["texImage2D"] == 0
              and calls["uniformMatrix4fv"] > 0 and calls["drawArraysInstanced"] > 0)
    return verdict.row("gl3d-uniform-only", expectation, measured, passed)


def faults_of(page, store_error):
    """The load smoke's error channels, read before the next load clears the page's events."""
    thrown = smokerows.events(page, "Runtime.exceptionThrown")
    called = smokerows.faults(page, "Runtime.consoleAPICalled", ("type",), ("error", "assert"))
    logged = smokerows.faults(page, "Log.entryAdded", ("entry", "level"), ("error",))
    faults = [*thrown, *called, *logged]
    if store_error is not None:
        faults.append(f"store error {smokerows.short(store_error)}")
    return faults


def row_clean(faults):
    """The webgl2 page parity and the drag used: a layer that threw would still draw."""
    expectation = "no store error, no uncaught exception and no console error on the webgl2 page"
    measured = "none" if not faults else "; ".join(faults)
    return verdict.row("gl3d-clean", expectation, measured, not faults)


def row_lost(lost, floor):
    """`lost` is the before, the after and the canvas: "before" proves there was a layer to lose."""
    expectation = ("a WebGL2 3D layer that was drawing, then lost its context, falls back to "
                   "canvas2d, names the lost context, and keeps drawing")
    stats, before, drawn = lost["stats"], lost["before"], lost["shot"]["offBackground"]
    measured = (f"backend {before['backend']!r} before the loss, {stats['backend']!r} after, "
                f"backendFailure {stats['backendFailure']!r}, {drawn:.4%} of the canvas off the "
                f"background ({lost['loseContext']})")
    passed = (before["backend"] == "webgl2" and stats["backend"] == "canvas2d"
              and "lost" in stats["backendFailure"] and drawn >= floor)
    return verdict.row("gl3d-context-lost", expectation, measured, passed)


def row_fallback(fallback, floor):
    expectation = ("?backend=webgl2 on a browser with no WebGL2 draws the 3D frame with canvas2d "
                   "and says why")
    stats, drawn = fallback["stats"], fallback["shot"]["offBackground"]
    measured = (f"backend {stats['backend']!r}, backendFailure {stats['backendFailure']!r}, "
                f"{drawn:.4%} of the canvas off the background")
    passed = stats["backend"] == "canvas2d" and stats["backendFailure"] != "" and drawn >= floor
    return verdict.row("gl3d-fallback", expectation, measured, passed)


def row_auto(auto, nodes):
    """`auto` on a software rasteriser keeps a 3D frame past the layer's threshold on Canvas2D."""
    expectation = (f"?backend=auto on a renderer named as software draws a {nodes}-node 3D frame, "
                   "past the 3D layer's threshold, with canvas2d and no failure")
    stats, renderer = auto["stats"], str(auto["renderer"])
    software = any(name in renderer.lower() for name in gpu.SOFTWARE_NAMES)
    measured = (f"backend {stats['backend']!r}, backendFailure {stats['backendFailure']!r}, "
                f"{stats['nodes']} nodes and {stats['edges']} edges, {_orbit(auto['orbit'])}, "
                f"renderer {renderer!r}")
    passed = (software and stats["backend"] == "canvas2d" and stats["backendFailure"] == ""
              and stats["nodes"] == nodes and auto["orbit"] is not None)
    return verdict.row("gl3d-auto-software", expectation, measured, passed)


def run_rows(runs):
    """The rows, in the order a reader meets the failures; `runs` is what gl.py measured."""
    parity = runs["parity"]
    return [row_backend(parity),
            row_parity(parity, CEILING),
            row_drawn(parity, BLANK_FLOOR),
            row_uniform_only(runs["drag"]["calls"]),
            row_clean(runs["drag"]["faults"]),
            row_lost(runs["context-lost"], BLANK_FLOOR),
            row_fallback(runs["fallback"], BLANK_FLOOR),
            row_auto(runs["auto"], AUTO_NODES)]
