"""The edge gradient row: a mixed edge runs from its source's colour to its target's.

The dock's own segmented control is clicked with a real mouse event — no dispatch through the
studio's API — and three pixels along one known mixed edge are read back from the canvas. The
expected colours are computed here, from the two palette entries and the five-stop linear
rule, so the gate carries its own oracle and not the renderer's.

A stroke is at most 1.5 CSS px wide, so a pixel on a line is that colour over the background
at a coverage the gate cannot see, and which pixel of a small patch the stroke landed on is
not known either. Each sample is therefore fitted: the pixel of its patch that carries the
ramp's colour best, at the best coverage over the ground, and the row passes when the three
fits are within a byte per channel. `expect_gradient=False` is the negative control — the
mode is never switched, so the same three pixels come back in the theme's flat colour.
"""
import json
import math
import time

from drive import SETTLE_S
from gradientpage import ALONG, CLEAR, FIND, MARGIN, MIN_CHORD, READ
from verdict import row

# The gate's own copy of colour/srgb.ts: the sRGB transfer function, IEC 61966-2-1.
KNEE = 0.0031308
DECODE_KNEE = 0.04045
SLOPE = 12.92
SCALE = 1.055
GAMMA = 1 / 2.4
DECODE_SCALE = 2.4
OFFSET = 0.055
# GRADIENT_STOPS in packages/graph-render/src/colour/blend.ts: the ramp a gradient carries.
STOPS = 5
# The fit's acceptance: one byte per channel, and a line covering most of the pixel it is read from.
BYTE = 1
COVERED = 0.6
# The view counts as moving for this long after the last camera change (MOVING_MS in
# packages/graph-render/src/canvas2d/loop.ts), so a gradient arrives on the frame after that one.
MOVING_MS = 140


def encode(c):
    x = min(1.0, max(0.0, c))
    return SLOPE * x if x <= KNEE else SCALE * x ** GAMMA - OFFSET


def decode(byte):
    x = min(1.0, max(0.0, byte / 255))
    return x / SLOPE if x <= DECODE_KNEE else ((x + OFFSET) / SCALE) ** DECODE_SCALE


def byte_of(c):
    return math.floor(255 * encode(c) + 0.5)


def stop_of(at, frm, to):
    """One stop of the ramp: the two colours mixed in linear light, each channel encoded once."""
    lo, hi = [decode(b) for b in frm], [decode(b) for b in to]
    return [byte_of(lo[i] + (hi[i] - lo[i]) * at) for i in range(3)]


def shows_at(t, frm, to):
    """What the rasteriser shows at `t`: the encoded stops around it, mixed in sRGB."""
    scaled = t * (STOPS - 1)
    low = min(STOPS - 1, max(0, math.floor(scaled)))
    high = min(STOPS - 1, low + 1)
    f = scaled - low
    a, b = stop_of(low / (STOPS - 1), frm, to), stop_of(high / (STOPS - 1), frm, to)
    return [a[i] + (b[i] - a[i]) * f for i in range(3)]


def fit(bg, want_at, patch):
    """The pixel of `patch` that carries the ramp's colour best: (error, cover, colour)."""
    best = (float("inf"), 0.0, None)
    for row in patch:
        for seen in row:
            for step in range(int(COVERED * 100), 101):
                cover = step / 100
                error = max(abs(cover * w + (1 - cover) * b - s) for w, b, s in zip(want_at, bg, seen))
                if error < best[0]:
                    best = (error, cover, seen)
    return best


def click_control(studio, selector):
    """The centre of a dock control, in page coordinates, or None when it is not there."""
    return studio.page.evaluate(f"""
    (() => {{
      const root = document.querySelector('graph-studio').shadowRoot;
      const el = {selector};
      if (el === null || el === undefined || el.hidden || el.disabled) return null;
      el.scrollIntoView({{ block: 'center' }});
      const r = el.getBoundingClientRect();
      return [r.left + r.width / 2, r.top + r.height / 2];
    }})()
    """)


def _segment(label, mode):
    """The `mode` button of the dock's segmented control titled `label`."""
    return (f"[...document.querySelector('graph-studio').shadowRoot"
            f".querySelectorAll('[role=group][aria-label={json.dumps(label)}] button')]"
            f".find((b) => b.textContent.trim() === {json.dumps(mode)} && !b.disabled)")


GRADIENT_STROKES = "document.querySelector('graph-studio').view.stats().gradientStrokes"


def await_gradients(studio, cap=6.0):
    """Wait for the frame the view has stopped for: the one whose mixed edges took a gradient."""
    deadline = time.monotonic() + cap
    while time.monotonic() < deadline:
        if studio.page.evaluate(GRADIENT_STROKES) > 0:
            return True
        time.sleep(0.1)
    return False


def section_head(name):
    """The dock's own head for one section, so a row can open it with a click of its own."""
    return (f"document.querySelector('graph-studio').shadowRoot"
            f".querySelector('#gs-dock-{name.lower()}-head')")


def dock_button(label):
    """The dock button with that exact title, in the section the dock renders it in."""
    return (f"[...document.querySelector('graph-studio').shadowRoot.querySelectorAll('button')]"
            f".find((b) => b.textContent.trim() === {json.dumps(label)}"
            f" && !b.disabled && b.getAttribute('aria-disabled') !== 'true'"
            f" && b.getClientRects().length > 0)")


def open_section(studio, name):
    """Open one dock section with a click on its head; False when there is no head to click."""
    head = click_control(studio, section_head(name))
    if head is None:
        return False
    studio.click(tuple(head))
    time.sleep(SETTLE_S)
    return True


def pause_forces(studio):
    """Pause the live force loop, so the view stops moving and a frame is painted at rest."""
    if not open_section(studio, "Forces"):
        return False
    at = click_control(studio, dock_button("Pause"))
    if at is None:
        return False
    studio.click(tuple(at))
    time.sleep(SETTLE_S)
    return True


def set_mode(studio, mode):
    """One real mouse click on the dock's Edge colour control, or False when it is not there."""
    button = click_control(studio, _segment("Edge colour", mode))
    if button is None:
        return False
    studio.click(tuple(button))
    studio.page.evaluate("document.querySelector('graph-studio').focus()")
    time.sleep(MOVING_MS / 1000 + SETTLE_S)
    if mode == "gradient":
        await_gradients(studio)
    return True


def measure(studio):
    """Read the edge and its three pixels: what the row claims, or why it cannot claim it."""
    found = studio.page.evaluate(FIND % (json.dumps(ALONG), CLEAR, MARGIN, MARGIN, MARGIN, MARGIN, MIN_CHORD))
    chosen = found["chosen"]
    if chosen is None:
        return {"why": f"of {found['edges']} edges, {found['mixed']} had two colours and none of them "
                       f"was a {MIN_CHORD} px chord with three points clear of every node and link"}
    read = studio.page.evaluate(f"({READ})({json.dumps(chosen)})")
    want = [shows_at(t, chosen["from"], chosen["to"]) for t in chosen["along"]]
    fits = [fit(read["bg"], want_at, patch) for want_at, patch in zip(want, read["patches"])]
    seen = "; ".join(" ".join(str(v) for v in f[2]) for f in fits)
    ought = "; ".join(" ".join(f"{v:.1f}" for v in triple) for triple in want)
    return {
        "error": max(f[0] for f in fits),
        "text": (f"edge {chosen['edge']} ({chosen['chord']:.0f} px) {chosen['from']} -> {chosen['to']} at "
                 f"t {[round(t, 3) for t in chosen['along']]}, mode {found['mode']}, "
                 f"ground {' '.join(str(v) for v in read['bg'])}; read [{seen}] against [{ought}], "
                 f"coverages {[round(f[1], 2) for f in fits]}, off by {max(f[0] for f in fits):.1f}"),
        "mode": found["mode"],
    }


def row_edge_gradient(studio):
    """The edge gradient: an edge between two colours runs from the source's to the target's.

    Everything the row needs is driven with real input whatever the verdict is going to be:
    `expect_gradient=False` (the negative control) leaves only the mode alone, so it reads the
    flat drawing the studio opens on and the same three pixels come back grey.
    """
    studio.key("f")
    time.sleep(SETTLE_S)
    if not pause_forces(studio):
        return row("edge-gradient", "the live force loop can be paused from the dock",
                   "no Pause button", False, "the Forces section offered no Pause to click")
    if studio.expect_gradient:
        if not open_section(studio, "Appearance"):
            return row("edge-gradient", "the Appearance section's head is on screen", "not found",
                       False, "the dock has no openable Appearance section on the page")
        if not set_mode(studio, "gradient"):
            return row("edge-gradient", "the Edge colour control offers a gradient",
                       "not found", False, "the Appearance section has no Edge colour button to click")
    measured = measure(studio)
    if studio.expect_gradient:
        set_mode(studio, "flat")
    if "why" in measured:
        return row("edge-gradient", "a mixed edge long enough to read three pixels of it",
                   measured["why"], False, measured["why"])
    return row("edge-gradient", "a mixed edge runs from its source's colour to its target's, "
               "linear in light: each of three pixels along it fits the ramp within a byte",
               measured["text"], measured["mode"] == "gradient" and measured["error"] <= BYTE)
