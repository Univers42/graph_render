// How long the GPU layer takes to fill its settled picture with every edge, polled from the
// page so the wait costs no CDP round trip per sample, with the frame times it saw on the way and
// a fingerprint of the pixels it drew.
//
// The wait is the loop's own flag: `refining` is true while the still picture still lacks a chunk
// (webgl2/hook.ts), false on the frame that completes it and on the frame after a camera move.
// The page reports it in `view.stats()`, so the probe needs no counter arithmetic. The counters are
// still read, as the check that the picture the flag reports full really holds every edge.
//
// The frame times are sampled on the same poll as the wait, so they are the times of frames the
// poll happened to land between, not every frame: enough to rank the frames a fill is made of, not
// a histogram of them.
//
// `pixelHash` fingerprints the canvas so two builds can be shown to have drawn the same pixels. The
// full-page screenshot the python side writes cannot say that: its status line reads the frame rate,
// so two runs of one build differ there. The whole file is one expression, because the python side
// wraps it in parentheses and calls it.
async (args) => {
  const view = window.__perf.view();
  if (view === null) throw new Error("the driver found no view");
  const round = (value) => Math.round(value * 100) / 100;
  const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
  const quantile = (from, q) => (from.length === 0 ? -1 : round(from.slice().sort((a, b) => a - b)[Math.floor((from.length - 1) * q)]));

  const wait = await waitForFull(view, args, sleep);
  const stats = wait.stats;
  const times = wait.times.slice().sort((a, b) => a - b);
  return {
    fullMs: wait.full < 0 ? -1 : Math.round(wait.full),
    polls: args.polls,
    backend: stats.backend,
    backendFailure: stats.backendFailure,
    refining: stats.refining,
    nodes: stats.nodes,
    edges: stats.edges,
    drawnEdges: stats.drawnEdges,
    drawnNodes: stats.drawnNodes,
    frames: stats.frames - wait.first.frames,
    sampledFrames: times.length,
    p50FrameMs: quantile(times, 0.5),
    p95FrameMs: quantile(times, 0.95),
    maxFrameMs: times.length === 0 ? -1 : round(times[times.length - 1]),
    lastFrameMs: round(stats.frameMs),
    pixelHash: await pixelHashOf(),
  };

  /** Polls until the flag says the picture is full, or for `polls` polls, keeping the frame times. */
  async function waitForFull(v, plan, nap) {
    const started = performance.now();
    const first = v.stats();
    const seen = [];
    let stats = first;
    for (let poll = 0; poll < plan.polls; poll += 1) {
      stats = v.stats();
      if (stats.frameMs > 0) seen.push(stats.frameMs);
      // The flag first, the counters as the check on it: a moving frame owes no chunk and reports
      // `refining` false while the picture is far from full.
      if (stats.edges > 0 && !stats.refining && stats.drawnEdges >= stats.edges) {
        return { full: performance.now() - started, stats, first, times: seen };
      }
      await nap(plan.everyMs);
    }
    return { full: -1, stats, first, times: seen };
  }

  /** The canvas' own bytes as an FNV-1a 32, over its whole area, once at the end of a run. */
  async function pixelHashOf() {
    const canvas = window.__perf.canvas();
    if (canvas === null) return "no canvas";
    const ctx = canvas.getContext("2d");
    if (ctx === null) return "no 2d";
    const { data } = ctx.getImageData(0, 0, canvas.width, canvas.height);
    let hash = 0x811c9dc5;
    for (let at = 0; at < data.length; at += 1) hash = Math.imul(hash ^ data[at], 0x01000193) >>> 0;
    return `${canvas.width}x${canvas.height}:${hash.toString(16)}`;
  }
}