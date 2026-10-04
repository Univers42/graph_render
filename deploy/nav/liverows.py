"""Rows of the live-force gate: the settle, the drag's neighbours, the progress bar, and the
watchdog over a motor worker that dies under a live strip.

Every position is read through the element's own `view`, and every drag is real CDP mouse
input, so a row passes only for something a hand could have done.

The negative control takes the motor away from the page — `studio.destroy()` closes the
worker port, so no force request is ever sent again — and removes the view's paint hook, the
one edge a live frame takes to the canvas. All four rows must go red.
"""
import time

ROOT = "document.querySelector('graph-studio').shadowRoot"
HOST = "document.querySelector('graph-studio')"
# Two samples of the same drawing, this far apart, with nothing sent to the page between them.
SAMPLE_S = 0.4
# How long the drag row waits after letting go, so the neighbours have time to follow.
AFTER_DRAG_S = 1.2
# A settle is "moving" when some node has travelled more than this, in world units.
MOVING_PX = 1.0
# And "the neighbours followed" is this much travel by at least one of them.
NEIGHBOUR_PX = 5.0
DRAG_PX = 150
# What the row allows on top of the page's own bound: the strip is polled, so the watchdog can
# fire on time and the row still not see it until the next poll. The bound itself is read from
# the page (`watchdogBoundMs`, SILENCE_MS in packages/graph-studio/src/motor/watchdog.ts) —
# a row carrying its own copy of the number would drift from it silently.
BOUND_SLACK_S = 1.5


def row(name, expectation, measured, ok):
    return {"row": name, "expectation": expectation, "measured": measured, "verdict": "PASS" if ok else "FAIL"}


def not_run(name, expectation, measured, why):
    return {"row": name, "expectation": expectation, "measured": measured, "verdict": "NOT-RUN", "why": why}


def positions(studio):
    """Every node's world position, as the view is drawing it right now."""
    return studio.page.evaluate(f"""
    (() => {{
      const view = {HOST}.view;
      const n = view.frame().nodeCount;
      const out = [];
      for (let i = 0; i < n; i += 1) out.push(view.position(i));
      return out;
    }})()
    """)


def moved(a, b):
    """The largest L1 travel between two samples, or None when the drawing has no nodes."""
    if not a or len(a) != len(b):
        return None
    return max(abs(p["x"] - q["x"]) + abs(p["y"] - q["y"]) for p, q in zip(a, b))


def bar(studio):
    """The progress strip: is it in the document, and how wide is it on screen."""
    return studio.page.evaluate(f"""
    (() => {{
      const el = {ROOT}.querySelector('.gs-progress');
      if (el === null) return {{ present: false, width: 0, label: '' }};
      const box = el.getBoundingClientRect();
      return {{ present: true, width: box.width, height: box.height, label: el.textContent }};
    }})()
    """)


def break_the_loop(studio):
    """Take the motor away, so nothing ticks and nothing is ever drawn.

    `studio.destroy()` closes the worker port: every later force request is dropped before it
    leaves the page, the panel falls back to "not asked yet", and `forces.animate` is refused
    — so all three rows go red for one reason, the absence of the live session. The paint hook
    goes too, so the control does not rest on the loop alone.
    """
    studio.page.evaluate(f"""
    (() => {{
      const host = {HOST};
      host.view.setPositions = () => undefined;
      host.studio.destroy();
    }})()
    """)


def wait_settled(studio, seconds=6.0):
    """Sleep until the strip is gone, or the deadline passes. Returns whether it went."""
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        if bar(studio)["width"] <= 0:
            return True
        time.sleep(0.15)
    return bar(studio)["width"] <= 0


def row_settle(studio):
    """A settle moves the drawing with nothing else sent.

    A small graph's layout run starts its session cold, on the picture the layout drew, so the
    drawing rests until something asks: the row asks the way the Animate button does.
    """
    expectation = f"after Animate, node positions change between two samples {SAMPLE_S}s apart, with no other input"
    studio.page.evaluate(f"{HOST}.studio.dispatch('forces.animate', {{ on: true }})")
    wait_shown(studio)
    first = positions(studio)
    if not first:
        return not_run("live-settle", expectation, "no nodes drawn", "the view reports an empty frame")
    time.sleep(SAMPLE_S)
    travel = moved(first, positions(studio))
    measured = f"largest travel {travel:.3f} world units (threshold {MOVING_PX})"
    return row("live-settle", expectation, measured, travel is not None and travel > MOVING_PX)


def row_drag_neighbour(studio):
    expectation = f"after a {DRAG_PX} px drag, at least one neighbour of the dragged node moved"
    # The settle is waited out first: a node found mid-settle has drifted out from under the
    # pointer by the time the press lands, and the row would be measuring the miss, not the drag.
    if not wait_settled(studio):
        return row("live-drag-neighbour", expectation, f"the graph never settled: {bar(studio)}", False)
    at = studio.find_node()
    if at is None:
        return not_run("live-drag-neighbour", expectation, "no node found", "the view's hit-test found no node")
    index = studio.page.evaluate(f"{HOST}.view.pick({{x: {at[0]}, y: {at[1]}}})")
    neighbours = studio.page.evaluate(f"[...{HOST}.studio.neighbours({index})]")
    if not neighbours:
        return not_run("live-drag-neighbour", expectation, f"node {index} has no neighbours",
                       "a node with no neighbours cannot show that the rest of the graph followed")
    before = {i: studio.page.evaluate(f"{HOST}.view.position({i})") for i in neighbours}
    studio.drag(at, DRAG_PX, 0)
    time.sleep(AFTER_DRAG_S)
    travel = max(
        abs(studio.page.evaluate(f"{HOST}.view.position({i})")["x"] - point["x"])
        + abs(studio.page.evaluate(f"{HOST}.view.position({i})")["y"] - point["y"])
        for i, point in before.items()
    )
    measured = f"{len(neighbours)} neighbours, largest travel {travel:.3f} world units (threshold {NEIGHBOUR_PX})"
    return row("live-drag-neighbour", expectation, measured, travel > NEIGHBOUR_PX)


def row_progress(studio):
    """The strip must appear for a settle and be gone once it has settled.

    The second sample is taken by forcing a fresh settle from the console, which is the only
    way to see the bar again after it has hidden itself: a row that merely waited for the
    first bar to vanish would pass on a strip that never appeared.
    """
    expectation = "the progress bar is visible while the graph settles and hidden once it has"
    if not wait_settled(studio):
        return row("live-progress", expectation, f"still visible after the settle: {bar(studio)}", False)
    studio.page.evaluate(f"{HOST}.studio.dispatch('forces.animate', {{ on: true }})")
    seen = False
    deadline = time.monotonic() + 3.0
    while time.monotonic() < deadline:
        if bar(studio)["width"] > 0:
            seen = True
            break
        time.sleep(0.05)
    hidden = wait_settled(studio)
    measured = f"shown during a settle: {seen}, hidden after: {hidden}"
    return row("live-progress", expectation, measured, seen and hidden)


def wait_shown(studio, seconds=3.0):
    """Sleep until the strip is up, or the deadline passes. Returns whether it appeared."""
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        if bar(studio)["width"] > 0:
            return True
        time.sleep(0.05)
    return bar(studio)["width"] > 0


def dead_note(studio):
    """What the console was told when a live session ended, or None if it said nothing."""
    return studio.page.evaluate(f"""
    (() => {{
      const entry = {HOST}.studio.store.get().log.find((e) => e.error?.title === 'MotorWorkerLost');
      return entry === undefined ? null : entry.error.detail;
    }})()
    """)


def row_dead_worker(studio):
    """The strip must go away when the motor worker dies under it.

    The worker is stopped from outside the page with `stopMotor()`, which is what a crashed or
    terminated worker looks like from here: no `error` event, no message, just silence. The
    watchdog is what turns that silence into a hidden strip and a named cause, and the row
    measures how long that took against the bound the page itself is using.

    This row kills the motor, so it runs last: every row above it needs a live worker.
    """
    expectation = "the progress strip is hidden within the watchdog's bound after the motor worker dies"
    if not wait_settled(studio):
        return row("live-dead-worker", expectation, f"the graph never settled: {bar(studio)}", False)
    studio.page.evaluate(f"{HOST}.studio.dispatch('forces.animate', {{ on: true }})")
    if not wait_shown(studio):
        return row("live-dead-worker", expectation, f"the strip never appeared: {bar(studio)}", False)
    bound_ms = studio.page.evaluate(f"{HOST}.watchdogBoundMs")
    if not isinstance(bound_ms, (int, float)) or bound_ms <= 0:
        return not_run("live-dead-worker", expectation, f"the page reports no bound: {bound_ms!r}",
                       "watchdogBoundMs is what the row measures against; without it the row could pass on any delay")
    deadline = bound_ms / 1000.0 + BOUND_SLACK_S
    studio.page.evaluate(f"{HOST}.stopMotor()")
    began = time.monotonic()
    took = wait_settled(studio, deadline)
    elapsed = time.monotonic() - began
    reported = dead_note(studio)
    if not took:
        return row("live-dead-worker", expectation,
                   f"still visible {elapsed:.1f}s after the worker died (bound {bound_ms / 1000.0:.1f}s): {bar(studio)}",
                   False)
    if reported is None:
        return row("live-dead-worker", expectation, "the strip went but no console line named the cause", False)
    measured = f"hidden {elapsed:.1f}s after the worker died (bound {bound_ms / 1000.0:.1f}s), console said: {reported}"
    return row("live-dead-worker", expectation, measured, True)


def run_rows(studio, broken):
    if broken:
        break_the_loop(studio)
        time.sleep(0.4)
    return [row_settle(studio), row_drag_neighbour(studio), row_progress(studio), row_dead_worker(studio)]