"""The layout-parameter rows: what the dock shows, what one gesture costs, and what the console
says about the same value.

Every row is a claim about the served app read from three places the studio cannot fake for
itself: the shadow root's own controls, the run report the motor sent back (its digest), and the
pixels the canvas holds once the transition has stopped. A row that cannot be driven is
NOT-RUN with the reason, never PASS.

`--break` reaches exactly one thing: the drag presses where the thumb already is and walks to
where it already is, so the value does not move. Every row that claims a change changed then has
to be red, and the gate exits non-zero.
"""
import cdp
import paramspage as page
from navrows import row
from verdict import row as judged

FORCE = "layout.force.spring"
LAYERED = "layout.dag.sugiyama"
THRESHOLD = "threshold"
SPACING = "layer_spacing"
# Where on the track a hand puts the pointer, as a fraction of its width: the far end is the
# largest value the schema publishes for `threshold`, which is a visibly different drawing.
FAR = 0.9
# What the reset says when the layout is already where the reset would put it.
ALREADY = "already at the motor's defaults"


def _ready(studio, layout):
    """On `layout`, at the motor's own defaults, with the section open. The reason, or nothing.

    WHY the reset is here and not in each row: the values are kept per layout id, so without it
    every row would start from whatever the row before it left, and a row that claims "the
    drawing changed" would be reading its own leftovers.
    """
    answer = page.console(studio, f"layout {layout}")
    if not answer["ok"]:
        return f"`layout {layout}` was refused: {(answer['error'] or {}).get('detail', answer['message'])}"
    page.settled(studio)
    # The reset refuses when there is nothing to put back, and that refusal is the state this
    # wants: a layout already at its published defaults is what every row starts from.
    reset = page.console(studio, "layoutreset")
    detail = (reset["error"] or {}).get("detail", reset["message"])
    if not reset["ok"] and ALREADY not in detail:
        return f"`layoutreset` was refused: {detail}"
    page.settled(studio)
    if not page.open_section(studio):
        return "the dock has no Layout settings section to open"
    return None


def _shown(studio):
    """The controls the panel is drawing, or the reason it is drawing none."""
    shown = page.controls(studio)
    if shown is None:
        raise cdp.CdpError("the dock carries no Layout settings section")
    return shown


def row_panel(studio, broken):
    """The panel shows one control per published parameter, of the kind the schema says."""
    gap = _ready(studio, FORCE)
    if gap is not None:
        return judged("params-panel", "one control per published parameter", "nothing was driven", False, gap)
    shown = _shown(studio)
    at = page.state(studio)
    passed = shown["labels"] == ["iterations", THRESHOLD, "scale"] and shown["sliders"] == 2 \
        and shown["numbers"] == 1 and shown["switches"] == 0 and at["specs"] == shown["labels"]
    return row("params-panel", "a force layout publishing three parameters shows three controls: two sliders and a number field",
               f"{at['layout']} publishes {at['specs']}, the panel shows {shown['labels']} "
               f"({shown['sliders']} sliders, {shown['numbers']} numbers)", passed)


def _drag(studio, name, broken):
    """Drag `name`'s slider to the far end. Where it was, where it went, and what it cost."""
    before = page.state(studio)
    was = page.still(studio)
    walked = page.drag_slider(studio, name, FAR, broken)
    if walked is None:
        return None
    page.settled(studio)
    after = page.state(studio)
    now = page.still(studio)
    return {"before": before, "was": was, "now": now, "after": after, "walked": walked,
            "calls": after["layoutCalls"] - before["layoutCalls"]}


def row_slider(studio, broken):
    """A slider moved by hand moves the drawing: the run's digest and the pixels both change."""
    gap = _ready(studio, FORCE)
    if gap is not None:
        return judged("params-slider", "a moved slider redraws", "nothing was driven", False, gap)
    moved = _drag(studio, THRESHOLD, broken)
    if moved is None:
        return judged("params-slider", "a moved slider redraws", "the panel has no such slider", False,
                      f"the panel draws no slider labelled `{THRESHOLD}`")
    held = moved["after"]["params"].get(FORCE, {}).get(THRESHOLD)
    passed = held is not None and held != moved["walked"]["before"] \
        and moved["after"]["digest"] != moved["before"]["digest"] and moved["now"] != moved["was"]
    return row("params-slider", "dragging a published parameter's slider changes the value, the run's digest and the pixels",
               f"{THRESHOLD} {moved['walked']['before']} → {held}, digest "
               f"{(moved['before']['digest'] or '')[:8]} → {(moved['after']['digest'] or '')[:8]}, "
               f"pixels {moved['was']} → {moved['now']}", passed)


def row_one_run(studio, broken):
    """A slider dragged across its track costs one run, not one per pixel of the drag."""
    gap = _ready(studio, FORCE)
    if gap is not None:
        return judged("params-one-run", "one drag, one run", "nothing was driven", False, gap)
    moved = _drag(studio, THRESHOLD, broken)
    if moved is None:
        return judged("params-one-run", "one drag, one run", "the panel has no such slider", False,
                      f"the panel draws no slider labelled `{THRESHOLD}`")
    held = moved["after"]["params"].get(FORCE, {}).get(THRESHOLD)
    passed = moved["calls"] == 1 and held is not None and held != moved["walked"]["before"]
    return row("params-one-run", "a 20-step drag of one slider asks the motor for exactly one run",
               f"20 pointer moves, {moved['calls']} layout call(s), {THRESHOLD} → {held}", passed)


def row_console(studio, broken):
    """The typed word reaches the same place: one value, one run, a drawing of its own.

    `iterations` and not `threshold`: the published range of the early-exit threshold is 0..1 and
    anything above the first iterate's own norm stops the pass after one gather, so two such
    values draw the same picture and the row would be measuring the motor's floor, not the panel.
    Two gathers against the published fifty is a difference any run can see.
    """
    gap = _ready(studio, FORCE)
    if gap is not None:
        return judged("params-console", "the console word moves the value", "nothing was driven", False, gap)
    before = page.state(studio)
    was = page.still(studio)
    asked = PUBLISHED_GATHERS if broken else 2
    answer = page.console(studio, f"layoutset iterations {asked}")
    page.settled(studio)
    after = page.state(studio)
    now = page.still(studio)
    held = after["params"].get(FORCE, {}).get("iterations")
    passed = answer["ok"] and held == asked and after["digest"] != before["digest"] and now != was \
        and after["layoutCalls"] == before["layoutCalls"] + 1
    return row("params-console", f"`layoutset iterations {2}` writes the value, costs one run and redraws",
               f"{'ok' if answer['ok'] else answer['message']}, iterations → {held}, digest "
               f"{(before['digest'] or '')[:8]} → {(after['digest'] or '')[:8]}, "
               f"{after['layoutCalls'] - before['layoutCalls']} run(s)", passed)


# The published default of `iterations` is fifty, so a word asking for that is the break's
# fault: the run happens, the value is right, and the drawing is the one that was already there.
PUBLISHED_GATHERS = 50


def row_reset(studio, broken):
    """`layoutreset` gives back the motor's own drawing, byte for byte."""
    gap = _ready(studio, FORCE)
    if gap is not None:
        return judged("params-reset", "the reset restores the published run", "nothing was driven", False, gap)
    first = page.state(studio)["digest"]
    moved = _drag(studio, THRESHOLD, broken)
    if moved is None:
        return judged("params-reset", "the reset restores the published run", "the panel has no such slider", False,
                      f"the panel draws no slider labelled `{THRESHOLD}`")
    answer = page.console(studio, "layoutreset")
    page.settled(studio)
    after = page.state(studio)
    passed = answer["ok"] and after["params"].get(FORCE) is None and after["digest"] == first
    return row("params-reset", "after a change, `layoutreset` redraws the published defaults, the same digest as the first run",
               f"{'ok' if answer['ok'] else answer['message']}, held {after['params'].get(FORCE)}, digest "
               f"{(first or '')[:8]} → {(after['digest'] or '')[:8]}", passed)


def row_refused(studio, broken):
    """A value outside the published range is refused by name, and the drawing stands."""
    gap = _ready(studio, FORCE)
    if gap is not None:
        return judged("params-refused", "an out-of-range value is refused", "nothing was driven", False, gap)
    before = page.state(studio)
    answer = page.console(studio, "layoutset threshold 5")
    page.settled(studio)
    after = page.state(studio)
    detail = (answer["error"] or {}).get("detail", answer["message"])
    passed = not answer["ok"] and "threshold" in detail and "0..1" in detail \
        and after["digest"] == before["digest"] and after["params"] == before["params"]
    return row("params-refused", "`layoutset threshold 5` is refused by name and the drawing is untouched",
               f"{'accepted' if answer['ok'] else detail[:90]}, digest unchanged: {after['digest'] == before['digest']}",
               passed)


def row_layered(studio, broken):
    """A layered layout's own parameter moves its own drawing: the panel follows the layout."""
    gap = _ready(studio, LAYERED)
    if gap is not None:
        return judged("params-layered", "a layered layout's parameter redraws it", "nothing was driven", False, gap)
    shown = _shown(studio)
    before = page.state(studio)
    was = page.still(studio)
    moved = page.drag_slider(studio, SPACING, FAR, broken)
    if moved is None:
        return judged("params-layered", "a layered layout's parameter redraws it", "the panel has no such slider", False,
                      f"the panel draws no slider labelled `{SPACING}`")
    page.settled(studio)
    after = page.state(studio)
    now = page.still(studio)
    held = after["params"].get(LAYERED, {}).get(SPACING)
    passed = shown["labels"] == [SPACING] and held is not None and held != moved["before"] \
        and after["digest"] != before["digest"] and now != was
    return row("params-layered", f"on {LAYERED} the panel shows only {SPACING}, and moving it redraws the layers",
               f"panel {shown['labels']}, {SPACING} {moved['before']} → {held}, digest "
               f"{(before['digest'] or '')[:8]} → {(after['digest'] or '')[:8]}, pixels {was} → {now}", passed)


ROWS = (row_panel, row_slider, row_one_run, row_console, row_reset, row_refused, row_layered)


def run_rows(studio, broken=False):
    studio.focus_page()
    page.install(studio)
    return [make(studio, broken) for make in ROWS]
