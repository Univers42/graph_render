"""Rows of the overlap probe: how many drawn node discs cover each other once a settle is over.

A pair overlaps when the distance between the two screen centres is under the sum of the two
screen radii, read the way the canvas painter draws them: the frame's own radius column, else
half the larger box side, else the style's radius, times the camera scale, floored at the
painter's minimum. Pairs are found with a uniform grid one disc wide, so a spread-out drawing
costs O(n); a pile-up costs O(k^2) in its own cell, which is the thing being measured.

Ponytail: MIN_SCREEN_RADIUS is a copy of packages/graph-render/src/canvas2d/nodes.ts:16. A change
to the painter's floor leaves this count reading the old floor until the copy is updated; it
under-reports overlap when the floor grows. The WebGL2 layer draws the same radius.
"""
import base64
import json
import statistics
import time

import liverows
import smokerows

HOST = "document.querySelector('graph-studio')"
CLUSTERED = "force/clustered.json"
SYNTHETIC = {"nodes": 10000, "degree": 2, "seed": 1, "shape": "random"}
# A settle at 10 000 nodes in a software-raster container takes seconds, not minutes.
SETTLE_CAP_S = 90.0
# The frame-rate window: long enough to hold a few hundred frames, short enough to sit inside
# one settle at 10 000 nodes. Three settles, and the median, because the host is shared.
FPS_WINDOW_S = 3.0
FPS_RUNS = 3
# `spread` has to cut the overlapping pairs by at least this much: a second settle at the
# defaults lands on a different random start and moves the count by a few percent either way.
SPREAD_CUT = 0.5

COUNT = """
(() => {
  const MIN_SCREEN_RADIUS = 1.25;
  const view = %s.view;
  const frame = view.frame();
  const style = view.style();
  const cam = view.camera();
  const port = view.viewport();
  const n = frame.nodeCount;
  const xs = new Float64Array(n), ys = new Float64Array(n), rs = new Float64Array(n);
  let reach = MIN_SCREEN_RADIUS, shown = 0;
  for (let i = 0; i < n; i += 1) {
    const p = view.position(i);
    xs[i] = p.x * cam.scale + cam.x;
    ys[i] = p.y * cam.scale + cam.y;
    const boxed = frame.w !== null && frame.h !== null ? Math.max(frame.w[i], frame.h[i]) / 2 : null;
    const extent = frame.r !== null ? frame.r[i] : (boxed ?? style.radius[i] ?? 0);
    rs[i] = Math.max(MIN_SCREEN_RADIUS, extent * cam.scale);
    if (rs[i] > reach) reach = rs[i];
    if (xs[i] >= 0 && ys[i] >= 0 && xs[i] <= port.width && ys[i] <= port.height) shown += 1;
  }
  const cell = 2 * reach;
  const grid = new Map();
  const keyOf = (cx, cy) => cx * 1048576 + cy;
  for (let i = 0; i < n; i += 1) {
    if (style.hidden !== null && style.hidden[i] === 1) continue;
    const key = keyOf(Math.floor(xs[i] / cell), Math.floor(ys[i] / cell));
    const bucket = grid.get(key);
    if (bucket === undefined) grid.set(key, [i]); else bucket.push(i);
  }
  let pairs = 0;
  const touched = new Uint8Array(n);
  for (const [key, bucket] of grid) {
    const cx = Math.round(key / 1048576), cy = key - cx * 1048576;
    for (let dx = -1; dx <= 1; dx += 1) for (let dy = -1; dy <= 1; dy += 1) {
      const other = grid.get(keyOf(cx + dx, cy + dy));
      if (other === undefined) continue;
      for (const i of bucket) for (const j of other) {
        if (j <= i) continue;
        const ddx = xs[i] - xs[j], ddy = ys[i] - ys[j], sum = rs[i] + rs[j];
        if (ddx * ddx + ddy * ddy < sum * sum) { pairs += 1; touched[i] = 1; touched[j] = 1; }
      }
    }
  }
  let nodes = 0;
  for (let i = 0; i < n; i += 1) nodes += touched[i];
  return { nodes: n, shown, pairs, overlapped: nodes, scale: cam.scale, reach };
})()
""" % HOST

# Every force frame the page draws goes through `view.setPositions`; counting the calls is the
# live loop's frame rate as the page receives it, painted or not.
HOOK = """
(() => {
  const view = %s.view;
  if (view.__counted) return true;
  const draw = view.setPositions.bind(view);
  window.__frames = [];
  view.setPositions = (xs, ys) => { window.__frames.push(performance.now()); draw(xs, ys); };
  view.__counted = true;
  return true;
})()
""" % HOST


def row(name, expectation, measured, ok):
    return {"row": name, "expectation": expectation, "measured": measured, "verdict": "PASS" if ok else "FAIL"}


def not_run(name, expectation, measured, why):
    return {"row": name, "expectation": expectation, "measured": measured, "verdict": "NOT-RUN", "why": why}


def dispatch(studio, action, args):
    """Runs one registry action; returns the log entry the studio wrote for it."""
    entry = studio.page.evaluate(f"{HOST}.studio.dispatch({json.dumps(action)}, {json.dumps(args)})")
    wait_idle(studio)
    return entry


def wait_idle(studio, cap=SETTLE_CAP_S):
    deadline = time.monotonic() + cap
    while time.monotonic() < deadline:
        studio.page.watch_workers()
        if studio.page.evaluate(f"{HOST}.studio.store.get().busy.length") == 0:
            return True
        time.sleep(0.1)
    return False


def settle(studio):
    """A fresh settle from random positions, waited out, then fitted: the whole graph on screen."""
    studio.page.evaluate(f"{HOST}.studio.dispatch('forces.animate', {{ on: true }})")
    liverows.wait_shown(studio)
    began = time.monotonic()
    done = liverows.wait_settled(studio, SETTLE_CAP_S)
    took = time.monotonic() - began
    studio.page.evaluate(f"{HOST}.view.fit()")
    time.sleep(0.5)
    return done, took


def counted(studio):
    return studio.page.evaluate(COUNT)


def shoot(page, path):
    data = page.call("Page.captureScreenshot", {"format": "png"})["data"]
    path.write_bytes(base64.b64decode(data))


def fps_once(studio):
    """Frames the page received over one window of a settle at the current knobs."""
    studio.page.evaluate("window.__frames = []")
    studio.page.evaluate(f"{HOST}.studio.dispatch('forces.animate', {{ on: true }})")
    time.sleep(FPS_WINDOW_S)
    stamps = studio.page.evaluate("window.__frames.slice()")
    liverows.wait_settled(studio, SETTLE_CAP_S)
    if len(stamps) < 2:
        return 0.0
    return (len(stamps) - 1) * 1000.0 / (stamps[-1] - stamps[0])


def describe(at, took):
    return (f"{at['pairs']} pairs, {at['overlapped']}/{at['nodes']} nodes in one "
            f"({at['shown']} on screen, scale {at['scale']:.4g}, largest disc {at['reach']:.3g} px, "
            f"settle {took:.1f}s)")


def errors(page):
    """The four places a fault shows itself: exceptions, console errors, the store, the banner."""
    page.watch_workers()
    thrown = smokerows.events(page, "Runtime.exceptionThrown")
    logged = smokerows.faults(page, "Runtime.consoleAPICalled", ("type",), ("error",))
    probe = page.evaluate(smokerows.PROBE) or {}
    return {"exceptions": thrown[:3], "console": logged[:3], "store": probe.get("error"),
            "banner": probe.get("banner")}


def fps_row(studio, label):
    rates = [fps_once(studio) for _ in range(FPS_RUNS)]
    median = statistics.median(rates)
    measured = f"median {median:.2f} fps over {FPS_RUNS} windows of {FPS_WINDOW_S}s: " + \
        ", ".join(f"{rate:.2f}" for rate in rates)
    return row(f"live-fps-10k-{label}", "the live loop delivers frames at 10 000 nodes", measured, median > 0)


def has(studio, action):
    return studio.page.evaluate(f"{HOST}.studio.registry.find({json.dumps(action)}) !== undefined")


def measured_case(studio, name, shot):
    """A settle from random positions at the knobs already set, then the count: (row, count or None)."""
    expectation = "a settle that ends, and every drawn disc counted"
    done, took = settle(studio)
    at = counted(studio)
    if shot is not None:
        shoot(studio.page, shot)
    if not done or not at or at["nodes"] == 0:
        return row(name, expectation, f"settled={done} after {took:.1f}s, count {at}", False), None
    return row(name, expectation, describe(at, took), True), at


def judged_spread(name, before, after_row, after):
    """The spread row: its own count, judged against the same source's count at the defaults."""
    if before is None or after is None:
        return after_row
    allowed = int((1.0 - SPREAD_CUT) * before["pairs"])
    expectation = f"`spread` cuts the overlapping pairs by at least {SPREAD_CUT:.0%} ({before['pairs']} -> <= {allowed})"
    return row(name, expectation, after_row["measured"], after["pairs"] <= allowed)


def spread_case(studio, name, before, shot):
    """`spread` on the drawing already on screen, the way a user presses it, then a settle."""
    entry = dispatch(studio, "forces.spread", {})
    case_row, at = measured_case(studio, name, shot)
    case_row["measured"] += f"; spread said: {(entry or {}).get('message')}"
    return judged_spread(name, before, case_row, at)


def spread_rows(studio, broken, at, out):
    """`spread` on the 10k drawing, its frame rate, then the clustered fixture from the defaults."""
    names = ("overlap-10k-spread", "live-fps-10k-spread", "overlap-clustered-spread")
    if broken or not has(studio, "forces.spread"):
        why = "the negative control skips `spread`" if broken else "this build has no `forces.spread`"
        return [not_run(name, "measured after `spread`", "not measured", why) for name in names]
    rows = [spread_case(studio, names[0], at["10k"], None), fps_row(studio, "spread")]
    dispatch(studio, "forces.reset", {})
    dispatch(studio, "source.fixture", {"path": CLUSTERED})
    rows.append(spread_case(studio, names[2], at["clustered"], out / "clustered-spread.png"))
    return rows


def errors_row(page):
    seen = errors(page)
    clean = not seen["exceptions"] and not seen["console"] and seen["store"] is None and not seen["banner"]
    return row("overlap-no-fault", "no exception, no console error, no store error, no banner", json.dumps(seen), clean)


def run_rows(studio, broken, out):
    """Defaults first, on both sources; then `spread` on both; then the four fault channels."""
    studio.page.evaluate(HOOK)
    at = {}
    dispatch(studio, "source.fixture", {"path": CLUSTERED})
    clustered_row, at["clustered"] = measured_case(studio, "overlap-clustered-defaults", out / "clustered-defaults.png")
    dispatch(studio, "source.synthetic", SYNTHETIC)
    big_row, at["10k"] = measured_case(studio, "overlap-10k-defaults", None)
    rows = [clustered_row, big_row, fps_row(studio, "defaults")]
    rows.extend(spread_rows(studio, broken, at, out))
    rows.append(errors_row(studio.page))
    return rows
