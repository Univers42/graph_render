/**
 * Visual-parity check: does the extracted standalone engine render the same as the
 * host's in-tree engine?
 *
 * Verification rig — not part of the published package.
 *
 * The page (verify/parity) mounts the host engine twice and the standalone engine
 * once in a single document, all with the same model and the host's real token
 * values. The host-vs-host comparison is the control that establishes the noise
 * floor; the host-vs-standalone comparison is the signal. This script serves the
 * page, waits for the result, and reports it together with console errors, page
 * errors and failed requests — so the verdict is never taken on faith.
 *
 * Run inside the rig image (which has Chromium), not on the VM host: the VM has no
 * browser and cannot install one.
 */

import { chromium } from "@playwright/test";
import { readFileSync } from "node:fs";

/**
 * Where the rig is served. Derived from PARITY_PORT so the check and the dev
 * server cannot disagree: they used to be two independent literals (5555 in this
 * file, 5555 in the vite config), which meant moving the rig to a firewall-allowed
 * port would have left the check polling a dead port and reporting INCONCLUSIVE
 * for the wrong reason.
 */
const PORT = Number(process.env.PARITY_PORT ?? 4322);
const BASE = process.env.PARITY_URL ?? `http://127.0.0.1:${PORT}/`;
const SHOT = process.env.PARITY_SHOT ?? "/work/verify/parity.png";
const PAGE_W = Number(process.env.PARITY_W ?? 1820);
const PAGE_H = Number(process.env.PARITY_H ?? 660);

const browser = await chromium.launch();
// `prefers-reduced-motion: reduce` is not cosmetic here. The scene staggers its
// reveal animation off performance.now() (core/render/scene.ts:100), so two panels
// built microseconds apart fade their nodes in at different phases and their
// pixels differ EVEN WHEN THE SOURCE IS IDENTICAL — which is what the control
// pair demonstrates. Reduced motion makes the reveal instant, which removes the
// only time-dependent term and lets a pixel comparison mean something.
const page = await browser.newPage({
  viewport: { width: PAGE_W, height: PAGE_H },
  reducedMotion: "reduce",
});

/**
 * Freeze the clock.
 *
 * Two independent time-dependent effects were making identical code render
 * differently, and both had to go before a pixel comparison meant anything:
 *
 *  1. The reveal stagger — `startReveal(..., performance.now())` in
 *     core/render/scene.ts. Handled by `reducedMotion: "reduce"` above.
 *  2. The edge-flow animation — dashes advance along each edge as a function of
 *     elapsed time (core/render/edgeFlow.ts). This one reduced motion does not
 *     cover, and it was the dominant residual: the differing pixels formed the
 *     same diffuse band along the edges in the control and the signal pair, with
 *     the same bounding box and near-identical occupancy.
 *
 * Pinning performance.now() to a constant makes every time-derived phase
 * identical in every panel. It also collapses dt to zero, so nothing integrates
 * forward and the frame is a pure function of (model, layout state, camera,
 * resolved theme) — which is exactly the claim being tested. The page itself is
 * unmodified; the clock is pinned by the driver, which is standard practice for
 * this class of comparison.
 */
/**
 * Replace the clock with one that ADVANCES but is SHARED and QUANTIZED.
 *
 * Getting this right took two attempts, and the failure mode of getting it wrong
 * is worth recording because it produces a convincing false pass:
 *
 *  1. A CONSTANT clock (performance.now = () => FIXED) makes every time-derived
 *     value identical across panels — the edge-flow dashes stop differing, which
 *     was the goal. But it also freezes the reveal stagger at progress 0, so
 *     every node renders fully transparent. The graph layer then compares as
 *     "pixel-identical" while being empty, and the screenshot shows an aurora
 *     field with no graph at all. The first run of this check reported
 *     PIXEL-IDENTICAL across all 16 palettes on an empty canvas.
 *  2. A real wall clock leaves the flow animation at a different phase in each
 *     panel, because the panels are constructed microseconds apart.
 *
 * So: a clock that increments once per animation FRAME and is read identically
 * by every panel within that frame. Panels share the rAF clock, so at the moment
 * of comparison all three read the same value; the reveal still advances to
 * completion, and `settle()` below waits for that to happen. `fgInkPixels` in the
 * result is the guard that would have caught attempt 1.
 */
await page.addInitScript(() => {
  const START = 1_700_000_000_000;
  const STEP = 1000 / 60;
  let frame = 0;
  const tick = () => {
    frame += 1;
    requestAnimationFrame(tick);
  };
  requestAnimationFrame(tick);
  performance.now = () => START + frame * STEP;
});

/**
 * Inject the host's token stylesheet, plus the package's own graph.css, straight
 * into the page.
 *
 * Read from disk here and handed to the page as a string, rather than fetched over
 * HTTP. That is deliberate: serving the tokens over HTTP failed twice in a row and
 * both failures were invisible (see the long note in verify/parity/index.html).
 * With the text handed over directly there is no status code, no content-type and
 * no fallback index.html to get between the tokens and the engine.
 *
 * graph.css is injected for the SCREENSHOT, not for the measurement. The
 * comparison reads canvas backing stores through getImageData, which no stylesheet
 * can influence, so the numbers are unaffected either way. Without it, though,
 * `.osio-graph__bg` and `.osio-graph__fg` have no z-index and stack in normal
 * flow, so the graph canvas is pushed below the visible area of the cell and every
 * screenshot shows an empty aurora field. An artifact that misrepresents the very
 * thing it is meant to document is worse than no artifact.
 */
const TOKENS_CSS_PATH = process.env.PARITY_TOKENS ?? "/work/verify/parity/tokens.host.css";
const GRAPH_CSS_PATH = process.env.PARITY_GRAPH_CSS ?? "/work/src/styles/graph.css";
const tokensCss = readFileSync(TOKENS_CSS_PATH, "utf8");
const graphCss = readFileSync(GRAPH_CSS_PATH, "utf8");
await page.addInitScript((css) => {
  window.__TOKENS_CSS__ = css;
}, `${tokensCss}\n${graphCss}`);

/**
 * The host defines 7 named palettes plus a default, each in a light and a dark
 * variant. The engine resolves its colours from CSS custom properties under
 * [data-palette][data-theme], so "renders identically" has to hold for every one
 * of those combinations — a package that matched only the default would still
 * look wrong the moment a host changed palette. Each combination is a cold load,
 * which is how a host actually reaches the engine.
 *
 * `warm` is the default and has no block of its own in global.css (it falls
 * through to :root), so it is included as the baseline case.
 */
const THEMES = ["light", "dark"];
const PALETTES = ["warm", "mono", "contrast", "maximal", "nord", "solarized", "midnight", "rose"];

const consoleErrors = [];
const pageErrors = [];
const failedRequests = [];
page.on("console", (m) => {
  if (m.type() === "error") consoleErrors.push(m.text());
});
page.on("pageerror", (e) => pageErrors.push(`${e.name}: ${e.message}`));
page.on("requestfailed", (r) =>
  failedRequests.push(`${r.url()} ${r.failure()?.errorText ?? ""}`.trim()),
);

/** Run one theme/palette combination and return its two comparisons. */
async function runCombo(theme, palette) {
  await page.goto(`${BASE}?theme=${theme}&palette=${palette}`, { waitUntil: "load" });
  await page.waitForFunction(() => window.__parityReady === true, null, { timeout: 60_000 });
  const got = await page.evaluate(() => ({
    parity: window.__parity ?? null,
    error: window.__parityError ?? null,
  }));
  return got;
}

const combos = [];
for (const theme of THEMES) {
  for (const palette of PALETTES) {
    const got = await runCombo(theme, palette);
    const p = got.parity;
    const clean = got.error == null && p !== null;
    combos.push({
      theme,
      palette,
      clean,
      error: got.error ?? null,
      geometryIdentical: clean ? p.signalGeo.maxAbsPosDelta === 0 : null,
      cameraEqual: clean ? p.signalGeo.cameraEqual : null,
      controlFg: clean ? p.control.fg.differing : null,
      controlBg: clean ? p.control.bg.differing : null,
      signalFg: clean ? p.signal.fg.differing : null,
      signalBg: clean ? p.signal.bg.differing : null,
      signalMaxDelta: clean ? p.signal.maxDelta : null,
      nodes: clean ? p.nodeCount : null,
      edges: clean ? p.edgeCount : null,
      inkPixels: clean ? p.ink.host.inkPixels : null,
      sampledPixels: clean ? p.ink.host.total : null,
      distinctColours: clean ? p.ink.host.distinctColours : null,
      // The host panel's own fingerprint. Recorded so the sweep can prove the
      // theme/palette parameter actually changed the render: if all 16 combos
      // fingerprinted identically, the sweep would be comparing one image to
      // itself 16 times and "pixel-identical" would be an artefact of the
      // parameter doing nothing.
      hostFgChecksum: clean ? p.control.fg.checksumA : null,
      hostBgChecksum: clean ? p.control.bg.checksumA : null,
      notes: clean ? p.notes : null,
      tokensResolved: clean ? p.tokensResolved : null,
    });
  }
}

// One representative screenshot for the record.
await page.goto(`${BASE}?theme=dark&palette=warm`, { waitUntil: "load" });
await page.waitForFunction(() => window.__parityReady === true, null, { timeout: 60_000 });
await page.screenshot({ path: SHOT });

const canvasCount = await page.evaluate(() => ({
  total: document.querySelectorAll("canvas").length,
  fg: document.querySelectorAll(".osio-graph__fg").length,
  bg: document.querySelectorAll(".osio-graph__bg").length,
}));

/**
 * Verdict, stated as a rule rather than eyeballed:
 *   - PRESENCE: every combination must have actually drawn the graph. A parity
 *     result over a blank layer is worthless, and this check has already produced
 *     one (a constant clock froze the reveal at zero, and 16 empty palettes
 *     compared "identical"). A combination must have a meaningful share of
 *     inked pixels and more than a handful of distinct colours.
 *   - CLEAN: no page errors, no failed requests, no console errors, 6 canvases.
 *   - DETERMINISM: the control (two instances of the SAME source) must be
 *     pixel-identical in every combination. This is what proves the rig is
 *     measuring rather than merely insensitive.
 *   - PARITY: the signal (host vs extraction) must be pixel-identical too, with
 *     identical geometry and camera.
 *
 * Exit 0 only when all hold. Perturbing one hex in the standalone copy is known
 * to move this to DIVERGENT, so a pass is a measurement and not a tautology.
 */
const MIN_INK_FRACTION = 0.02;
const MIN_COLOURS = 8;

// Every graph token the engine reads must actually resolve. Without this the rig
// will happily report a pass while the engine is rendering its hardcoded
// fallbacks, because two identical fallback-coloured renders are identical.
const tokensResolved = combos.every((c) => c.tokensResolved === true);

const present = combos.every(
  (c) =>
    c.inkPixels !== null &&
    c.sampledPixels > 0 &&
    c.inkPixels / c.sampledPixels > MIN_INK_FRACTION &&
    c.distinctColours >= MIN_COLOURS,
);
const allClean = combos.every((c) => c.clean);
const allIdentical = combos.every(
  (c) => c.signalFg === 0 && c.signalBg === 0 && c.controlFg === 0 && c.controlBg === 0,
);
const allGeometry = combos.every((c) => c.geometryIdentical === true && c.cameraEqual === true);

// The sweep must be discriminating: the host's own render has to actually change
// between combinations, otherwise the 16 comparisons are one comparison repeated.
const distinctFgFingerprints = new Set(combos.map((c) => c.hostFgChecksum)).size;
const distinctBgFingerprints = new Set(combos.map((c) => c.hostBgChecksum)).size;
const sweepDiscriminates = distinctBgFingerprints > 1;
const structural =
  canvasCount.total === 6 && canvasCount.fg === 3 && canvasCount.bg === 3 &&
  pageErrors.length === 0 && failedRequests.length === 0 && consoleErrors.length === 0;

const verdict = !allClean || !structural
  ? "INCONCLUSIVE"
  : !present
    ? "BLANK-GRAPH"
    : !sweepDiscriminates
      ? "SWEEP-NON-DISCRIMINATING"
    : allIdentical && allGeometry
      ? "PIXEL-IDENTICAL"
      : "DIVERGENT";

console.log(JSON.stringify({ verdict, combosCompared: combos.length, tokensResolved, distinctFgFingerprints, distinctBgFingerprints, minInkFraction: MIN_INK_FRACTION, minColours: MIN_COLOURS, combos, canvasCount, pageErrors, consoleErrors, failedRequests, screenshot: SHOT }, null, 2));

await browser.close();
process.exit(verdict === "PIXEL-IDENTICAL" ? 0 : verdict === "DIVERGENT" ? 2 : 1);
