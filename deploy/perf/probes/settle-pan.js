// The frame rate of one camera pan over a filled picture: the gaps between the frames the view
// paints after `panBy` moved the camera, polled and timestamped from the page so neither the wait
// nor the loop costs a CDP round trip per sample.
//
// `panBy` is the view's own camera move, the one a drag makes (view.ts); no wheel or pointer event
// is dispatched, so the gaps are the renderer's and not an input round trip.
//
// The wait is for a fill a quarter of the way in: `refining` says the still picture still lacks a
// chunk, and a quarter of the frame's edges says it is this frame's picture and not the graph the
// studio opened with, which settles first. A quarter rather than a count: the probe is asked for a
// picture of any size, and a share of the edges it has is the only midpoint that need not be told it.
//
// ponytail: gaps, not fps. A frame rate needs a start and an end that agree on what a frame is;
// the gaps do not, and a run that drew three frames in two seconds says so without being divided.
async (args) => {
  const view = window.__perf.view();
  if (view === null) throw new Error("the driver found no view");
  const round = (value) => Math.round(value * 100) / 100;
  const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
  const started = performance.now();
  let filledAtMs = -1;
  for (let poll = 0; poll < args.fillPolls; poll += 1) {
    const stats = view.stats();
    if (stats.refining === true && stats.edges > 0 && stats.drawnEdges >= stats.edges / 4) {
      filledAtMs = performance.now() - started;
      break;
    }
    await sleep(args.everyMs);
  }
  const before = view.stats();
  const pannedAt = performance.now();
  view.panBy({ x: 60, y: 40 });
  // Inside the loop's own MOVING_MS window a frame is a moving one, and a moving frame owes no
  // still chunk: `refining` must read false here even though the picture was mid-fill a moment ago.
  await sleep(30);
  const inMotion = view.stats();
  const stamps = [];
  let minDrawnAfter = inMotion.drawnEdges;
  const until = pannedAt + args.measureMs;
  // At least 20 gaps, or the measure window over, whichever is later: 21 stamps are 20 gaps.
  // The extra window is slack, so a frame that arrives late is not the run's last.
  const deadline = until + 20 * args.everyMs;
  while (performance.now() < deadline && (performance.now() < until || stamps.length < 21)) {
    stamps.push(await new Promise((resolve) => requestAnimationFrame(resolve)));
    minDrawnAfter = Math.min(minDrawnAfter, view.stats().drawnEdges);
  }
  const gaps = stamps.slice(1).map((time, at) => time - stamps[at]);
  // Two windows, because they are two different things: the gaps ending inside the loop's own
  // MOVING_MS are the pan's own frames (the camera is still moving, so the layer draws its moving
  // budget), and the gaps after it are the re-fill of the picture the pan restarted. The floor a
  // settled fill takes moves the second window and not the first, so the two are read apart.
  const panGaps = gaps.filter((_, at) => stamps[at + 1] <= pannedAt + args.movingMs);
  const refill = gaps.filter((_, at) => stamps[at + 1] > pannedAt + args.movingMs);
  const ordered = gaps.slice().sort((a, b) => a - b);
  const quantile = (from, q) => (from.length === 0 ? -1 : round(from.slice().sort((a, b) => a - b)[Math.floor((from.length - 1) * q)]));
  const after = view.stats();
  return {
    filledAtMs: filledAtMs < 0 ? -1 : round(filledAtMs),
    refiningBefore: before.refining,
    // False while the camera is still moving: the still owes no chunk, so the pan took the fill.
    refiningMoving: inMotion.refining,
    drawnMoving: inMotion.drawnEdges,
    refiningAfter: after.refining,
    drawnBefore: before.drawnEdges,
    // The picture starts again from nothing once the pan settles, so the least it ever read after the
    // pan is what says the still was dropped and not carried on.
    minDrawnAfter,
    drawnAfter: after.drawnEdges,
    edges: after.edges,
    framesAfter: after.frames - before.frames,
    gaps: gaps.length,
    p50GapMs: quantile(gaps, 0.5),
    p95GapMs: quantile(gaps, 0.95),
    maxGapMs: ordered.length === 0 ? -1 : round(ordered[ordered.length - 1]),
    // The pan's own frames: the camera is moving through these, and the still floor touches none.
    panGaps: panGaps.length,
    panP50GapMs: quantile(panGaps, 0.5),
    panMaxGapMs: quantile(panGaps, 1),
    // The re-fill the pan restarted: the windows a settled floor does move.
    refillGaps: refill.length,
    refillP50GapMs: quantile(refill, 0.5),
    refillMaxGapMs: quantile(refill, 1),
    // From `panBy` to the first frame that answers it: the latency a drag feels, and the only gap
    // this probe cannot read as a frame interval because nothing has been painted yet.
    firstFrameMs: stamps.length === 0 ? -1 : round(stamps[0] - pannedAt),
    lastFrameMs: round(after.frameMs),
    backend: after.backend,
    backendFailure: after.backendFailure,
    nodes: after.nodes,
  };
}