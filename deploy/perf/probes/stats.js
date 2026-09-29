// The view's own counters, read from outside: strokes per frame, sprites baked on a second
// identical frame, and label layouts over a redraw, a parked frame and a zoom.
//
// Ponytail: "a redraw" is the view asked to paint again with nothing changed
// (togglePin(-1) invalidates and changes no state); a hover-driven redraw is not driven
// here, because a pointer on a node changes the focus and the layout is owed then.
async (args) => {
  const native = window.requestAnimationFrame.bind(window);
  const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
  const nextFrame = () => new Promise((resolve) => native(() => resolve()));
  const view = window.__perf.view();
  if (view === null) throw new Error("the driver found no view");

  // Until no frame has been painted for 400 ms: the label sprites bake a batch a frame.
  async function settle() {
    let seen = -1;
    while (view.stats().frames !== seen) {
      seen = view.stats().frames;
      await sleep(400);
    }
  }
  async function repaint(change) {
    const before = view.stats().frames;
    change();
    for (let i = 0; i < 20 && view.stats().frames === before; i += 1) await nextFrame();
    await sleep(200);
    return view.stats().frames - before;
  }

  await sleep(args.settleMs);
  await settle();
  const base = view.stats();
  const redrawn = await repaint(() => view.togglePin(-1));
  const second = view.stats();
  await sleep(600);
  const parked = view.stats();
  const zoomed = await repaint(() => view.zoomBy(1.5));
  const after = view.stats();
  return {
    nodes: base.nodes, edges: base.edges, drawnEdges: base.drawnEdges, drawnLabels: base.drawnLabels,
    strokeCalls: base.strokeCalls, edgeStyles: base.edgeStyles, arrowFills: base.arrowFills,
    glowFills: base.glowFills, redrawFrames: redrawn, zoomFrames: zoomed,
    spritesSecondFrame: second.spritesRasterised,
    layoutRunsRedraw: second.layoutRuns - base.layoutRuns,
    layoutRunsParked: parked.layoutRuns - second.layoutRuns,
    layoutRunsZoom: after.layoutRuns - parked.layoutRuns,
  };
}
