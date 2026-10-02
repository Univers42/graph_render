"""The frame rate of a continuous wheel zoom on N open nodes, in and then out (a probe, not a gate).

    docker run --rm --memory 10g --memory-swap 10g -v "$PWD:/w" -w /w gm-chromium \
      python3 deploy/perf/zoom.py 1000000 webgl2 [label]

Build first (scripts/studio.sh build). Prints one JSON line per direction, the page's errors, and
writes target/studio-zoom/<label>/{moving,settled}.png: a frame taken mid-zoom and the same view
once it has settled.

Why in-page wheel events: driving the zoom through CDP Input.dispatchMouseEvent measured the
input round-trip, not the renderer. A build that drew moving frames in half the time read lower
there (28.9 fps against 39.8 from in-page events, both at 1M nodes on webgl2).

Caveat: one event every 16 ms on a timer, so a renderer that keeps up reads about 60 fps and a
faster one cannot read higher; the gaps between frames count whatever else the page did, so one
garbage collection shows in max and p95. SwiftShader is a CPU rasteriser: the numbers rank
builds on one host, they are not what a GPU would show.
"""
import base64
import os
import sys
import tempfile
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "nav"))

import nav  # first: it puts the perf gate's CDP client on the path
import smokecdp
import smokerows

ZOOM = open("deploy/perf/probes/zoom.js").read()
NUDGE = """(() => { const c = document.querySelector('graph-studio').shadowRoot.querySelector('canvas');
  const b = c.getBoundingClientRect();
  for (let i = 0; i < 3; i += 1) c.dispatchEvent(new WheelEvent('wheel', { clientX: b.x + b.width / 2,
    clientY: b.y + b.height / 2, deltaY: -60, bubbles: true, cancelable: true, composed: true })); })()"""


def shoot(page, out, name):
    data = page.call("Page.captureScreenshot", {"format": "png"})["data"]
    with open(os.path.join(out, f"{name}.png"), "wb") as file:
        file.write(base64.b64decode(data))


def zoom(page, out):
    for direction, delta in (("in", -60), ("out", 60)):
        print(direction, page.evaluate(f"({ZOOM})({delta})", timeout=300))
        if direction == "in":
            page.evaluate(NUDGE)
            shoot(page, out, "moving")
            time.sleep(3.0)
            shoot(page, out, "settled")


def errors(page):
    print("exceptions", smokerows.events(page, "Runtime.exceptionThrown"))
    calls = smokerows.events(page, "Runtime.consoleAPICalled")
    print("console errors", [call for call in calls if "error" in call][:5])
    probe = page.evaluate(smokerows.PROBE)
    print("store error", probe["error"], "alert", probe.get("alert"))


def run(page, base, nodes, out):
    page.start_watching()
    page.set_viewport(1920, 1080, 1)
    page.navigate(base)
    page.evaluate(open("deploy/perf/drivers/hook.js").read())
    started = time.monotonic()
    page.evaluate(f"window.__perf.open({nodes}, 'layout.random')", timeout=300)
    print("open s", round(time.monotonic() - started, 2))
    time.sleep(1.0)
    zoom(page, out)
    errors(page)


def main():
    nodes, backend = int(sys.argv[1]), sys.argv[2]
    out = os.path.join("target/studio-zoom", sys.argv[3] if len(sys.argv) > 3 else "current")
    os.makedirs(out, exist_ok=True)
    server = nav.serve("app/dist")
    with tempfile.TemporaryDirectory() as profile:
        browser = nav.launch_browser(profile, extra=["--enable-unsafe-swiftshader"])
        try:
            served = f"http://127.0.0.1:{server.server_address[1]}/?backend={backend}"
            run(smokecdp.Watcher(nav.DEBUG_PORT), served, nodes, out)
        finally:
            browser.terminate()
            browser.wait(timeout=10)
            server.shutdown()


if __name__ == "__main__":
    main()
