"""The chrome rows' hands: a real viewport, a real hand, and the pixels the page drew.

Nothing here reaches into the studio to make a claim true. The studio is driven with the
same CDP input a hand sends — `Input.dispatchMouseEvent`, `Input.dispatchKeyEvent` — and
every measurement is read back out of the rendered page: the layout numbers from
`getBoundingClientRect` and `scrollWidth`, the canvas' own backing store from
`getImageData`, and the composite from `Page.captureScreenshot`.

Two things about the perf gate's CDP client shape this module around:

* `cdp.Page` is event-blind (`deploy/perf/cdp.py`: it drops every message without a matching
  id), so `Runtime.enable` and a `Runtime.consoleAPICalled` subscription would deliver
  nothing. Console errors and uncaught exceptions are therefore collected by a hook
  installed with `Page.addScriptToEvaluateOnNewDocument`, which is the same capture from
  the page's own side and is readable through `Runtime.evaluate`.
* The image has no Pillow and no ImageMagick (`deploy/chromium.Dockerfile` installs
  chromium, python3, ca-certificates and one font). The screenshot is decoded here with
  `zlib` and `struct` from the standard library: a PNG is IDAT chunks of deflate, and
  `Page.captureScreenshot` emits a non-interlaced 8-bit RGB or RGBA image, which is all
  `png_pixels` reads. There is no new dependency and nothing to install.

Ponytail: the studio draws in a `requestAnimationFrame` loop, so a screenshot is whatever
frame the compositor had; this gate asks about flat regions of the right and bottom edge
where the answer is the same on any frame. It is not a pixel-diff against a golden image.
"""
import struct
import time
import zlib

import cdp

# What the studio's own fixtures draw for; long enough for the graph to arrive, short
# enough that a stuck studio is reported instead of waited on forever.
OPEN_S = 90.0
SETTLE_S = 0.35
# The chrome insets every panel by 12 px, so an 8 px band on the right and the bottom is
# inside that gutter: a panel is never legitimately in the band, and anything flat and
# unfamiliar there is the page showing through rather than the canvas.
BAND_CSS = 8
# Colours collected from the canvas, on a grid, to decide what "painted by the canvas"
# means. The count is exact; only the returned set is capped.
COLOUR_STRIDE = 7
COLOUR_CAP = 4096
# How deep into the canvas' own right and bottom edges the colours are read at full
# resolution: the 8 CSS px band, at the largest ratio the renderer allocates one for.
EDGE_SCAN = 16
# The renderer clamps the device pixel ratio it will allocate a backing store for.
MAX_DPR = 2
# How far a channel may sit from the background and still read as the background: a
# rasteriser is asked to draw anyway, and a whole device pixel of slack is under a texel.
CHANNEL_TOLERANCE = 2

# The hook the page runs before its own scripts: it is the only way to see a console call
# through an event-blind client.
CONSOLE_HOOK = """
(() => {
  const log = [];
  Object.defineProperty(window, '__chromeLog', { value: log, configurable: true });
  const push = (kind, text) => { log.push(kind + ': ' + text); };
  for (const kind of ['error', 'warn', 'log']) {
    const original = console[kind].bind(console);
    console[kind] = (...parts) => {
      push(kind, parts.map((p) => (p instanceof Error ? p.message : String(p))).join(' '));
      original(...parts);
    };
  }
  addEventListener('error', (event) => push('uncaught', event.message));
  addEventListener('unhandledrejection', (event) => push('rejection', String(event.reason)));
})();
"""

# The studio's themes, from packages/graph-render/src/theme.ts. They are read here rather
# than off the canvas so that a canvas filled with the wrong colour is a failure and not
# quietly the thing the bands are compared against.
THEME_BACKGROUND = {"dark": (27, 27, 31), "light": (251, 251, 252)}


# ------------------------------------------------------------------------------- the image


class PngError(RuntimeError):
    """The screenshot was not a PNG this module can read."""


def _paeth(a, b, c):
    p = a + b - c
    pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
    if pa <= pb and pa <= pc:
        return a
    return b if pb <= pc else c


def png_pixels(data):
    """`(width, height, pixels)` for a non-interlaced 8-bit RGB or RGBA PNG.

    `pixels` is a flat `bytearray` of RGB triples in row-major order: the alpha channel is
    dropped, because every claim here is about a colour on an opaque page.
    """
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise PngError("the screenshot is not a PNG")
    idat, width, height, depth, colour, interlace = bytearray(), 0, 0, 0, 0, 0
    at = 8
    while at < len(data):
        (length,) = struct.unpack(">I", data[at:at + 4])
        kind = data[at + 4:at + 8]
        body = data[at + 8:at + 8 + length]
        at += 12 + length
        if kind == b"IHDR":
            width, height, depth, colour, _, _, interlace = struct.unpack(">IIBBBBB", body)
        elif kind == b"IDAT":
            idat += body
        elif kind == b"IEND":
            break
    if depth != 8 or interlace != 0 or colour not in (2, 6):
        raise PngError(f"unsupported PNG: depth {depth}, colour {colour}, interlace {interlace}")
    channels = 3 if colour == 2 else 4
    raw = zlib.decompress(bytes(idat))
    stride = width * channels
    if len(raw) != (stride + 1) * height:
        raise PngError(f"the PNG's rows are {len(raw)} bytes, not {(stride + 1) * height}")
    out = bytearray(stride * height)
    prior = bytearray(stride)
    at = 0
    for y in range(height):
        filter_type, at = raw[at], at + 1
        line = bytearray(raw[at:at + stride])
        at += stride
        if filter_type == 1:
            for i in range(channels, stride):
                line[i] = (line[i] + line[i - channels]) & 0xFF
        elif filter_type == 2:
            for i in range(stride):
                line[i] = (line[i] + prior[i]) & 0xFF
        elif filter_type == 3:
            for i in range(stride):
                left = line[i - channels] if i >= channels else 0
                line[i] = (line[i] + ((left + prior[i]) >> 1)) & 0xFF
        elif filter_type == 4:
            for i in range(stride):
                left = line[i - channels] if i >= channels else 0
                upper_left = prior[i - channels] if i >= channels else 0
                line[i] = (line[i] + _paeth(left, prior[i], upper_left)) & 0xFF
        elif filter_type != 0:
            raise PngError(f"unknown PNG filter {filter_type}")
        out[y * stride:(y + 1) * stride] = line
        prior = line
    if channels == 3:
        return width, height, out
    packed = bytearray(width * height * 3)
    for i in range(width * height):
        packed[i * 3:i * 3 + 3] = out[i * 4:i * 4 + 3]
    return width, height, packed


def screenshot(page):
    """The composite the compositor drew, as `(width, height, pixels)`."""
    shot = page.call("Page.captureScreenshot", {"format": "png", "fromSurface": True})
    import base64
    return png_pixels(base64.b64decode(shot["data"]))


# ------------------------------------------------------------------------- the page, driven


class Chrome:
    """One page, one viewport, and the input a hand would send."""

    def __init__(self, page, url):
        self.page = page
        self.url = url
        self.width, self.height, self.dpr = 0, 0, 0

    def open(self, width, height, dpr):
        """A viewport of `width`x`height` CSS px at `dpr`, and the studio drawn in it."""
        self.width, self.height, self.dpr = width, height, dpr
        self.page.call("Page.addScriptToEvaluateOnNewDocument", {"source": CONSOLE_HOOK})
        # The scrollbars are what turns a page-sized element into a page that scrolls,
        # so the run asks for them by name rather than hoping for the default.
        self.page.call("Emulation.setScrollbarsHidden", {"hidden": False})
        self.page.set_viewport(width, height, dpr)
        self.page.navigate("about:blank")
        self.page.navigate(self.url)
        self.page.evaluate("customElements.whenDefined('graph-studio')")
        deadline = time.monotonic() + OPEN_S
        while time.monotonic() < deadline:
            at = self.page.evaluate("""
            (() => {
              const host = document.querySelector('graph-studio');
              const studio = host === null ? null : host.studio;
              if (studio === null) return null;
              const state = studio.store.get();
              return { done: state.busy.length === 0 && state.meta !== null, error: state.error };
            })()
            """)
            if at is not None and at["error"] is not None:
                raise cdp.CdpError(f"the studio failed to open: {at['error']}")
            if at is not None and at["done"]:
                time.sleep(1.5)  # the nodes are still travelling to the layout's positions
                return self
            time.sleep(0.2)
        raise cdp.CdpError(f"the studio drew no graph in {OPEN_S:.0f}s")

    # ------------------------------------------------------------------------- reading

    def metrics(self):
        """The layout numbers a strip on the right edge would have to show up in."""
        return self.page.evaluate("""
        (() => {
          const host = document.querySelector('graph-studio');
          const canvas = host === null ? null : host.shadowRoot.querySelector('canvas');
          const box = host === null ? null : host.getBoundingClientRect();
          const page = document.documentElement;
          return {
            innerWidth: innerWidth, innerHeight: innerHeight, dpr: devicePixelRatio,
            scrollWidth: page.scrollWidth, clientWidth: page.clientWidth,
            scrollHeight: page.scrollHeight, clientHeight: page.clientHeight,
            right: box === null ? null : box.right, bottom: box === null ? null : box.bottom,
            left: box === null ? null : box.left, top: box === null ? null : box.top,
            hostWidth: box === null ? null : box.width, hostHeight: box === null ? null : box.height,
            canvasWidth: canvas === null ? null : canvas.width,
            canvasHeight: canvas === null ? null : canvas.height,
            theme: host.studio.store.get().settings.appearance.theme,
          };
        })()
        """)

    def screenshot(self):
        """The composite, as `(width, height, pixels)`. A method so the rows read the same
        way whether they are looking at the canvas' own store or the page around it."""
        return screenshot(self.page)

    def canvas_report(self):
        """The canvas' own backing store: its size, the colour it was filled with, and the
        colours it is painting — over the whole canvas on a grid, and over its own outer
        `EDGE_SCAN` device rows and columns at full resolution.

        The full-resolution edge is what decides the strip rows. A grid stride walks past a
        12 px label glyph without landing on it, so a label at the right edge is a colour
        the canvas did paint and the grid never saw; read at the edge itself, every colour
        there is by definition the canvas', and what is left over is what the page put
        beside it. The width is scanned in device pixels wide enough for the 8 CSS px band
        at any ratio the renderer will allocate (it clamps at 2).
        """
        return self.page.evaluate(f"""
        (() => {{
          const canvas = document.querySelector('graph-studio').shadowRoot.querySelector('canvas');
          const ctx = canvas.getContext('2d');
          const all = ctx.getImageData(0, 0, canvas.width, canvas.height).data;
          const at = (x, y) => {{
            const i = (y * canvas.width + x) * 4;
            return [all[i], all[i + 1], all[i + 2]];
          }};
          const add = (set, x, y) => {{
            const p = at(x, y);
            set.add((p[0] << 16) | (p[1] << 8) | p[2]);
          }};
          const colours = new Set();
          for (let y = 0; y < canvas.height; y += {COLOUR_STRIDE}) {{
            for (let x = 0; x < canvas.width; x += {COLOUR_STRIDE}) add(colours, x, y);
          }}
          const edge = new Set();
          const deep = Math.min({EDGE_SCAN}, canvas.width, canvas.height);
          for (let y = 0; y < canvas.height; y += 1) {{
            for (let x = canvas.width - deep; x < canvas.width; x += 1) add(edge, x, y);
          }}
          for (let y = canvas.height - deep; y < canvas.height; y += 1) {{
            for (let x = 0; x < canvas.width; x += 1) add(edge, x, y);
          }}
          return {{
            width: canvas.width, height: canvas.height,
            corner: at(1, 1), centre: at(canvas.width >> 1, canvas.height >> 1),
            distinct: colours.size, edgeDistinct: edge.size,
            colours: Array.from(colours).slice(0, {COLOUR_CAP}),
            edgeColours: Array.from(edge).slice(0, {COLOUR_CAP}),
          }};
        }})()
        """)

    def chrome_boxes(self):
        """The rects of the studio's own painted boxes inside its shadow root.

        This is the other half of "is that strip the page or the studio". The chrome is
        allowed to reach the edge band — a panel is inset by 12 px, but a panel wider than
        the viewport (the HUD is `white-space: nowrap` and is 448 px at a 320 px viewport) is
        clipped by the host and paints right up to the edge. `elementFromPoint` cannot be
        used to find that: a clipped box is not hit-testable where it was clipped, so the
        probe reports the host. The boxes' own rects can, and a page background showing
        through is outside every one of them.
        """
        return [tuple(box) for box in self.page.evaluate("""
        (() => {
          const host = document.querySelector('graph-studio');
          const shadow = host.shadowRoot;
          const frame = host.getBoundingClientRect();
          const area = Math.max(1, frame.width * frame.height);
          const boxes = [];
          for (const element of shadow.querySelectorAll('*')) {
            const r = element.getBoundingClientRect();
            // A wrapper that covers the host is not a painted box: .gs-root is inset:0 and
            // would make every pixel in the band look like the studio's own.
            if (r.width <= 0 || r.height <= 0) continue;
            if (r.width * r.height >= 0.8 * area) continue;
            boxes.push([r.left, r.top, r.right, r.bottom]);
          }
          return boxes;
        })()
        """)]

    def console_log(self):
        """What the page logged, by the hook installed before it loaded."""
        return self.page.evaluate("(window.__chromeLog || []).slice(0, 40)")

    # -------------------------------------------------------------------------- hands

    def centre_of(self, at):
        return self.page.evaluate(
            f"(() => {{ const r = document.querySelector('graph-studio')"
            f".getBoundingClientRect(); return [r.left + {at[0]}, r.top + {at[1]}]; }})()"
        )

    def click(self, at, clicks=1):
        x, y = at
        for kind, detail in (("mousePressed", 1), ("mouseReleased", 0)):
            self.page.call("Input.dispatchMouseEvent", {
                "type": kind, "x": x, "y": y, "button": "left", "buttons": detail,
                "clickCount": clicks,
            })

    def key(self, key, code, vk, text=""):
        """A press and a release, with the text a typing key would insert."""
        for kind in ("keyDown", "keyUp"):
            params = {
                "type": kind, "key": key, "code": code,
                "windowsVirtualKeyCode": vk, "nativeVirtualKeyCode": vk,
            }
            if kind == "keyDown" and text:
                params["text"] = text
            self.page.call("Input.dispatchKeyEvent", params)

    def type_text(self, text):
        for character in text:
            if character == " ":
                self.key(" ", "Space", 32, " ")
            else:
                self.key(character, f"Key{character.upper()}", ord(character.upper()), character)

    def focus_page(self):
        """A click on the canvas, so the page has the keyboard the way a click gives it."""
        self.click(self.centre_of((self.width // 2, self.height // 2)))

    def console_open(self):
        return self.page.evaluate(
            "document.querySelector('graph-studio').shadowRoot.querySelector('.gs-console') !== null")

    def toggle_console(self):
        """` is the studio's own shortcut for its console, and the console's Escape."""
        self.focus_page()
        time.sleep(SETTLE_S)
        self.key("`", "Backquote", 192, "`")
        time.sleep(SETTLE_S)
        return self.console_open()

    def ensure_console(self, want_open):
        """The console in a stated state, reached by the shortcut however it started.

        A row must not depend on which row ran before it, so this reads the state first
        and presses the shortcut only when the state is not the one asked for.
        """
        for _ in range(2):
            if self.console_open() is want_open:
                return True
            self.toggle_console()
        return self.console_open() is want_open

    def close_console(self):
        self.ensure_console(False)

    def _shadow_box(self, expression):
        """The centre of an element inside the studio's shadow root, in page coordinates."""
        at = self.page.evaluate(f"""
        (() => {{
          const shadow = document.querySelector('graph-studio').shadowRoot;
          const element = {expression};
          if (element === null) return null;
          const r = element.getBoundingClientRect();
          return [r.left + r.width / 2, r.top + r.height / 2];
        }})()
        """)
        if at is None:
            raise cdp.CdpError(f"the studio has no {expression}")
        return at

    def type_command(self, line):
        """The console's own input, clicked and typed into, the way a hand does it."""
        at = self._shadow_box(
            "shadow.querySelector('input[aria-label=\"Console command\"]')")
        self.click(at)
        time.sleep(SETTLE_S)
        self.type_text(line)
        self.key("Enter", "Enter", 13, "\r")
        time.sleep(SETTLE_S)

    def dock_open(self):
        return self.page.evaluate("""
        (() => {
          const dock = document.querySelector('graph-studio').shadowRoot.querySelector('.gs-dock-body');
          return dock === null ? null : !dock.hasAttribute('hidden');
        })()
        """)

    def toggle_dock(self):
        """The dock's own Controls button, clicked where it is drawn."""
        at = self._shadow_box("""
        Array.from(shadow.querySelectorAll('button.gs-section-head'))
          .find((b) => b.textContent.includes('Controls')) || null
        """)
        self.click(at)
        time.sleep(SETTLE_S)
        return self.dock_open()
