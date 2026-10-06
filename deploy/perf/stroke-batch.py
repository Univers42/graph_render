"""Canvas2D stroke raster time by stroke width and segments per stroke, on a 3840x2160 canvas.

    scripts/studio-probe.sh stroke-batch

Needs no build: the page is about:blank. Prints one line per (width, segments per stroke), the mean
of 6 frames after 2 warm-up frames, over 1242 seeded segments up to 5760 px long. The table in
docs/measurements/studio-thick-edges.md is its output; canvas2d/edges.ts `chunkOf` is the decision.

Caveat: the browser's own rasteriser, on whatever host runs it. SwiftShader here, so a GPU host can
rank the batch sizes differently; the segments are random, not a graph's.
"""
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "nav"))

import nav
import smokecdp

BENCH = r"""
async () => {
  const canvas = document.createElement("canvas");
  canvas.width = 3840; canvas.height = 2160;
  canvas.style.width = "1920px"; canvas.style.height = "1080px";
  document.body.replaceChildren(canvas);
  const ctx = canvas.getContext("2d");
  let seed = 7; const rnd = () => ((seed = (seed * 1103515245 + 12345) % 2147483648) / 2147483648);
  const N = 1242; const seg = [];
  for (let i = 0; i < N; i += 1) seg.push([rnd() * 5760 - 960, rnd() * 3240 - 540, rnd() * 5760 - 960, rnd() * 3240 - 540]);
  const native = requestAnimationFrame.bind(window);
  const next = () => new Promise((r) => native(r));
  const frame = (width, chunk, mode) => {
    ctx.setTransform(1, 0, 0, 1, 0, 0); ctx.fillStyle = "#fff"; ctx.fillRect(0, 0, 3840, 2160);
    ctx.strokeStyle = "rgba(80,80,80,0.6)"; ctx.lineWidth = width;
    if (mode === "path") {
      ctx.beginPath(); let n = 0;
      for (const [a, b, c, d] of seg) { ctx.moveTo(a, b); ctx.lineTo(c, d); n += 1; if (n >= chunk) { ctx.stroke(); ctx.beginPath(); n = 0; } }
      if (n) ctx.stroke();
    }
  };
  const time = async (width, chunk, mode) => {
    for (let i = 0; i < 2; i += 1) { frame(width, chunk, mode); await next(); }
    const t0 = performance.now();
    for (let i = 0; i < 6; i += 1) { frame(width, chunk, mode); await next(); }
    return +((performance.now() - t0) / 6).toFixed(1);
  };
  const out = [];
  const runs = [[0, 1, "none"]];
  for (const width of [1, 1.2, 1.5, 2, 3]) for (const chunk of [2048, 16, 4, 1]) runs.push([width, chunk, "path"]);
  for (const [width, chunk, mode] of runs) {
    out.push(`${mode} width ${width} chunk ${chunk}: ${await time(width, chunk, mode)} ms/frame`);
  }
  return out.join("\n");
}
"""


def main():
    with tempfile.TemporaryDirectory() as profile:
        browser = nav.launch_browser(profile)
        try:
            page = smokecdp.Watcher(nav.DEBUG_PORT)
            page.set_viewport(1920, 1080, 2)
            page.navigate("about:blank")
            print(page.evaluate(f"({BENCH})()", timeout=600))
        finally:
            browser.terminate()
            browser.wait(timeout=10)
    return 0


if __name__ == "__main__":
    sys.exit(main())
