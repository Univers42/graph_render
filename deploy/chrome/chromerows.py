"""The chrome rows: the right edge and the bottom edge, at every size, in both themes.

A cell is one viewport at one device pixel ratio with the studio's panels in a stated
arrangement. Each cell makes three claims about the same flat strip on the right edge:

1. the page does not scroll sideways: `documentElement.scrollWidth == innerWidth`, and the
   host's own box ends exactly at the viewport's right and bottom edges;
2. the rightmost and bottom-most `BAND_CSS` pixels of the composite are the canvas, and not
   the page showing through beside it;
3. the canvas' backing store is the CSS box times the device pixel ratio, clamped at 2.

(2) needs a definition of "the canvas painted this", and the obvious one is wrong. The
composite is the canvas plus the studio's own panels over it, and every panel is inset by
12 px with a 24 px blurred box shadow, so the band legitimately contains a darkening
gradient of the canvas. The test is therefore: a band pixel is the canvas when its colour
is the theme's background, or a colour the canvas is painting, or one of those darkened —
because a shadow over the canvas is the canvas multiplied down, and a page showing through
beside it is a flat colour of its own that the canvas was never asked to draw. A scrollbar
track, an unstyled page background and a gap the host left are all that last kind, and a
flat strip of them is what this row is for.

The other thing that makes a band's colour unfamiliar is a device pixel ratio the view has
not re-measured: the compositor then rescales the backing store, and a resampled edge pixel
is a blend of two of the canvas' own colours rather than either. Both cases are why the
palette here is the canvas' whole colour set and not just what is inside the band.

Nothing here dispatches through the studio's own API: the theme is switched by typing
`theme light` into the console's input, and the dock by clicking its Controls button.
"""
import math
import time

import cdp
from chromedrive import BAND_CSS, CHANNEL_TOLERANCE, MAX_DPR, THEME_BACKGROUND

# The sizes a studio is opened at. 320 is the narrowest phone, 2560 a 5K display; 768 and
# 1024 are the tablet and laptop breakpoints the chrome lays itself out around.
WIDTHS = (320, 375, 768, 1024, 1440, 1920, 2560)
DPRS = (1, 1.25, 1.5, 2)
# How tall each width is opened at: wide screens are short and narrow ones tall, both
# within what a real window manager would give.
HEIGHTS = {320: 700, 375: 812, 768: 1024, 1024: 768, 1440: 900, 1920: 1080, 2560: 1400}
# What a resize is given time to settle before the numbers are read back.
SETTLE_S = 0.5


def row(name, expectation, measured, passed, why=None):
    """One row of the report. `why` is why a row could not run — never why it failed."""
    verdict = "PASS" if passed else ("NOT-RUN" if why is not None else "FAIL")
    return {"row": name, "expectation": expectation, "measured": measured,
            "verdict": verdict, "why": why}


# ------------------------------------------------------------------------- what is on screen


def _at(pixels, width, x, y):
    i = (y * width + x) * 3
    return (pixels[i], pixels[i + 1], pixels[i + 2])


def _unpack(packed):
    """The canvas' colours as triples. They cross the wire packed into one int each, which
    is what the page's own Set wanted and is a third of the JSON of three numbers."""
    return [(value >> 16 & 0xFF, value >> 8 & 0xFF, value & 0xFF) for value in packed]


def _quantise(colour):
    """A colour as a lookup key. Quantising to a step of `CHANNEL_TOLERANCE + 2` is what
    makes "within a channel or two of" an exact set membership instead of a scan."""
    step = CHANNEL_TOLERANCE + 2
    return (colour[0] // step, colour[1] // step, colour[2] // step)


# A shadow is the canvas multiplied down. These are the factors a blurred `rgba(0,0,0,α)`
# over it can produce, from half black to nothing at all.
DARKENING = (0.5, 0.6, 0.7, 0.8, 0.9, 1.0)


def _palette(background, canvas_colours):
    """Every colour the canvas can be responsible for at the edge, darkened for shadows."""
    keys = {_quantise(background)}
    for colour in canvas_colours:
        keys.add(_quantise(colour))
        for factor in DARKENING[:-1]:
            keys.add(_quantise(tuple(round(channel * factor) for channel in colour)))
    return keys


def _foreign(colours, keys):
    return sorted(c for c in colours if _quantise(c) not in keys)


def _edges(pixels, width, height, dpr, boxes, scale):
    """The distinct colours of the `BAND_CSS` band on the right edge and on the bottom,
    with the studio's own boxes removed. A pixel the studio painted is not a strip, however
    unfamiliar its colour, so the band is judged only where the studio painted nothing."""
    band = max(1, round(BAND_CSS * dpr))
    inside = [(box[0] * scale, box[1] * scale, box[2] * scale, box[3] * scale) for box in boxes]
    right, bottom = set(), set()
    for y in range(0, height, 2):
        for x in range(width - band, width):
            if not _in_any(x + 0.5, y + 0.5, inside):
                right.add(_at(pixels, width, x, y))
    for x in range(0, width, 2):
        for y in range(height - band, height):
            if not _in_any(x + 0.5, y + 0.5, inside):
                bottom.add(_at(pixels, width, x, y))
    return right, bottom


def _in_any(x, y, boxes):
    return any(left <= x <= right and top <= y <= bottom for left, top, right, bottom in boxes)


def _js_round(value):
    """`Math.round`: half away from zero up. Python's `round` is half to even, and
    375 x 1.5 is exactly one of those, so the expected backing store has to be rounded the
    way the renderer rounds it or the row fails on the gate's own arithmetic."""
    return math.floor(value + 0.5)


# ------------------------------------------------------------------------------ one cell


def check(chrome, name, theme=None):
    """One viewport. Returns a row, and the measurements it was judged from."""
    metrics = chrome.metrics()
    width, height, pixels = chrome.screenshot()
    canvas = chrome.canvas_report()
    dpr = metrics["dpr"]
    background = THEME_BACKGROUND[metrics["theme"]]
    # The screenshot is in device pixels and the studio's own boxes are in CSS pixels.
    scale = width / metrics["innerWidth"] if metrics["innerWidth"] else dpr
    right, bottom = _edges(pixels, width, height, dpr, chrome.chrome_boxes(), scale)
    keys = _palette(background, _unpack(canvas["colours"]) + _unpack(canvas["edgeColours"]))

    scroll = metrics["scrollWidth"] == metrics["innerWidth"]
    flush = (abs(metrics["right"] - metrics["innerWidth"]) <= 0.5
             and abs(metrics["bottom"] - metrics["innerHeight"]) <= 0.5)
    want_w = _js_round(metrics["hostWidth"] * min(MAX_DPR, dpr))
    want_h = _js_round(metrics["hostHeight"] * min(MAX_DPR, dpr))
    backing = canvas["width"] == want_w and canvas["height"] == want_h
    stray_right = _foreign(right, keys)
    stray_bottom = _foreign(bottom, keys)
    edges = not stray_right and not stray_bottom

    if theme is not None and metrics["theme"] != theme:
        return row(name, f"the theme action switches to {theme}",
                   f"the studio is on {metrics['theme']} after `theme {theme}`", False,
                   f"the console did not take the command `theme {theme}`: the studio stayed "
                   f"on {metrics['theme']}")
    if canvas["corner"] != list(background):
        return row(name, f"the canvas is filled with the {metrics['theme']} background",
                   f"corner {canvas['corner']}, token {list(background)}", False)

    measured = (f"scrollW {metrics['scrollWidth']}={metrics['innerWidth']} · "
                f"right {metrics['right']:.1f}={metrics['innerWidth']} · "
                f"bottom {metrics['bottom']:.1f}={metrics['innerHeight']} · "
                f"backing {canvas['width']}x{canvas['height']} "
                f"{'=' if backing else '!='} {want_w}x{want_h} · "
                f"right band {len(right)} colours, {len(stray_right)} foreign"
                + (f" {stray_right[:2]}" if stray_right else "")
                + f" · bottom band {len(bottom)} colours, {len(stray_bottom)} foreign"
                + (f" {stray_bottom[:2]}" if stray_bottom else "")
                + f" · canvas edge has {canvas['edgeDistinct']} colours")
    passed = scroll and flush and backing and edges
    expectation = (f"the page does not scroll, the host's right and bottom edges are the "
                   f"viewport's, the {BAND_CSS} px right and bottom bands are the canvas, and "
                   f"the backing store is the box at {min(MAX_DPR, dpr)}x")
    return row(name, expectation, measured, passed)


def resize(chrome, width, height, dpr):
    chrome.page.set_viewport(width, height, dpr)
    time.sleep(SETTLE_S)


# ---------------------------------------------------------------------------- the cells


def run_sizes(chrome):
    """Every width at every device pixel ratio, with the studio in its default arrangement."""
    rows = []
    for width in WIDTHS:
        height = HEIGHTS[width]
        for dpr in DPRS:
            resize(chrome, width, height, dpr)
            rows.append(check(chrome, f"chrome-size-{width}x{height}@{dpr}"))
    return rows


def run_themes(chrome):
    """Both themes, switched by the console's own action, at two sizes and two ratios."""
    rows = []
    for width, height, dpr in ((1440, 900, 1), (320, 700, 2), (2560, 1400, 1.5)):
        resize(chrome, width, height, dpr)
        for theme in ("light", "dark", "light"):
            if not chrome.ensure_console(True):
                rows.append(row(f"chrome-theme-{theme}-{width}@{dpr}", "the console opens",
                                "the console is not in the document", False,
                                "the backquote shortcut did not open the console, so the "
                                "theme action could not be reached"))
                break
            chrome.type_command(f"theme {theme}")
            time.sleep(SETTLE_S)
            rows.append(check(chrome, f"chrome-theme-{theme}-{width}@{dpr}", theme=theme))
        chrome.close_console()
    return rows


def run_panels(chrome):
    """The dock and the console, open and closed: the chrome must not reach the edge band,
    so the band's cleanliness cannot be the panels covering a broken edge."""
    rows = []
    for width, height, dpr in ((1440, 900, 1), (320, 700, 1), (1920, 1080, 2)):
        resize(chrome, width, height, dpr)
        before = chrome.dock_open()
        after = chrome.toggle_dock()
        if after == before:
            rows.append(row(f"chrome-dock-{width}@{dpr}", "clicking Controls closes the dock",
                            f"the dock was {'open' if before else 'closed'} and stayed so", False,
                            "the Controls button did not toggle the dock, so the closed dock "
                            "was never on screen"))
        else:
            rows.append(check(chrome, f"chrome-dock-closed-{width}@{dpr}"))
            rows.append(check(chrome, f"chrome-dock-open-{width}@{dpr}"))
        opened = chrome.ensure_console(True)
        if not opened:
            rows.append(row(f"chrome-console-{width}@{dpr}", "` opens the console",
                            "the console is not in the document", False,
                            "the backquote shortcut did not open the console, so the open "
                            "console was never on screen"))
        else:
            rows.append(check(chrome, f"chrome-console-open-{width}@{dpr}"))
        chrome.close_console()
        rows.append(check(chrome, f"chrome-console-closed-{width}@{dpr}"))
    return rows


def run_sweep(chrome):
    """Widths up and then back down, at a fixed height and ratio: a host sized from `100vw`
    is re-laid-out on every one of those, and one of them leaving an edge behind is the bug."""
    widths = (320, 375, 768, 1024, 1440, 1920, 2560)
    steps = [check_after(chrome, width, 900, 1.5) for width in widths + widths[::-1]]
    failed = [step for step in steps if not step["verdict"] == "PASS"]
    worst = max((step for step in steps if step["verdict"] != "PASS"),
                key=lambda s: s["row"], default=None)
    return [row("chrome-sweep", "every width up to 2560 and back down to 320 keeps the page "
                "flush and the edge bands the canvas'",
                f"{len(steps)} steps, {len(failed)} not clean"
                + (f"; worst {worst['row']}: {worst['measured']}" if worst else ""),
                not failed)]


def check_after(chrome, width, height, dpr):
    resize(chrome, width, height, dpr)
    return check(chrome, f"chrome-sweep-{width}@{dpr}")


def run_dpr_change(chrome):
    """The device pixel ratio changing with the CSS size held still: a window moved to a
    screen of another density, or a browser zoom. Nothing about the canvas' CSS box moves,
    so a view that only re-measures on a resize never learns the ratio changed."""
    rows = []
    for width, height in ((1440, 900), (320, 700), (1024, 768)):
        for first, second in ((1, 2), (1, 1.5), (1.5, 1), (2, 1.25)):
            resize(chrome, width, height, first)
            resize(chrome, width, height, second)
            rows.append(check(chrome, f"chrome-dpr-{first}to{second}-{width}"))
    return rows


def run_console(chrome):
    """Nothing the run did left an error in the page's own log."""
    log = chrome.console_log()
    bad = [line for line in log if line.startswith(("error", "uncaught", "rejection"))]
    return [row("chrome-console-clean", "no error or uncaught exception in the page's log",
                f"{len(log)} lines, {len(bad)} of them errors"
                + (f": {bad[0][:80]}" if bad else ""), not bad)]
