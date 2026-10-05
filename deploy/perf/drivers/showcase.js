// Driver for the showcase recording (deploy/perf/showcase.py): the studio's own actions, plus a
// camera glide. Every change goes through `studio.dispatch`, the path the dock and the console
// take, so each shot in the video is something a user can do by hand.
(() => {
  const studioOf = () => {
    const studio = document.querySelector("graph-studio")?.studio ?? null;
    if (studio === null) throw new Error("showcase driver: no <graph-studio> with a studio on the page");
    return studio;
  };
  const idle = (state) => state.busy.length === 0 && (state.run !== null || state.error !== null);
  const settled = async () => {
    await customElements.whenDefined("graph-studio");
    const studio = studioOf();
    if (idle(studio.store.get())) return studio;
    await new Promise((resolve) => {
      const stop = studio.store.subscribe(() => {
        if (!idle(studio.store.get())) return;
        stop();
        resolve();
      });
    });
    return studio;
  };
  const frame = () => new Promise((resolve) => requestAnimationFrame(resolve));
  window.__show = {
    run: async (id, args) => {
      const entry = await (await settled()).dispatch(id, args ?? {});
      if (!entry.ok) throw new Error(`${entry.command}: ${entry.message}`);
      await settled();
      return entry.message ?? "";
    },
    // Zoom by `zoom` and pan by (dx, dy) screen pixels over `ms`, eased, one step per frame.
    glide: async ({ zoom = 1, dx = 0, dy = 0, ms = 2000 }) => {
      const studio = await settled();
      const ease = (t) => (t < 0.5 ? 4 * t * t * t : 1 - (-2 * t + 2) ** 3 / 2);
      const start = performance.now();
      let done = 0;
      while (done < 1) {
        await frame();
        const next = Math.min(1, (performance.now() - start) / ms);
        const step = ease(next) - ease(done);
        if (zoom !== 1) await studio.dispatch("view.zoom", { factor: zoom ** step });
        if (dx !== 0 || dy !== 0) await studio.dispatch("view.pan", { dx: dx * step, dy: dy * step });
        done = next;
      }
    },
    // Hide the panels over the canvas (search, dock, legend, status) for a full-frame shot.
    chrome: (on) => {
      const shadow = document.querySelector("graph-studio").shadowRoot;
      let style = shadow.getElementById("show-chrome");
      if (style === null) {
        style = document.createElement("style");
        style.id = "show-chrome";
        shadow.appendChild(style);
      }
      style.textContent = on ? "" : ".gs-root { visibility: hidden !important; }";
    },
    catalog: () => {
      const state = studioOf().store.get();
      return { catalog: state.catalog, settings: state.settings, nodes: state.meta?.ids?.length ?? null };
    },
    log: (n) => studioOf().store.get().log.slice(-(n ?? 3)),
  };
})()
