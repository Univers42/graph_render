// How long the GPU layer takes to fill its settled picture with every edge, polled from the
// page so the wait costs no CDP round trip per sample, with the frame times it saw on the way.
//
// The wait is the loop's own flag: `refining` is true while the still picture still lacks a chunk
// (webgl2/hook.ts), false on the frame that completes it and on the frame after a camera move.
// The page reports it in `view.stats()`, so the probe needs no counter arithmetic.
//
// The frame times are sampled on the same poll as the wait, so they are the times of frames the
// poll happened to land between, not every frame: enough to rank the frames a fill is made of,
// not a histogram of them.
async (args) => {
  const view = window.__perf.view();
  if (view === null) throw new Error("the driver found no view");
  const round = (value) => Math.round(value * 100) / 100;
  const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
  const started = performance.now();
  const first = view.stats();
  const times = [];
  let full = -1;
  let stats = first;
  for (let poll = 0; poll < args.polls; poll += 1) {
    stats = view.stats();
    if (stats.frameMs > 0) times.push(stats.frameMs);
    if (stats.edges > 0 && !stats.refining && stats.drawnEdges >= stats.edges) {
      full = performance.now() - started;
      break;
    }
    await sleep(args.everyMs);
  }
  const ordered = times.slice().sort((a, b) => a - b);
  const quantile = (q) => (ordered.length === 0 ? -1 : round(ordered[Math.floor((ordered.length - 1) * q)]));
  return {
    fullMs: full < 0 ? -1 : Math.round(full),
    polls: args.polls,
    backend: stats.backend,
    backendFailure: stats.backendFailure,
    refining: stats.refining,
    nodes: stats.nodes,
    edges: stats.edges,
    drawnEdges: stats.drawnEdges,
    drawnNodes: stats.drawnNodes,
    frames: stats.frames - first.frames,
    sampledFrames: ordered.length,
    p50FrameMs: quantile(0.5),
    p95FrameMs: quantile(0.95),
    maxFrameMs: ordered.length === 0 ? -1 : round(ordered[ordered.length - 1]),
    lastFrameMs: round(stats.frameMs),
  };
}