"""Page-side helpers for the layout-parameter rows: the panel as the dock draws it, the run the
motor reported, and the pixels that run left on the canvas.

The panel is read out of the shadow root and driven with real CDP mouse events, so a row is
about a hand moving a control — not about a dispatch the studio would have accepted from a
script. `studio.run(line)` is the console's own parser and is the only other way in, because a
typed line is a thing the product has.
"""
import time

BUSY_S = 90.0
SETTLE_S = 0.35
# The moving picture is a transition, so the pixels are read once they have stopped moving.
STILL_S = 0.4
STILL_CAP_S = 8.0

PANEL_ID = "gs-dock-layout-settings"

INSTALL = """
(() => {
  const host = document.querySelector('graph-studio');
  const studio = host.studio, view = host.view;
  const root = host.shadowRoot;
  const sleep = (ms) => new Promise((done) => setTimeout(done, ms));
  const canvas = root.querySelector('canvas');
  window.__p = {
    studio, view, root, sleep, canvas,
    section: () => root.querySelector('#%(panel)s'),
    head: () => root.querySelector('#%(panel)s-head'),
    /** The controls of the panel, in the order the motor published the parameters. */
    controls: () => {
      const body = window.__p.section();
      if (body === null) return null;
      const labels = Array.from(body.querySelectorAll('.gs-field-label')).map((node) => node.textContent);
      const count = (selector) => body.querySelectorAll(selector).length;
      return {
        labels,
        sliders: count('input.gs-range'),
        numbers: count('input[type="number"]'),
        switches: count('input.gs-check'),
        buttons: Array.from(body.querySelectorAll('button')).map((node) => node.textContent.trim()),
        reason: (body.querySelector('.gs-reason') || {}).textContent || '',
      };
    },
    /** Where a slider sits on screen, and how wide its track is, or null. */
    track: (name) => {
      const body = window.__p.section();
      const label = Array.from(body.querySelectorAll('.gs-field-label')).find((node) => node.textContent === name);
      if (label === undefined) return null;
      const input = label.parentElement.querySelector('input.gs-range');
      if (input === null) return null;
      // The dock scrolls, and a hand scrolls it to the control first. Without this the press
      // lands below the fold: the open Layout section put the threshold track at y = 963 in a
      // 720 px viewport (2026-10-04), and every drag row measured nothing.
      input.scrollIntoView({ block: 'center' });
      const box = input.getBoundingClientRect();
      const low = Number(input.min), high = Number(input.max), at = Number(input.value);
      const span = high - low;
      // Where the thumb is, as a fraction of the track: the press jumps the value to the
      // pointer, so a press on the thumb is the one press that cannot move it.
      const thumb = span > 0 ? (at - low) / span : 0;
      return {
        x: box.left, y: box.top + box.height / 2, width: box.width, value: input.value,
        thumb: Math.min(1, Math.max(0, thumb)),
      };
    },
    state: () => {
      const at = studio.store.get();
      return {
        busy: at.busy.length,
        error: at.error === null ? null : { title: at.error.title, detail: at.error.detail },
        layout: at.settings.layout,
        params: at.settings.params,
        specs: (at.schemas[at.settings.layout] || []).map((spec) => spec.name),
        digest: at.run === null ? null : at.run.digest,
        layoutCalls: at.layoutCalls,
        last: at.log.length === 0 ? null : { ok: at.log[at.log.length - 1].ok, message: at.log[at.log.length - 1].message },
      };
    },
    /** A signature of what is on the canvas: every 37th pixel, so it is cheap and stable. */
    pixels: () => {
      const at = canvas.getContext('2d').getImageData(0, 0, canvas.width, canvas.height).data;
      let sum = 0;
      for (let i = 0; i < at.length; i += 37 * 4) sum = (sum * 31 + at[i] + at[i + 1] * 3 + at[i + 2] * 7) %% 0xffffffff;
      return sum;
    },
  };
  return true;
})()
""" % {"panel": PANEL_ID}


def install(studio):
    studio.page.evaluate(INSTALL)


def state(studio):
    return studio.page.evaluate("window.__p.state()")


def controls(studio):
    """The panel as the dock draws it, or `None` when the dock has no such section."""
    return studio.page.evaluate("window.__p.controls()")


def console(studio, line):
    """A typed line through the console's own parser, and the log entry it produced."""
    return studio.page.evaluate(f"""
    (async () => {{
      const entry = await window.__p.studio.run({line!r});
      return {{
        ok: entry.ok === true, message: entry.message,
        error: entry.error === null ? null : {{ title: entry.error.title, detail: entry.error.detail }},
      }};
    }})()
    """)


def settled(studio, seconds=BUSY_S):
    """Wait for the motor: a row read against a layout still running is a lie."""
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        if studio.page.evaluate("window.__p.state().busy") == 0:
            break
        time.sleep(0.2)
    time.sleep(SETTLE_S)


def still(studio):
    """The pixel signature, once the transition has stopped moving it."""
    deadline = time.monotonic() + STILL_CAP_S
    was = studio.page.evaluate("window.__p.pixels()")
    while time.monotonic() < deadline:
        time.sleep(STILL_S)
        now = studio.page.evaluate("window.__p.pixels()")
        if now == was:
            return now
        was = now
    return was


def click(studio, at):
    """One click where a hand would put it."""
    studio.press_at(at[0], at[1], "left", 1)
    studio.release_at(at[0], at[1], "left")


def head_box(studio):
    """Where the section's own header is, so the row can open the section with a click."""
    return studio.page.evaluate("""
    (() => {
      const head = window.__p.head();
      if (head === null) return null;
      head.scrollIntoView({ block: 'center' });
      const box = head.getBoundingClientRect();
      return [box.left + box.width / 2, box.top + box.height / 2];
    })()
    """)


def open_section(studio):
    """Open the Layout settings section if it is closed, with a click on its header."""
    if studio.page.evaluate("window.__p.section().hasAttribute('hidden')"):
        at = head_box(studio)
        if at is None:
            return False
        click(studio, at)
        time.sleep(0.2)
    return True


def drag_slider(studio, name, fraction, broken=False, steps=20):
    """Press on a slider's track and walk the pointer along it, then let go.

    WHY the walk and not one press: the control draws a draft while the pointer moves and runs
    the layout when it is let go, so this is the gesture a hand makes and the row can count
    what it cost.

    `broken` is the negative control: the pointer never reaches the control at all, which is the
    gesture every row here depends on. A row that claims a change changed then has to be red on
    its own measurement, and a green one would be a row that measured nothing.
    """
    track = studio.page.evaluate(f"window.__p.track({name!r})")
    if track is None:
        return None
    if broken:
        return {"before": track["value"], "asked": fraction, "moved": False}
    from_fraction = 0.5
    to_fraction = fraction
    start = (track["x"] + track["width"] * from_fraction, track["y"])
    end = (track["x"] + track["width"] * to_fraction, track["y"])
    studio.press_at(start[0], start[1], "left", 1)
    for step in range(1, steps + 1):
        x = start[0] + (end[0] - start[0]) * step / steps
        studio.page.call("Input.dispatchMouseEvent", {
            "type": "mouseMoved", "x": x, "y": start[1], "button": "left", "buttons": 1,
        })
    studio.release_at(end[0], end[1], "left")
    return {"before": track["value"], "asked": fraction, "moved": not broken}
