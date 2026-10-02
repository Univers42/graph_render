"""The rows of the backend gate: who drew the last frame, and whether the two painters agree.

The WebGL2 layer is a second painter for one graph and nothing else in the tree compares the
two, so a layer that draws something else — or draws nothing at all — passes every other gate.
The four failures these rows name: the layer paints a different picture, `auto` never reaches for
it on a graph big enough, the browser gives no WebGL2 and the fallback must say so while still
drawing, and the context is lost in the middle of a session.

Every claim is read from the page as a user would meet it: the backend from the view's own
`stats()`, the fallback's reason from the same field, and the pixels from the canvas screenshots
the gate saved under `target/studio-backend/`.
"""
import smokerows
import verdict

# The graph both painters are given: the perf driver's own synthetic source (random, degree 3,
# seed 1 — one generator for the whole tree) laid out by `layout.grid`, which is a fixed
# function of the graph, so the two loads put every node in the same pixel.
PARITY_NODES = 2000
PARITY_LAYOUT = "layout.grid"
# A graph well over BULK_THRESHOLD (8192), where `auto` must reach for the GPU layer.
LARGE_NODES = 20000
# Pivot MDS takes 1.3 s at 20 000 nodes where ForceAtlas2 takes 66 s (deploy/perf/run.py); the
# row is about which backend draws the frame, not about the layout.
LARGE_LAYOUT = "layout.mds.pivot"

# The largest channel difference (0-255) one pixel may carry and still count as the same pixel.
# Measured on this host on the parity graph, the share of pixels differing by more than 8, 16, 32
# and 64 counts is 19.7%, 7.4%, 1.06% and 0.07%: most of what separates the two painters is a
# few counts on an antialiased edge, and a pixel the two painters place differently is what the
# row is for. GL has no line width, no arrows and no impostor spheres (webgl2/layer.ts), so the
# row is a ceiling, not equality.
GAP = 32
# The share of pixels the two painters may differ on. Measured on this host on 2026-10-02 at
# 1.0579% and set to the next round value above it; the run that read it is the report the gate
# wrote, under `target/studio-backend/`, and the row prints what it measured every time.
CEILING = 0.02
# A canvas with less than this share of its pixels off its own background is a blank canvas: a
# frame that never drew leaves the ground and nothing else. The ground is dithered, so a canvas
# that drew nothing reads 0.16% here and a canvas with this graph on it reads 41-44%; 1% is the
# one value between the two that is neither.
BLANK_FLOOR = 0.01
# Two cameras this far apart in x, y or scale are the same camera for a pixel row.
CAMERA_TOLERANCE = 1e-6


def _camera(camera):
    return f"({camera['x']:.4f},{camera['y']:.4f})x{camera['scale']:.6f}"


def _same_camera(one, other):
    return all(abs(one[key] - other[key]) <= CAMERA_TOLERANCE for key in ("x", "y", "scale"))


def row_parity(share, cameras, ceiling):
    expectation = (f"the same {PARITY_NODES}-node graph drawn by canvas2d and by webgl2, from the "
                   f"same camera, differs on at most {ceiling:.0%} of its pixels")
    measured = (f"{share:.4%} of the pixels differ by more than {GAP} of 255 in a channel; "
                f"canvas2d at {_camera(cameras[0])}, webgl2 at {_camera(cameras[1])}")
    return verdict.row("backend-parity-2k", expectation, measured,
                       share <= ceiling and _same_camera(*cameras))


def row_drawn(canvas2d, webgl2, floor):
    expectation = f"both canvas screenshots have at least {floor:.0%} of their pixels off the background"
    measured = f"canvas2d {canvas2d:.4%} off it, webgl2 {webgl2:.4%}"
    return verdict.row("backend-parity-drawn", expectation, measured, min(canvas2d, webgl2) >= floor)


def row_auto(stats):
    expectation = f"a {LARGE_NODES}-node graph at ?backend=auto is drawn by the WebGL2 layer"
    measured = f"backend {stats['backend']!r}, backendFailure {stats['backendFailure']!r}"
    return verdict.row("backend-auto-large", expectation, measured, stats["backend"] == "webgl2")


def row_fallback(stats, drawn, floor):
    expectation = "a browser with no WebGL2 on an OffscreenCanvas falls back to canvas2d and says why"
    measured = (f"backend {stats['backend']!r}, backendFailure {stats['backendFailure']!r}, "
                f"{drawn:.4%} of the canvas off the background")
    return verdict.row("backend-fallback", expectation, measured,
                       stats["backend"] == "canvas2d" and stats["backendFailure"] != "" and drawn >= floor)


def row_fallback_clean(page):
    """The same error channels the load smoke reads, on the fallback page and nothing else.

    A silent layer that gave up would still draw through the 2D painter, so the fallback rows
    alone cannot tell a browser without WebGL2 from a layer that threw its exception away.
    """
    expectation = "no uncaught exception and no console error on the fallback page"
    thrown = smokerows.events(page, "Runtime.exceptionThrown")
    called = smokerows.faults(page, "Runtime.consoleAPICalled", ("type",), ("error", "assert"))
    logged = smokerows.faults(page, "Log.entryAdded", ("entry", "level"), ("error",))
    faults = [*thrown, *called, *logged]
    measured = "none" if not faults else "; ".join(faults)
    return verdict.row("backend-fallback-clean", expectation, measured, not faults)


def row_lost(lost, floor):
    """`lost` is the whole context-lost measurement: the backend before, after, and the canvas.

    The "before" is what makes the row a loss and not a page that never had a layer: a fallback
    that was already on Canvas2D passes the other two claims without anything ever having been
    lost.
    """
    expectation = ("a WebGL2 layer that was drawing, then lost its context, falls back to canvas2d, "
                   "names the lost context, and keeps drawing")
    stats, before, drawn = lost["stats"], lost["before"], lost["shot"]["offBackground"]
    measured = (f"backend {before['backend']!r} before the loss, {stats['backend']!r} after, "
                f"backendFailure {stats['backendFailure']!r}, {drawn:.4%} of the canvas off the "
                f"background")
    passed = (before["backend"] == "webgl2" and stats["backend"] == "canvas2d"
              and "lost" in stats["backendFailure"] and drawn >= floor)
    return verdict.row("backend-context-lost", expectation, measured, passed)


def run_rows(runs):
    """The six rows, in the order a reader meets the failures.

    `runs` is the four measurements `backend.py` took, keyed by row group; the fallback's page
    carries the error channels, so the clean row reads its own measurement.
    """
    parity, auto, fallback = runs["parity"], runs["auto"], runs["fallback"]
    return [row_parity(parity["share"], parity["cameras"], CEILING),
            row_drawn(parity["canvas2d"]["offBackground"], parity["webgl2"]["offBackground"], BLANK_FLOOR),
            row_auto(auto["stats"]),
            row_fallback(fallback["stats"], fallback["shot"]["offBackground"], BLANK_FLOOR),
            row_fallback_clean(fallback["page"]),
            row_lost(runs["context-lost"], BLANK_FLOOR)]