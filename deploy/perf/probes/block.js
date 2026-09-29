// How long each layout keeps the main thread from running anything else.
//
// Ponytail: the stall is the longest gap between two beats of a 5 ms timer, so it also
// counts a stall caused by the host scheduler, and it cannot see one shorter than 5 ms.
// Over-reports under load, never under-reports a real freeze.
async (args) => {
  const BEAT_MS = 5;
  const native = window.requestAnimationFrame.bind(window);
  const nextFrame = () => new Promise((resolve) => native(() => resolve()));
  const beat = { last: performance.now(), worst: 0 };
  const timer = setInterval(() => {
    const now = performance.now();
    beat.worst = Math.max(beat.worst, now - beat.last - BEAT_MS);
    beat.last = now;
  }, BEAT_MS);

  const rows = [];
  for (const id of args.layouts ?? window.__perf.layouts()) {
    beat.last = performance.now();
    beat.worst = 0;
    const started = performance.now();
    let error = null;
    try {
      await window.__perf.run(id);
    } catch (thrown) {
      error = String(thrown);
    }
    await nextFrame();
    await nextFrame();
    rows.push({
      id, wallMs: +(performance.now() - started).toFixed(1),
      blockMs: +Math.max(0, beat.worst).toFixed(1), error,
    });
  }
  clearInterval(timer);
  return rows;
}
