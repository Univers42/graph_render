// One layout switch, from the click to the settled frame: where the time went and what it cost.
//
// Times five points off the user-timing marks the studio and the render side leave
// (`gm:transition:request`, `:bytes`, `:moved`, `:settled`) and the frame gaps the tween window
// held. Counts the GPU uploads by wrapping `bufferData`/`bufferSubData` for the window, so "one
// upload a frame" is a number read off the driver rather than a claim about the source.
//
// Ponytail: the frame gaps are the gaps between rAF callbacks, so they count the whole main
// thread — the snapshot's own decode, a garbage collection and anything else the page does are in
// them, and none of it is attributed. Failing input: a tween that never settles (the window caps
// at `settleMs`) reports `settled: null` and every point after it as null rather than a guess.
// Direction: counts uploads and bytes; it does not time them, because a `bufferSubData` that
// returns has already been charged by the driver against the next draw. Escape hatch: the marks
// are readable in the console with `performance.getEntriesByName('gm:transition:moved')`.
async (args) => {
  const native = window.requestAnimationFrame.bind(window);
  const view = window.__perf.view;
  const MARKS = { request: "gm:transition:request", bytes: "gm:transition:bytes", moved: "gm:transition:moved", settled: "gm:transition:settled" };
  const byName = (name) => {
    const found = performance.getEntriesByName(name);
    return found.length === 0 ? null : found[0].startTime;
  };
  const sorted = (values) => [...values].sort((a, b) => a - b);
  const quantile = (values, at) => values.length === 0 ? null : +values[Math.min(values.length - 1, Math.floor(at * values.length))].toFixed(2);

  // What the layer hands the driver, while the window is armed.
  const tally = { calls: 0, bytes: 0, armed: false };
  for (const name of ["bufferData", "bufferSubData"]) {
    const original = WebGL2RenderingContext.prototype[name];
    WebGL2RenderingContext.prototype[name] = function (...given) {
      if (tally.armed) {
        tally.calls += 1;
        tally.bytes += given[given.length - 1]?.byteLength ?? 0;
      }
      return original.apply(this, given);
    };
  }

  const frames = [];
  let last = performance.now();
  const beat = (now) => {
    frames.push({ at: now, gap: now - last, paint: tally.armed ? view().stats().frameMs : 0 });
    last = now;
    native(beat);
  };
  native(beat);

  // Two different layouts of the same graph: a switch that changes the node count is not a
  // transition at all, `showFrame` snaps those.
  const catalog = window.__perf.layouts();
  const from = args.from ?? catalog[0];
  const to = args.to ?? catalog[1];
  const rows = [];
  for (let round = 0; round < (args.rounds ?? 3); round += 1) {
    for (const mark of Object.values(MARKS)) performance.clearMarks(mark);
    const before = view().stats();
    frames.length = 0;
    tally.calls = 0;
    tally.bytes = 0;
    tally.armed = true;
    const clicked = performance.now();
    await window.__perf.run(to);
    // The tween starts on a later frame than the dispatch resolves on, so the marks and the
    // counters are read once it has had the chance to lay down the last one.
    const deadline = clicked + (args.settleMs ?? 3000);
    while (byName("gm:transition:settled") === null && performance.now() < deadline) {
      await new Promise((resolve) => native(() => resolve()));
    }
    const at = {};
    for (const [name, mark] of Object.entries(MARKS)) at[name] = byName(mark);
    const after = view().stats();
    tally.armed = false;
    await new Promise((resolve) => native(() => resolve()));
    const inWindow = frames.filter((f) => at.moved !== null && f.at >= at.moved && at.settled !== null && f.at <= at.settled);
    const span = (from) => (at[from] === null ? null : +(at[from] - clicked).toFixed(1));
    rows.push({
      round, nodes: after.nodes, edges: after.edges, backend: after.backend,
      clickMs: span("request"), workerMs: at.request === null || at.bytes === null ? null : +(at.bytes - at.request).toFixed(1),
      // A tween the 2D painter snapped over its node budget has no moving frame at all.
      snapped: at.settled !== null && at.moved === null,
      firstFrameMs: at.bytes === null || at.moved === null ? null : +(at.moved - at.bytes).toFixed(1),
      tweenMs: at.moved === null || at.settled === null ? null : +(at.settled - at.moved).toFixed(1),
      totalMs: span("settled"),
      frames: inWindow.length,
      plansPerFrame: +((after.layoutRuns - before.layoutRuns) / Math.max(1, inWindow.length)).toFixed(2),
      uploads: tally.calls, uploadKb: +(tally.bytes / 1024).toFixed(0),
      gapP50: quantile(sorted(inWindow.map((f) => f.gap)), 0.5), gapP95: quantile(sorted(inWindow.map((f) => f.gap)), 0.95),
      paintP50: quantile(sorted(inWindow.map((f) => f.paint)), 0.5), paintP95: quantile(sorted(inWindow.map((f) => f.paint)), 0.95),
    });
    // The way back has to settle before the next round clears the marks, or the earliest entry of
    // each name is this round's predecessor and every delta reads short.
    await window.__perf.run(from);
    const back = performance.now() + (args.settleMs ?? 3000);
    while (byName("gm:transition:settled") === null && performance.now() < back) {
      await new Promise((resolve) => native(() => resolve()));
    }
    await new Promise((resolve) => setTimeout(resolve, args.betweenMs ?? 250));
  }
  return { from, to, rows };
}