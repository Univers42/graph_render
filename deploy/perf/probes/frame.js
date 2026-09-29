// Frames per second and JS time per frame while zooming, one wheel event per frame.
//
// Ponytail: fps is frames over wall time, so it is capped by the 60 Hz frame clock and it
// drops with host load. `profile` wraps every 2D context call in two clock reads, which
// inflates the JS time: a profiled pass is for the breakdown, never for a budget row.
(() => {
  const METHODS = ["stroke", "fill", "drawImage", "strokeText", "fillText", "arc", "clearRect"];
  const native = window.requestAnimationFrame.bind(window);
  const nextFrame = () => new Promise((resolve) => native(() => resolve()));
  const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

  // Every animation callback, timed from outside. Returns the list it fills.
  function watchFrames() {
    const frames = [];
    window.requestAnimationFrame = (callback) => native((time) => {
      const started = performance.now();
      callback(time);
      frames.push(performance.now() - started);
    });
    return frames;
  }

  function profileContext(cost) {
    const proto = CanvasRenderingContext2D.prototype;
    const originals = {};
    for (const name of METHODS) {
      originals[name] = proto[name];
      cost[name] = { calls: 0, ms: 0 };
      proto[name] = function profiled(...rest) {
        const started = performance.now();
        const out = originals[name].apply(this, rest);
        cost[name].calls += 1;
        cost[name].ms += performance.now() - started;
        return out;
      };
    }
    return () => {
      for (const name of METHODS) proto[name] = originals[name];
    };
  }

  function summarise(label, steps, wallMs, probe) {
    const sorted = [...probe.frames].sort((a, b) => a - b);
    const p95 = sorted[Math.min(sorted.length - 1, Math.floor(sorted.length * 0.95))] ?? 0;
    const byMethod = {};
    for (const [name, slot] of Object.entries(probe.cost)) {
      if (slot.calls > 0) byMethod[name] = { calls: slot.calls, ms: +slot.ms.toFixed(2) };
    }
    return {
      label, steps, fps: +(1000 * steps / wallMs).toFixed(1),
      jsMeanMs: +(sorted.reduce((a, b) => a + b, 0) / Math.max(1, sorted.length)).toFixed(3),
      jsP95Ms: +p95.toFixed(3), jsMaxMs: +(sorted[sorted.length - 1] ?? 0).toFixed(3), byMethod,
    };
  }

  async function runPhase(probe, label, steps, deltaY) {
    probe.frames.length = 0;
    for (const slot of Object.values(probe.cost)) Object.assign(slot, { calls: 0, ms: 0 });
    const started = performance.now();
    for (let i = 0; i < steps; i += 1) {
      probe.wheel(deltaY);
      await nextFrame();
    }
    return summarise(label, steps, performance.now() - started, probe);
  }

  return async (args) => {
    const canvas = window.__perf.canvas();
    if (canvas === null) throw new Error("the driver found no graph canvas");
    const box = canvas.getBoundingClientRect();
    const probe = { frames: watchFrames(), cost: {}, wheel: null };
    probe.wheel = (deltaY) => canvas.dispatchEvent(new WheelEvent("wheel", {
      deltaY, clientX: box.left + box.width / 2, clientY: box.top + box.height / 2,
      bubbles: true, cancelable: true,
    }));
    const restore = args.profile ? profileContext(probe.cost) : () => {};
    await sleep(args.settleMs);
    const phases = [
      await runPhase(probe, "zoom-in", 40, -40),
      await runPhase(probe, "zoom-out", 80, 40),
      await runPhase(probe, "zoom-back", 40, -40),
    ];
    restore();
    window.requestAnimationFrame = native;
    return { dpr: devicePixelRatio, canvas: [canvas.width, canvas.height], phases };
  };
})()
