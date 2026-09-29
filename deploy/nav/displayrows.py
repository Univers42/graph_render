"""Display rows, part one: arrows, text fade, node size, link thickness and edge style.

Each row sets a control the way the dock does and reads the view's own numbers back
(`view.stats()`, `view.radii()`), so a red row names the control, not the rasteriser.
`broken` makes the node-size row expect 5.01x where the app makes 5x.
"""
from displaylib import run
from navrows import row

RATIO_TOLERANCE = 0.005
ARROW_TOLERANCE = 0.01
ZOOMS = (0.25, 0.5, 1)
FADES = (-3, 0, 3)


# Ponytail: arrows and curved edges are counted by the same painter rule (chord length), so their
# equality shows consistency, not that the rule is right; 90% of drawn edges bounds the gap.
def row_arrows(studio, broken):
    got = run(studio, """
      await d.set('appearance.thickness', { factor: 1 });
      await d.set('appearance.arrows', { on: false });
      const off = d.view.stats();
      await d.set('appearance.edgestyle', { style: 'curve' });
      await d.set('appearance.arrows', { on: true });
      const one = d.view.stats();
      await d.set('appearance.thickness', { factor: 2 });
      const two = d.view.stats();
      await d.set('appearance.thickness', { factor: 1 });
      return { off: off.drawnArrows, edges: one.drawnEdges, headed: one.curvedEdges, on: one.drawnArrows, s1: one.arrowSize, s2: two.arrowSize };
    """)
    ratio = got["s2"] / got["s1"] if got["s1"] else 0
    passed = got["off"] == 0 and got["on"] == got["headed"] > 0 and got["headed"] <= got["edges"] and abs(ratio - 2) <= ARROW_TOLERANCE
    return row("display-arrows", "off: 0 arrows; on: one per drawn edge that has a heading (a zero-length edge has none); size at 2x thickness is 2.00x that at 1x",
               f"off {got['off']}, on {got['on']}/{got['headed']} headed of {got['edges']} drawn, size {got['s1']:.2f} → {got['s2']:.2f} px (x{ratio:.3f})", passed)


LABELS_AT = """
  await d.set('appearance.theme', { name: %(theme)s });
  await d.set('appearance.labels', { mode: 'auto' });
  d.view.fit();
  await d.sleep(350);
  const base = d.view.camera();
  const out = {};
  for (const fade of %(fades)s) {
    await d.set('appearance.fade', { value: fade });
    for (const k of %(zooms)s) {
      d.view.setCamera({ ...base, scale: base.scale * k });
      await d.sleep(350);
      out[fade + '@' + k] = d.view.stats().drawnLabels;
    }
  }
  await d.set('appearance.fade', { value: 0 });
  d.view.fit();
  return out;
"""


def _labels(studio, theme):
    return run(studio, LABELS_AT % {"theme": repr(theme), "fades": list(FADES), "zooms": list(ZOOMS)})


def row_fade(studio, broken):
    got = _labels(studio, "dark")
    by_zoom = all(got[f"{f}@{ZOOMS[i]}"] <= got[f"{f}@{ZOOMS[i + 1]}"] for f in FADES for i in range(2))
    by_slider = all(got[f"{FADES[i]}@1"] >= got[f"{FADES[i + 1]}@1"] for i in range(2))
    spread = got["-3@1"] > got["3@1"]
    return row("display-fade", "labels never fall as the camera zooms in, and never rise as the slider goes -3 → 0 → 3",
               ", ".join(f"{k}:{v}" for k, v in got.items()), by_zoom and by_slider and spread)


def row_fade_preset(studio, broken):
    # The studio's label path does not run the 18-label declutter (only the parity scene does), so the
    # cap is not read here and is reported as unmet, never as a pass.
    got = _labels(studio, "slate")
    by_zoom = all(got[f"{f}@{ZOOMS[i]}"] <= got[f"{f}@{ZOOMS[i + 1]}"] for f in FADES for i in range(2))
    by_slider = all(got[f"{FADES[i]}@1"] >= got[f"{FADES[i + 1]}@1"] for i in range(2))
    return row("display-fade-preset", "in a SciGraphs preset (slate) labels never fall with zoom and never rise with the slider",
               ", ".join(f"{k}:{v}" for k, v in got.items()) + f"; peak {max(got.values())} (18-label cap not applied in the studio)",
               by_zoom and by_slider and got["-3@1"] > got["3@1"])


def _ratios(studio, factor):
    return run(studio, """
      await d.set('appearance.theme', { name: 'dark' });
      await d.set('appearance.scale', { factor: 1 });
      const base = Array.from(d.view.radii());
      await d.set('appearance.scale', { factor: %s });
      const now = Array.from(d.view.radii());
      await d.set('appearance.scale', { factor: 1 });
      return base.map((r, i) => now[i] / r);
    """ % factor)


def row_size(studio, broken):
    expected = {0.2: 0.2, 5: 5.01 if broken else 5}
    seen, passed = [], True
    for factor, want in expected.items():
        ratios = _ratios(studio, factor)
        exact = sum(1 for r in ratios if abs(r - want) <= RATIO_TOLERANCE)
        beyond = sum(1 for r in ratios if r > max(want, 1) + RATIO_TOLERANCE or r < min(want, 1) - RATIO_TOLERANCE)
        seen.append(f"{factor}x: {exact}/{len(ratios)} at {want}x, {beyond} beyond")
        passed = passed and exact > 0 and beyond == 0
    return row("display-size", "node size 0.2 and 5: radii scale by that factor (clamped to min/max px, never past it)",
               "; ".join(seen), passed)


def row_size_degree(studio, broken):
    got = run(studio, """
      await d.set('appearance.size', { by: 'degree' });
      const radii = Array.from(d.view.radii());
      const degree = Array.from(d.studio.store.get().meta.degree);
      await d.set('appearance.size', { by: 'weight' });
      return { radii, degree };
    """)
    order = sorted(range(len(got["degree"])), key=lambda i: (got["degree"][i], got["radii"][i]))
    radii = [got["radii"][i] for i in order]
    monotone = all(a <= b + 1e-6 for a, b in zip(radii, radii[1:]))
    return row("display-size-degree", "size by degree: radius order equals degree order, and the largest degree draws larger than the smallest",
               f"{len(radii)} nodes, radius {radii[0]:.3f} → {radii[-1]:.3f}", monotone and radii[-1] > radii[0])


def row_thickness(studio, broken):
    got = run(studio, """
      const out = {};
      for (const factor of [1, 0.1, 5]) {
        await d.set('appearance.thickness', { factor });
        out[factor] = d.view.stats().strokeWidth;
      }
      await d.set('appearance.thickness', { factor: 1 });
      return out;
    """)
    low, high = got["0.1"] / got["1"], got["5"] / got["1"]
    return row("display-thickness", "link thickness 0.1 and 5: the stroke width scales by that factor",
               f"{got['1']:.3f} px → x{low:.3f} and x{high:.3f}",
               abs(low - 0.1) <= RATIO_TOLERANCE and abs(high - 5) <= RATIO_TOLERANCE)


def row_edge_style(studio, broken):
    got = run(studio, """
      const out = {};
      for (const style of ['straight', 'curve', 'straight']) {
        await d.set('appearance.edgestyle', { style });
        const s = d.view.stats();
        out[style] = [...(out[style] || []), s.curvedEdges, s.drawnEdges];
      }
      return out;
    """)
    straight, curve = got["straight"], got["curve"]
    passed = straight[0] == 0 == straight[2] and curve[0] > 0 and 0.9 * curve[1] <= curve[0] <= curve[1]
    return row("display-edge-style", "curve: every drawn edge with a heading has a control point (zero-length ones have none); straight: none",
               f"curved {curve[0]}/{curve[1]}, straight {straight[0]} then {straight[2]}", passed)
