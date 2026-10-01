"""The page, and the real input a user's hand sends to a 3D drawing.

Nothing here reaches into the studio's internals to satisfy a row. The layout is chosen by
typing its id into the console, as a user does; the camera is moved with CDP mouse events;
and the node positions the rows read are the view's own projected points, which is the same
number the painter drew from.
"""
import time

import cdp

# A closed-form 3D layout: the sphere places its nodes on a shell, so the drawing is a
# recognisable shape rather than a flat line the first drag could be excused for.
SPACE_LAYOUT = "layout.basic3d.sphere"
# A drag long enough to turn the drawing visibly: 200 px is a third of a turn (orbit.ts).
DRAG_PX = 200
DRAG_STEPS = 20
# How long a row waits for the drawing to stop moving before it reads the projection.
SETTLE_S = 0.4
VIEWPORT = (1280, 800)
# The point the drag starts from: the middle of the canvas, above the HUD.
CENTRE = (640, 380)


class Space:
    def __init__(self, page, url, frozen=False):
        self.page = page
        self.url = url
        # The negative control's fault: the camera is held at the orbit it had, so no drag
        # can move it and every row that claims a drag changed something must fail.
        self.frozen = frozen

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
                self.choose_layout()
                self.settle()
                return
            time.sleep(0.2)
        raise cdp.CdpError("the studio drew no graph in 90s")

    # ---------------------------------------------------------------- choosing the layout

    def choose_layout(self):
        """Types the layout id into the console, which is how a user picks one."""
        self.console(f"layout {SPACE_LAYOUT}")
        deadline = time.monotonic() + 60
        while time.monotonic() < deadline:
            run = self.page.evaluate(f"""
            (() => {{
              const s = document.querySelector('graph-studio').studio.store.get();
              return s.run === null ? null : {{ id: s.run.layoutId, dim: s.run.dim }};
            }})()
            """)
            if run is not None and run["id"] == SPACE_LAYOUT and run["dim"] == 1:
                self.settle()
                return
            time.sleep(0.2)
        raise cdp.CdpError(f"the studio never ran {SPACE_LAYOUT} in 3D")

    def console(self, text):
        """Opens the console with the studio's own key if it is closed, then types a command.

        The console is not in the DOM until it is opened (`Shell.tsx:97`), and the key that
        opens it is the studio's own shortcut. The toggle is only sent when the input is not
        already there: the key is a toggle, so pressing it on an open console would close it
        and the line would be typed into nothing.
        """
        if not self.console_open():
            self.page.call("Input.dispatchKeyEvent", {
                "type": "rawKeyDown", "key": "`", "code": "Backquote",
                "windowsVirtualKeyCode": 192, "nativeVirtualKeyCode": 192,
            })
            self.page.call("Input.dispatchKeyEvent", {"type": "keyUp", "key": "`", "code": "Backquote"})
        opened = self.page.evaluate("""
        (() => {
          const input = document.querySelector('graph-studio').shadowRoot
            .querySelector('input[aria-label="Console command"]');
          if (input === null) return 'the console did not open';
          input.focus();
          return 'ok';
        })()
        """)
        if opened != "ok":
            raise cdp.CdpError(opened)
        return self.type_console(text)

    def console_open(self):
        return self.page.evaluate("""
        (() => document.querySelector('graph-studio').shadowRoot
          .querySelector('input[aria-label="Console command"]') !== null)()
        """)

    def shoot(self, path, name):
        """Saves what the drawing looks like, for a person and not for a row.

        No row reads a pixel: the numbers are the camera's, and a software rasteriser's output
        is not what any of them claims. This is the evidence for the claim no row can make —
        that the shape on screen is the shape the layout laid out. It returns nothing, so it
        cannot be mistaken for a row that ran.

        Two animation frames are waited for first, and that is not politeness: a capture
        lands on whatever the compositor has presented, and under software raster the drawing
        a camera change asked for is often one frame behind. Sleeping instead saved three
        identical files, which is a screenshot that lies about the camera.
        """
        self.page.evaluate("""
        new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(() => done('ok'))))
        """)
        self.page.screenshot(str(path / name))

    def type_console(self, text):
        """Types into the focused console input the way a hand does: real key events.

        A `keyDown` with `text` is what Chromium turns into a character, so each letter is
        one pair of events rather than one `insertText`, which the console's own input would
        never see. Enter is dispatched as a full key press, the way a hand's is.
        """
        for character in text:
            self.page.call("Input.dispatchKeyEvent", {"type": "keyDown", "text": character})
            self.page.call("Input.dispatchKeyEvent", {"type": "keyUp", "text": character})
        self.page.call("Input.dispatchKeyEvent", {
            "type": "rawKeyDown", "key": "Enter", "code": "Enter",
            "windowsVirtualKeyCode": 13, "nativeVirtualKeyCode": 13,
        })
        self.page.call("Input.dispatchKeyEvent", {"type": "keyUp", "key": "Enter", "code": "Enter"})
        return "ok"

    # ------------------------------------------------------------------------- reading it

    def positions(self):
        """The camera, and every node's projected screen point, as the view drew it.

        The points are read back out of the view's own projection rather than off the
        canvas: a row about a camera wants the camera's numbers, and a software rasteriser's
        pixels are not them. The pixel claim is a screenshot's, not this gate's.
        """
        found = self.page.evaluate("""
        (() => {
          const view = document.querySelector('graph-studio').view;
          const orbit = view.orbit();
          if (orbit === null) return null;
          const frame = view.frame();
          const points = view.projected();
          return {
            orbit: { yaw: orbit.yaw, pitch: orbit.pitch, distance: orbit.distance },
            z: Array.from(frame.z ?? []),
            points: points === null ? null : points.map((at) => ({ node: at.node, x: at.x, y: at.y, depth: at.depth })),
          };
        })()
        """)
        if found is None:
            raise cdp.CdpError("the view has no 3D camera: the drawing is 2D")
        return found

    def settled_positions(self):
        """The projection after the drawing has stopped moving."""
        time.sleep(SETTLE_S)
        return self.positions()

    def settle(self):
        time.sleep(SETTLE_S)

    def freeze_camera(self):
        """The negative control's fault, applied from the probe and not from the app.

        A rAF loop pins the orbit to the one it holds, so every drag the gate sends is
        accepted by the view and then thrown away. The drawing goes on repainting, and the
        projected positions never move — which is what "the camera is frozen" means, and it
        is a fault a user's hand could not tell from a broken gesture handler.
        """
        self.page.evaluate("""
        (() => {
          const host = document.querySelector('graph-studio');
          const held = host.view.orbit();
          if (held === null) return 'no orbit to freeze';
          window.__heldOrbit = held;
          const pin = () => {
            if (window.__heldOrbit !== null && window.__heldOrbit !== undefined) {
              host.view.setOrbit(window.__heldOrbit);
            }
            requestAnimationFrame(pin);
          };
          requestAnimationFrame(pin);
          return 'frozen';
        })()
        """)

    # --------------------------------------------------------------------- driving real input

    def drag(self, at, dx, dy, button="left", buttons=1):
        x, y = at
        self.page.call("Input.dispatchMouseEvent", {
            "type": "mousePressed", "x": x, "y": y, "button": button, "buttons": buttons, "clickCount": 1,
        })
        for step in range(1, DRAG_STEPS + 1):
            self.page.call("Input.dispatchMouseEvent", {
                "type": "mouseMoved", "x": x + dx * step / DRAG_STEPS, "y": y + dy * step / DRAG_STEPS,
                "button": button, "buttons": buttons,
            })
        self.page.call("Input.dispatchMouseEvent", {
            "type": "mouseReleased", "x": x + dx, "y": y + dy, "button": button, "buttons": 0,
        })

    def wheel(self, at, delta_y):
        self.page.call("Input.dispatchMouseEvent", {
            "type": "mouseWheel", "x": at[0], "y": at[1], "deltaX": 0, "deltaY": delta_y, "modifiers": 0,
        })

    def background(self):
        """A point no node is under, by the view's own hit-test, so the drag is the camera."""
        found = self.page.evaluate("""
        (() => {
          const view = document.querySelector('graph-studio').view;
          const clear = (x, y) => [[0, 0], [16, 0], [-16, 0], [0, 16], [0, -16]]
            .every(([dx, dy]) => view.pick({ x: x + dx, y: y + dy }) < 0);
          for (let r = 0; r <= 460; r += 14) {
            for (let a = 0; a < 16; a += 1) {
              const x = Math.round(640 + r * Math.cos(a * Math.PI / 8));
              const y = Math.round(380 + r * Math.sin(a * Math.PI / 8));
              if (x > 30 && x < 1250 && y > 30 && y < 770 && clear(x, y)) return [x, y];
            }
          }
          return null;
        })()
        """)
        return tuple(found) if found is not None else CENTRE