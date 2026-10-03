// The frame time of one orbit drag over a 3D drawing: one turn per animation frame, the gaps
// between the frames that answered it, timestamped in the page.
//
// The turn is the drag's own: `rotateBy` turns the yaw 0.01 rad per pixel (three/orbit.ts) and the
// pointer path hands it to `moveOrbit`, which `view.setOrbit` reaches too. Driving it from the page
// rather than through CDP Input keeps the browser's input round trip out of every gap (settle-pan.js
// gives the same reason).
//
// Caveat: a gap counts whatever else the page did in that frame, so one garbage collection shows in
// max and p95; on the software arm the raster is a CPU rasteriser, so the numbers rank two builds
// on one host and say nothing about a GPU.
async (args) => {
  const view = window.__perf.view();
  if (view === null) throw new Error("the driver found no view");
  const start = view.orbit();
  if (start === null) throw new Error("the frame on screen is not 3D: view.orbit() is null");
  const round = (value) => Math.round(value * 100) / 100;
  const frame = () => new Promise((resolve) => requestAnimationFrame(resolve));
  for (let warm = 0; warm < args.warm; warm += 1) await frame();
  const before = view.stats();
  const stamps = [];
  const costs = [];
  let orbit = start;
  for (let step = 0; step < args.steps; step += 1) {
    orbit = { ...orbit, yaw: orbit.yaw + args.dx * 0.01 };
    view.setOrbit(orbit);
    stamps.push(await frame());
    costs.push(view.stats().frameMs);
  }
  const after = view.stats();
  const gaps = stamps.slice(1).map((time, at) => time - stamps[at]);
  const quantile = (from, q) => {
    if (from.length === 0) return -1;
    return round(from.slice().sort((a, b) => a - b)[Math.floor((from.length - 1) * q)]);
  };
  return {
    layoutYaw: round(start.yaw),
    steps: args.steps,
    painted: after.frames - before.frames,
    gaps: gaps.length,
    p50GapMs: quantile(gaps, 0.5),
    p95GapMs: quantile(gaps, 0.95),
    maxGapMs: quantile(gaps, 1),
    // The loop's own CPU time per frame (projection and paint calls), raster excluded.
    p50FrameMs: quantile(costs, 0.5),
    p95FrameMs: quantile(costs, 0.95),
    backend: after.backend,
    backendFailure: after.backendFailure,
    nodes: after.nodes,
    edges: after.edges,
    drawnNodes: after.drawnNodes,
    drawnEdges: after.drawnEdges,
  };
}
