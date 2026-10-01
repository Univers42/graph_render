"""The page side of the edge gradient row: find one mixed edge to read, then read it.

Both scripts are one evaluate each. The first walks the frame for an edge whose ends wear
two colours, long enough on screen and clear of every node and every other link at three
points; the second reads a small patch of canvas pixels around each of them, because a
stroke is at most 1.5 px wide and the pixel it colours is within a pixel of the line.
"""
ALONG = [0.08, 0.5, 0.92]
MIN_CHORD = 150
CLEAR = 4
MARGIN = 40


FIND = """
(() => {
  const host = document.querySelector('graph-studio');
  const view = host.view, studio = host.studio;
  const canvas = host.shadowRoot.querySelector('canvas');
  const ctx = canvas.getContext('2d');
  const W = canvas.width, H = canvas.height;
  const dpr = W / canvas.getBoundingClientRect().width;
  const frame = view.frame(), style = view.style(), cam = view.camera();
  const at = (x, y) => Array.from(ctx.getImageData(Math.round(x), Math.round(y), 1, 1).data.slice(0, 3));
  const rgb = (css) => {
    const found = /^#([0-9a-f]{6})$/i.exec(css || '');
    return found === null ? null : [0, 2, 4].map((at) => parseInt(found[1].slice(at, at + 2), 16));
  };
  const on = (edge) => [0, 1].map((end) => {
    const at = view.position(frame[end === 0 ? 'source' : 'target'][edge] ?? 0);
    return [(at.x * cam.scale + cam.x) * dpr, (at.y * cam.scale + cam.y) * dpr];
  });
  const apart = (a, b, p) => {
    const dx = b[0] - a[0], dy = b[1] - a[1], len = dx * dx + dy * dy;
    const t = len === 0 ? 0 : Math.max(0, Math.min(1, ((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len));
    return Math.hypot(p[0] - (a[0] + t * dx), p[1] - (a[1] + t * dy));
  };
  const want = %s;
  const clear = (edge, p) => {
    if (view.pick({ x: p[0] / dpr, y: p[1] / dpr }) >= 0) return false;
    for (let o = 0; o < frame.edgeCount; o += 1) {
      if (o === edge) continue;
      const q = on(o);
      if (apart(q[0], q[1], p) < %d) return false;
    }
    return true;
  };
  const candidates = [];
  for (let edge = 0; edge < frame.edgeCount; edge += 1) {
    const s = style.colours[frame.source[edge]] ?? 0, t = style.colours[frame.target[edge]] ?? 0;
    if (s === t) continue;
    const frm = rgb(style.palette[s]), to = rgb(style.palette[t]);
    if (frm === null || to === null) continue;
    const [a, b] = on(edge);
    const inside = [a, b].every((p) => p[0] > %d && p[0] < W - %d && p[1] > %d && p[1] < H - %d);
    if (!inside) continue;
    const chord = Math.hypot(b[0] - a[0], b[1] - a[1]);
    if (chord < %d) continue;
    candidates.push({ edge, a, b, chord, from: frm, to });
  }
  // Longest first, edge index for a tie: the first candidate with three clear points wins.
  candidates.sort((p, q) => q.chord - p.chord || p.edge - q.edge);
  const seen = [];
  let best = null, mixed = 0;
  for (const c of candidates) {
    const clearAt = [];
    for (let k = 2; k <= 186; k += 1) {
      const f = k / 200;
      const p = [c.a[0] + (c.b[0] - c.a[0]) * f, c.a[1] + (c.b[1] - c.a[1]) * f];
      if (clear(c.edge, p)) clearAt.push([f, p]);
    }
    if (clearAt.length < want.length) continue;
    // One point per region, each the clear one nearest where the row wants to read.
    const picks = [];
    for (const aim of want) {
      let near = null;
      for (const pair of clearAt) {
        if (picks.some((taken) => Math.abs(taken[0] - pair[0]) < 0.05)) continue;
        if (near === null || Math.abs(pair[0] - aim) < Math.abs(near[0] - aim)) near = pair;
      }
      if (near === null) { picks.length = 0; break; }
      picks.push(near);
    }
    if (picks.length === 0) continue;
    best = { ...c, along: picks.map((pair) => pair[0]), points: picks.map((pair) => pair[1]) };
    break;
  }
  return { mode: studio.store.get().settings.appearance.edgeColour, style: style.edgeColour,
           stats: view.stats(), palette: style.palette.length,
           edges: frame.edgeCount, mixed: candidates.length, chosen: best };
})()
"""


READ = """
((chosen) => {
  const canvas = document.querySelector('graph-studio').shadowRoot.querySelector('canvas');
  const ctx = canvas.getContext('2d');
  const W = canvas.width, H = canvas.height;
  const at = (x, y) => Array.from(ctx.getImageData(Math.round(x), Math.round(y), 1, 1).data.slice(0, 3));
  // The ground the stroke is laid over: the colour most of the four corners carry.
  const corners = [at(2, 2), at(W - 3, 2), at(2, H - 3), at(W - 3, H - 3)];
  const tally = new Map();
  for (const c of corners) tally.set(c.join(','), (tally.get(c.join(',')) ?? 0) + 1);
  let bg = corners[0], most = 0;
  for (const [key, n] of tally) if (n > most) { most = n; bg = key.split(',').map(Number); }
  // A stroke is at most 1.5 px wide, so the pixel it colours is within a pixel or two of the
  // point on the line: the patch around each sample point is read whole, and the row looks
  // for the pixel of it that carries the ramp's colour.
  const R = 2;
  const patches = chosen.points.map((p) => {
    const patch = [];
    for (let dy = -R; dy <= R; dy += 1) {
      const row = [];
      for (let dx = -R; dx <= R; dx += 1) row.push(at(p[0] + dx, p[1] + dy));
      patch.push(row);
    }
    return patch;
  });
  return { bg, patches };
})
"""


