// How long the GPU layer takes to fill its settled picture with every edge, polled from the
// page so the wait costs no CDP round trip per sample.
//
// The page has no "the still is full" flag (the loop's `refining` is internal), so the probe
// reads the counters a settled frame keeps: `drawnEdges` is the pairs the picture holds and
// `edges` the pairs the frame has, so the picture is full when they are equal. A frame that
// has not drawn yet reports drawnEdges 0 against the frame's edges, which is also "not full".
//
// ponytail: polling, not a flag. `drawnEdges >= edges` is one frame late at worst (the frame
// that completes the picture is the one that reports it), which is under the poll interval.
// Change view-stats.ts to expose `refining` if a sub-frame reading is ever wanted.
async (args) => {
  const view = window.__perf.view();
  if (view === null) throw new Error("the driver found no view");
  const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
  const started = performance.now();
  const first = view.stats();
  let full = -1;
  let stats = first;
  for (let poll = 0; poll < args.polls; poll += 1) {
    stats = view.stats();
    if (stats.edges > 0 && stats.drawnEdges >= stats.edges) {
      full = performance.now() - started;
      break;
    }
    await sleep(args.everyMs);
  }
  return {
    waitedMs: Math.round(performance.now() - started),
    fullMs: full < 0 ? -1 : Math.round(full),
    polls: args.polls,
    backend: stats.backend,
    backendFailure: stats.backendFailure,
    nodes: stats.nodes,
    edges: stats.edges,
    drawnEdges: stats.drawnEdges,
    drawnNodes: stats.drawnNodes,
    frames: stats.frames - first.frames,
    lastFrameMs: Math.round(stats.frameMs * 1000) / 1000,
    fps: Math.round(stats.fps * 10) / 10,
  };
}