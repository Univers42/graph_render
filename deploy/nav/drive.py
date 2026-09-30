"""The real input a user's hand sends: CDP mouse, wheel and key events on one page.

Nothing here reaches into the studio: it dispatches the same events a hand does, and the
rows read the camera back through the element's own view.
"""
import time

import cdp

# Background points: the navigation rows are about the background, and a node under the
# cursor would make a double-click the node's own gesture (S2).
CENTRE = (700, 450)
OFF_CENTRE = (420, 320)
DRAG_PX = 200
# A hand does not teleport: a drag is dispatched in 20 steps of 10 px. The first four
# pixels are the view's own slop; the rows are about the camera afterwards.
DRAG_STEPS = 20
DRAG_TOLERANCE = 0.5
# The world point under the cursor must not move by more than this. Half a pixel is what a
# rasteriser is asked to draw anyway; a visible slide is more than that.
ANCHOR_TOLERANCE = 0.5
WHEEL_NOTCH = -240
PINCH_NOTCH = -60
# How many presses of + or - it takes to reach a clamp from wherever the camera is.
CLAMP_PRESSES = 12
# How long a row waits for the drawing to stop moving before it reads the camera.
SETTLE_S = 0.35
VIEWPORT = (1400, 900)


def screen_to_world(camera, point):
    return {
        "x": (point[0] - camera["x"]) / camera["scale"],
        "y": (point[1] - camera["y"]) / camera["scale"],
    }


def world_to_screen(camera, world):
    return {
        "x": world["x"] * camera["scale"] + camera["x"],
        "y": world["y"] * camera["scale"] + camera["y"],
    }


def apart(a, b):
    return ((a["x"] - b["x"]) ** 2 + (a["y"] - b["y"]) ** 2) ** 0.5


# ------------------------------------------------------------------------- driving real input


class Studio:
    """The page, and the real input a user's hand would send."""

    def __init__(self, page, url, expect_drag=None):
        self.page = page
        self.url = url
        self.expect_drag = DRAG_PX if expect_drag is None else expect_drag

    def open(self):
        self.page.set_viewport(VIEWPORT[0], VIEWPORT[1], 1)
        self.page.navigate("about:blank")
        self.page.navigate(self.url)
        self.page.evaluate("customElements.whenDefined('graph-studio')")
        deadline = time.monotonic() + 90
        while time.monotonic() < deadline:
            state = self.page.evaluate("""
            (() => {
              const s = document.querySelector('graph-studio').studio;
              if (s === null) return null;
              const at = s.store.get();
              return { done: at.busy.length === 0 && at.meta !== null, error: at.error };
            })()
            """)
            if state is None:
                time.sleep(0.2)
                continue
            if state["error"] is not None:
                raise cdp.CdpError(f"the studio failed to open: {state['error']}")
            if state["done"]:
                # The nodes travel to where the layout put them; a camera read mid-flight
                # would be a camera read of a drawing that is still arriving.
                time.sleep(1.5)
                return
            time.sleep(0.2)
        raise cdp.CdpError("the studio drew no graph in 90s")

    def read(self):
        report = self.page.evaluate("""
        (() => {
          const host = document.querySelector('graph-studio');
          const studio = host === null ? null : host.studio;
          const view = host === null ? null : host.view;
          const canvas = host === null ? null : host.shadowRoot.querySelector('canvas');
          if (studio === null || view === null || canvas === null) return null;
          const box = canvas.getBoundingClientRect();
          return {
            camera: view.camera(), limits: view.limits(), selected: studio.store.get().selected,
            box: [box.left, box.top, box.width, box.height], dpr: devicePixelRatio,
            nodes: view.stats().nodes,
          };
        })()
        """)
        if report is None:
            raise cdp.CdpError("the page has no graph-studio with a studio and a view on it")
        return report

    def camera(self):
        return self.read()["camera"]

    def settle(self):
        time.sleep(SETTLE_S)
        return self.camera()

    def press_at(self, x, y, button, buttons, clicks=1):
        self.page.call("Input.dispatchMouseEvent", {
            "type": "mousePressed", "x": x, "y": y, "button": button, "buttons": buttons, "clickCount": clicks,
        })

    def release_at(self, x, y, button, clicks=1):
        self.page.call("Input.dispatchMouseEvent", {
            "type": "mouseReleased", "x": x, "y": y, "button": button, "buttons": 0, "clickCount": clicks,
        })

    def drag(self, at, dx, dy, button="left", buttons=1):
        x, y = at
        self.press_at(x, y, button, buttons)
        for step in range(1, DRAG_STEPS + 1):
            self.page.call("Input.dispatchMouseEvent", {
                "type": "mouseMoved", "x": x + dx * step / DRAG_STEPS, "y": y + dy * step / DRAG_STEPS,
                "button": button, "buttons": buttons,
            })
        self.release_at(x + dx, y + dy, button)

    def wheel(self, at, delta_y, ctrl=False):
        self.page.call("Input.dispatchMouseEvent", {
            "type": "mouseWheel", "x": at[0], "y": at[1], "deltaX": 0, "deltaY": delta_y,
            "modifiers": 2 if ctrl else 0,
        })

    def click(self, at, button="left", buttons=1, clicks=1):
        self.press_at(at[0], at[1], button, buttons, clicks)
        self.release_at(at[0], at[1], button, clicks)

    def double_click(self, at):
        self.click(at, clicks=1)
        self.press_at(at[0], at[1], "left", 1, clicks=2)
        self.release_at(at[0], at[1], "left", clicks=2)

    def key(self, name, code=None, vk=0):
        keys = {"f": ("f", "KeyF", 70), "0": ("0", "Digit0", 48), "+": ("+", "Equal", 187),
                "=": ("=", "Equal", 187), "-": ("-", "Minus", 189), "_": ("_", "Minus", 189),
                "Escape": ("Escape", "Escape", 27), "ArrowLeft": ("ArrowLeft", "ArrowLeft", 37),
                "ArrowRight": ("ArrowRight", "ArrowRight", 39), "ArrowUp": ("ArrowUp", "ArrowUp", 38),
                "ArrowDown": ("ArrowDown", "ArrowDown", 40), " ": (" ", "Space", 32)}
        if name not in keys:
            raise cdp.CdpError(f"no key named {name}")
        key, code_name, key_vk = keys[name]
        for kind in ("rawKeyDown", "keyUp"):
            self.page.call("Input.dispatchKeyEvent", {
                "type": kind, "key": key, "code": code or code_name, "windowsVirtualKeyCode": vk or key_vk,
                "nativeVirtualKeyCode": vk or key_vk,
            })

    def hold_space(self):
        self.page.call("Input.dispatchKeyEvent", {
            "type": "rawKeyDown", "key": " ", "code": "Space", "windowsVirtualKeyCode": 32,
            "nativeVirtualKeyCode": 32,
        })

    def release_space(self):
        self.page.call("Input.dispatchKeyEvent", {
            "type": "keyUp", "key": " ", "code": "Space", "windowsVirtualKeyCode": 32,
            "nativeVirtualKeyCode": 32,
        })

    def focus_page(self):
        """Keys go to the studio, which listens on the page: focus it as a click would."""
        self.page.call("Runtime.evaluate", {"expression": "document.querySelector('graph-studio').focus()"})

    def find_node(self):
        """A screen point that hits a node, found by asking the view's own hit-test."""
        return self.page.evaluate("""
        (() => {
          const view = document.querySelector('graph-studio').view;
          for (let y = 60; y < 840; y += 7) {
            for (let x = 60; x < 1340; x += 7) {
              if (view.pick({ x, y }) >= 0) return [x, y];
            }
          }
          return null;
        })()
        """)

    def background(self, preferred):
        """The point nearest `preferred` that no node is under, by the view's own hit-test.

        Ponytail: the search steps 12 px out to 480 px and stops at the first miss with a 14 px
        clear margin on four sides; a graph that covers the whole canvas returns `preferred`.
        """
        found = self.page.evaluate(f"""
        (() => {{
          const view = document.querySelector('graph-studio').view;
          const clear = (x, y) => [[0, 0], [14, 0], [-14, 0], [0, 14], [0, -14]].every(([dx, dy]) => view.pick({{ x: x + dx, y: y + dy }}) < 0);
          for (let r = 0; r <= 480; r += 12) {{
            for (let a = 0; a < 16; a += 1) {{
              const x = Math.round({preferred[0]} + r * Math.cos(a * Math.PI / 8));
              const y = Math.round({preferred[1]} + r * Math.sin(a * Math.PI / 8));
              if (x > 20 && x < 1380 && y > 20 && y < 880 && clear(x, y)) return [x, y];
            }}
          }}
          return null;
        }})()
        """)
        return tuple(found) if found is not None else tuple(preferred)

    def border_drawn(self):
        """How many pixels of the canvas' outer band are not the background it was filled with."""
        return self.page.evaluate("""
        (() => {
          const canvas = document.querySelector('graph-studio').shadowRoot.querySelector('canvas');
          const ctx = canvas.getContext('2d');
          const d = ctx.getImageData(0, 0, canvas.width, canvas.height).data;
          const at = (x, y) => { const i = (y * canvas.width + x) * 4; return [d[i], d[i + 1], d[i + 2]]; };
          const ref = at(0, 0);
          const same = (p) => p[0] === ref[0] && p[1] === ref[1] && p[2] === ref[2];
          const band = 6;
          let drawn = 0;
          for (let x = 0; x < canvas.width; x += 1) {
            for (const y of [0, 1, band - 1, canvas.height - band, canvas.height - 2, canvas.height - 1]) {
              if (!same(at(x, y))) drawn += 1;
            }
          }
          for (let y = 0; y < canvas.height; y += 1) {
            for (const x of [0, 1, band - 1, canvas.width - band, canvas.width - 2, canvas.width - 1]) {
              if (!same(at(x, y))) drawn += 1;
            }
          }
          return drawn;
        })()
        """)
