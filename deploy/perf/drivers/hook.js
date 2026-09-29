// Driver for <graph-studio>: it dispatches the studio's own actions, the ones the dock and
// the console dispatch, so a measured run is a run a user can type.
//
// The graph is `random`, degree 3, seed 1: the baseline was recorded on that graph
// (docs/measurements/studio-perf-baseline.md), and a ratio against it means nothing on another.
(() => {
  const SOURCE = { degree: 3, seed: 1, shape: "random" };
  const studioOf = () => {
    const studio = document.querySelector("graph-studio")?.studio ?? null;
    if (studio === null) throw new Error("hook driver: no <graph-studio> with a studio on the page");
    return studio;
  };
  const settled = (state) => state.busy.length === 0 && (state.run !== null || state.error !== null);
  // The studio draws its own first graph when it starts; a command sent before that has
  // finished would be measured together with it.
  const ready = async () => {
    await customElements.whenDefined("graph-studio");
    const studio = studioOf();
    if (settled(studio.store.get())) return studio;
    await new Promise((resolve) => {
      const stop = studio.store.subscribe(() => {
        if (!settled(studio.store.get())) return;
        stop();
        resolve();
      });
    });
    return studio;
  };
  const ran = async (id, args) => {
    const entry = await (await ready()).dispatch(id, args);
    if (!entry.ok) throw new Error(`${entry.command}: ${entry.message}`);
  };
  window.__perf = {
    maxNodes: 20000,
    canvas: () => document.querySelector("graph-studio")?.shadowRoot?.querySelector("canvas") ?? null,
    layouts: () => studioOf().store.get().catalog?.layouts ?? [],
    run: (id) => ran("layout.run", { id }),
    // The layout first, on the small graph the studio opened with: the large one is then
    // laid out once, by the layout that was asked for.
    open: async (nodes, layout) => {
      await ran("layout.run", { id: layout });
      await ran("source.synthetic", { ...SOURCE, nodes });
    },
  };
})()
