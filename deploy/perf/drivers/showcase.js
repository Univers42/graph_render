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
    // `display: none`, not `visibility`: the fit's safe area is measured from those panels,
    // so only a removed panel gives the graph the whole frame back.
    chrome: async (on) => {
      const element = document.querySelector("graph-studio");
      let style = element.shadowRoot.getElementById("show-chrome");
      if (style === null) {
        style = document.createElement("style");
        style.id = "show-chrome";
        element.shadowRoot.appendChild(style);
      }
      style.textContent = on ? "" : ".gs-root { display: none !important; }";
      await frame();
      await frame();
      await (await settled()).dispatch("view.fit", {});
    },
    // Turn a 3D drawing about its vertical axis by `turns` over `ms`, eased, one step per frame.
    orbit: async ({ turns = 1, tilt = 0, ms = 6000 }) => {
      const view = document.querySelector("graph-studio").view;
      const start = performance.now();
      const from = view.orbit();
      if (from === null) throw new Error("showcase driver: orbit on a 2D drawing");
      let done = 0;
      while (done < 1) {
        await frame();
        done = Math.min(1, (performance.now() - start) / ms);
        const eased = done < 0.5 ? 2 * done * done : 1 - (-2 * done + 2) ** 2 / 2;
        view.setOrbit({ ...view.orbit(), yaw: from.yaw + 2 * Math.PI * turns * eased,
          pitch: from.pitch + tilt * Math.sin(Math.PI * eased) });
      }
    },
    // Where the most connected node is drawn, in page pixels, for a real pointer drag.
    hub: () => {
      const element = document.querySelector("graph-studio");
      const view = element.view;
      const state = studioOf().store.get();
      const { nodeCount, source, target } = view.frame();
      const degree = new Uint32Array(nodeCount);
      for (let edge = 0; edge < source.length; edge += 1) { degree[source[edge]] += 1; degree[target[edge]] += 1; }
      let best = 0;
      for (let node = 1; node < nodeCount; node += 1) if (degree[node] > degree[best]) best = node;
      const camera = view.camera();
      const world = view.position(best);
      const box = element.getBoundingClientRect();
      return { node: best, degree: degree[best], nodes: state.meta?.ids?.length ?? nodeCount,
        x: box.left + world.x * camera.scale + camera.x, y: box.top + world.y * camera.scale + camera.y };
    },
    catalog: () => {
      const state = studioOf().store.get();
      return { catalog: state.catalog, settings: state.settings, nodes: state.meta?.ids?.length ?? null };
    },
    log: (n) => studioOf().store.get().log.slice(-(n ?? 3)),
  };
})()
