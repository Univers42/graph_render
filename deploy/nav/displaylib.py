"""Page-side helpers for the display rows: one small object the rows call into.

`window.__d` holds the studio and its view. `set` dispatches an action the way the dock does
(`studio.dispatch(id, args)`) and waits for the next frames, so a row reads a drawing that has
finished changing. Nothing here paints: every number is the view's own state or one canvas pixel.
"""
import time

SETTLE_MS = 350

INSTALL = f"""
(() => {{
  const host = document.querySelector('graph-studio');
  const studio = host.studio, view = host.view;
  const sleep = (ms) => new Promise((done) => setTimeout(done, ms));
  const canvas = host.shadowRoot.querySelector('canvas');
  window.__d = {{
    studio, view, sleep, canvas,
    set: async (id, args) => {{
      const entry = await studio.dispatch(id, args);
      await sleep({SETTLE_MS});
      return entry;
    }},
    appearance: () => JSON.parse(JSON.stringify(studio.store.get().settings.appearance)),
    corner: () => Array.from(canvas.getContext('2d').getImageData(0, 0, 1, 1).data.slice(0, 3)),
    pixels: () => canvas.getContext('2d').getImageData(0, 0, canvas.width, canvas.height).data,
  }};
  return true;
}})()
"""


def install(studio):
    studio.page.evaluate(INSTALL)


def run(studio, body):
    """Evaluate `body` (an async function body using `d`) and return its JSON value."""
    return studio.page.evaluate(f"(async () => {{ const d = window.__d; {body} }})()")


def settle(seconds=SETTLE_MS / 1000):
    time.sleep(seconds)
