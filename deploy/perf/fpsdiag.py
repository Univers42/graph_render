"""Scratch: where a zoomed-in frame at 2000 nodes, DPR 2 spends its time (not committed)."""
import json
import os
import sys
import tempfile
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "nav"))

import nav
import smokecdp

PROBE = r"""
async (skip) => {
  const host = document.querySelector("graph-studio");
  const canvas = host.shadowRoot.querySelector("canvas");
  const box = canvas.getBoundingClientRect();
  const proto = CanvasRenderingContext2D.prototype;
  const saved = {};
  const lineTo = proto.lineTo, stroke = proto.stroke, begin = proto.beginPath;
  for (const name of skip) {
    if (name === "perEdge") {
      saved.lineTo = lineTo;
      proto.lineTo = function (x, y) { lineTo.call(this, x, y); if (this.lineWidth * devicePixelRatio > 1.01 && this.strokeStyle !== this.fillStyle) { stroke.call(this); begin.call(this); } };
      continue;
    }
    saved[name] = proto[name]; proto[name] = function () {};
  }
  const native = requestAnimationFrame.bind(window);
  const next = () => new Promise((r) => native((t) => r(t)));
  const wheel = (dy) => { const t0 = performance.now(); canvas.dispatchEvent(new WheelEvent("wheel", {
    deltaY: dy, clientX: box.left + box.width / 2, clientY: box.top + box.height / 2, bubbles: true, cancelable: true })); return performance.now() - t0; };
  const long = []; const obs = new PerformanceObserver((l) => { for (const e of l.getEntries()) long.push(e.duration); });
  obs.observe({ type: "longtask" });
  const run = async (steps, dy) => {
    let wheelMs = 0; const t0 = performance.now(); long.length = 0;
    for (let i = 0; i < steps; i += 1) { wheelMs += wheel(dy); await next(); }
    const wall = performance.now() - t0;
    for (const e of obs.takeRecords()) long.push(e.duration);
    const s = host.view.stats();
    return { fps: +(1000 * steps / wall).toFixed(1), wheelMs: +wheelMs.toFixed(1), longs: long.length,
      longMax: Math.round(Math.max(0, ...long)), edges: s.drawnEdges, nodes: s.drawnNodes, labels: s.drawnLabels };
  };
  await new Promise((r) => setTimeout(r, 800));
  const zin = await run(40, -40);
  const zout = await run(40, 40);
  obs.disconnect();
  for (const [name, fn] of Object.entries(saved)) proto[name] = fn;
  return JSON.stringify({ skip, zin, zout });
}
"""


def main():
    nodes, dpr = int(sys.argv[1]), int(sys.argv[2])
    server = nav.serve("app/dist")
    with tempfile.TemporaryDirectory() as profile:
        browser = nav.launch_browser(profile)
        try:
            page = smokecdp.Watcher(nav.DEBUG_PORT)
            page.start_watching()
            page.set_viewport(1920, 1080, dpr)
            page.navigate(f"http://127.0.0.1:{server.server_address[1]}/" + (sys.argv[3] if len(sys.argv) > 3 else ""))
            page.evaluate(open("deploy/perf/drivers/hook.js").read())
            page.evaluate(f"window.__perf.open({nodes}, 'layout.forceatlas2.barnes_hut')", timeout=300)
            print("backend", page.evaluate("document.querySelector('graph-studio').view.stats().backend"))
            for skip in ([], ["perEdge"], [], ["perEdge"]):
                print(page.evaluate(f"({PROBE})({json.dumps(skip)})", timeout=600), flush=True)
        finally:
            browser.terminate()
            browser.wait(timeout=10)
            server.shutdown()
    return 0


if __name__ == "__main__":
    sys.exit(main())
