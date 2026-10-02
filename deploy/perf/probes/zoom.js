// Sixty wheel events, one every 16 ms, at the canvas centre; the gaps between the frames the
// view painted meanwhile. `delta` < 0 zooms in. Returns JSON: frames, s, fps, p50/p95/max gap ms.
async (delta) => {
  const host = document.querySelector("graph-studio");
  const view = host.view;
  const canvas = host.shadowRoot.querySelector("canvas");
  const box = canvas.getBoundingClientRect();
  const at = { clientX: box.x + box.width / 2, clientY: box.y + box.height / 2 };
  const stamps = [];
  let last = view.stats().frames;
  let live = true;
  const watch = (time) => {
    const frames = view.stats().frames;
    if (frames !== last) { stamps.push(time); last = frames; }
    if (live) requestAnimationFrame(watch);
  };
  requestAnimationFrame(watch);
  const first = view.stats().frames;
  const started = performance.now();
  for (let event = 0; event < 60; event += 1) {
    canvas.dispatchEvent(new WheelEvent("wheel", { ...at, deltaY: delta, bubbles: true, cancelable: true, composed: true }));
    await new Promise((resolve) => setTimeout(resolve, 16));
  }
  const ended = performance.now();
  live = false;
  const frames = view.stats().frames - first;
  const gaps = stamps.slice(1).map((time, at) => time - stamps[at]).sort((a, b) => a - b);
  const gap = (q) => (gaps.length ? Math.round(gaps[Math.min(gaps.length - 1, Math.floor(q * gaps.length))] * 10) / 10 : -1);
  const stats = view.stats();
  return JSON.stringify({
    frames, s: Math.round(ended - started) / 1000, fps: Math.round((frames / (ended - started)) * 10000) / 10,
    p50: gap(0.5), p95: gap(0.95), max: gap(1), backend: stats.backend, nodes: stats.drawnNodes, edges: stats.drawnEdges,
  });
}
