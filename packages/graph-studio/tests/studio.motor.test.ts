// The studio end to end over the real wasm module and a recording view: what is drawn,
// what is refused, and that a recipe replays. Gate rows `actions-parity` (unknown action
// refused) and `recipe-replay` (changed layout, different digest).
import assert from "node:assert/strict";
import { test } from "node:test";

import { FIT_MAX_SCALE, worldToScreen } from "../../graph-render/src/camera.ts";
import { LIGHT_THEME } from "../../graph-render/src/theme.ts";
import { GROUP_PALETTE } from "../src/look/palette.ts";
import { sha256Hex } from "../src/motor/session.ts";
import { DEFAULT_SETTINGS } from "../src/state/settings.ts";
import { DESK_VIEWPORT, type Desk, desk, scriptedClient } from "./desk.ts";
import { SKIP, realClient } from "./motor.ts";

async function started(): Promise<Desk> {
  const made = desk(realClient());
  const entry = await made.studio.start();
  assert.equal(entry.ok, true, entry.message);
  return made;
}

/** The same desk over a scripted motor, so the count is checked even without the wasm. */
async function scripted(): Promise<Desk> {
  const made = desk(scriptedClient());
  const entry = await made.studio.start();
  assert.equal(entry.ok, true, entry.message);
  return made;
}

test("the opening graph is one layout call, and its filter is remembered", async () => {
  const { studio } = await scripted();
  assert.equal(studio.store.get().layoutCalls, 1);
  assert.equal(studio.store.get().runFilter, JSON.stringify(studio.store.get().settings.filter));
});

test("a look and a filter restyle without asking the motor again", async () => {
  await restyleOnly(await scripted());
});

test("a relayout filter runs the layout once, and never twice for the same filter", async () => {
  await relayoutOnce(await scripted());
});

test("fitting the results frames what the search highlighted, and needs no wasm", async () => {
  const made = await scripted();
  const empty = made.pipeline.fitResults();
  assert.match(empty.message, /fitted to 3 of 3 nodes/, "no search text: the whole drawing");
  const whole = made.seen.cameras[0];
  assert.ok((whole?.scale ?? 0) > 0);
  // One result: the camera goes in, and the studio says what it framed.
  const settings = made.studio.store.get().settings;
  made.studio.store.update((state) => ({ ...state, settings: { ...settings, filter: { ...settings.filter, text: "alpha" } } }));
  const one = made.pipeline.fitResults();
  assert.match(one.message, /fitted to 1 of 3 nodes/);
  const shown = made.seen.cameras.at(-1);
  assert.ok(shown !== undefined, "a camera was set for the one result");
  // "Frames what the search highlighted" means the matched node is under the middle of the
  // canvas: Alpha sits at the origin, so that is the offset the camera has to hold.
  const frame = made.seen.frames.at(-1)?.frame;
  const alpha = { x: frame?.x[0] ?? NaN, y: frame?.y[0] ?? NaN };
  assert.deepEqual(worldToScreen(shown, alpha), { x: DESK_VIEWPORT.width / 2, y: DESK_VIEWPORT.height / 2 });
  assert.notEqual(shown.x, whole?.x, "not the whole drawing's centre, which is a node to the right");
  // Three nodes at the renderer's own ceiling, never the view's 40: at 40× this desk reads as
  // a broken view, which is what ST-10 was.
  assert.ok(shown.scale <= FIT_MAX_SCALE, `never past ${FIT_MAX_SCALE}x, got ${shown.scale}x`);
});

test("starting draws the opening graph with its own names and sizes", { skip: SKIP }, async () => {
  const { studio, seen } = await started();
  const state = studio.store.get();
  assert.deepEqual([state.graph?.nodeCount, state.graph?.edgeCount], [400, 798]);
  assert.deepEqual(seen.frames.map((shown) => [shown.frame.nodeCount, shown.animate]), [[400, false]]);
  const style = seen.styles.at(-1);
  assert.ok(style !== undefined);
  assert.equal(style.labels.filter((label) => /^node\b/i.test(label)).length, 0);
  assert.equal(new Set(style.labels).size, 400);
  assert.ok(Math.max(...style.radius) > 2 * Math.min(...style.radius));
  assert.deepEqual(state.settings, DEFAULT_SETTINGS);
  assert.deepEqual(state.busy, []);
  assert.ok(state.catalog?.layouts.includes("layout.forceatlas2"));
});

test("every action is logged with its command, its time and the digest of what it drew", { skip: SKIP }, async () => {
  const { studio } = await started();
  const [entry] = studio.store.get().log;
  assert.equal(entry?.command, "synthetic 400 2 1 vault");
  assert.equal(entry.digest, studio.store.get().run?.digest);
  assert.match(entry.digest ?? "", /^[0-9a-f]{64}$/);
  // The id the line names is the one the opening asked for, read off the default: name the
  // string, not today's value of it, and the row keeps meaning what it meant before.
  const asked = DEFAULT_SETTINGS.layout.replaceAll(".", String.raw`\.`);
  assert.match(entry.message, new RegExp(String.raw`400 nodes, 798 links · ${asked} \d+ ms`));
  assert.ok(entry.ms > 0);
});

test("another layout keeps the graph, moves the nodes and changes the digest", { skip: SKIP }, async () => {
  const { studio, seen } = await started();
  const before = studio.store.get().run?.digest;
  const entry = await studio.dispatch("layout", { id: "grid" });
  assert.equal(entry.command, "layout layout.grid");
  assert.deepEqual(seen.frames.map((shown) => shown.animate), [false, true]);
  assert.equal(studio.store.get().settings.layout, "layout.grid");
  assert.notEqual(entry.digest, before);
  assert.equal(studio.store.get().graph?.nodeCount, 400);
});

test("an unknown command, parameter or value is refused and nothing changes", { skip: SKIP }, async () => {
  const { studio, seen } = await started();
  const before = studio.store.get();
  const unknown = await studio.dispatch("layuot", { id: "grid" });
  assert.deepEqual([unknown.ok, unknown.error?.title], [false, "ActionRefusal"]);
  assert.match(unknown.message, /`layuot` is not a command; nearest: layout/);
  assert.equal((await studio.dispatch("layout", { name: "grid" })).ok, false);
  const value = await studio.dispatch("layout", { id: "layout.nowhere" });
  assert.match(value.message, /`id` is not one of: layout\./);
  assert.equal(studio.store.get().settings, before.settings);
  assert.equal(studio.store.get().run, before.run);
  assert.equal(seen.frames.length, 1);
  assert.equal(studio.store.get().error?.title, "ActionRefusal");
});

test("a line typed in the console runs what a dispatch runs", { skip: SKIP }, async () => {
  const { studio } = await started();
  assert.equal((await studio.run("layout pivot")).command, "layout layout.mds.pivot");
  assert.equal(studio.store.get().settings.layout, "layout.mds.pivot");
  const open = await studio.run('filter "unclosed');
  assert.deepEqual([open.ok, open.command, open.error?.title], [false, 'filter "unclosed', "CommandRefusal"]);
  assert.equal((await studio.run("help layout")).ok, true);
});

test("an analysis colours the nodes and says what it measured; off gives the groups back", { skip: SKIP }, async () => {
  const { studio, seen } = await started();
  const entry = await studio.dispatch("analysis", { id: "louvain" });
  const state = studio.store.get();
  assert.equal(entry.ok, true, entry.message);
  assert.equal(state.analysis?.id, "analysis.communities.louvain");
  assert.deepEqual([state.settings.analysis, state.settings.appearance.colourBy], ["analysis.communities.louvain", "analysis"]);
  assert.ok(entry.notes.some((note) => /^modularity 0\.\d+/.test(note)));
  // Off gives the group colouring back: one palette entry per document group, the group
  // palette's colours in order (the array itself is built per colouring, not shared).
  const palette = seen.styles.at(-1)?.palette ?? [];
  assert.deepEqual([...palette], Array.from({ length: palette.length }, (_, i) => GROUP_PALETTE[i % GROUP_PALETTE.length]));
  assert.equal(seen.frames.length, 1);
  await studio.dispatch("analysis", { id: "off" });
  assert.deepEqual([studio.store.get().analysis, studio.store.get().settings.appearance.colourBy], [null, "group"]);
});

test("an analysis that did not converge says so", { skip: SKIP }, async () => {
  const { studio } = await started();
  const entry = await studio.dispatch("analysis", { id: "eigenvector" });
  assert.equal(studio.store.get().analysis?.converged, false);
  assert.ok(entry.notes.some((note) => note.includes("did not converge")));
});

/** Across a theme, a scale, a filter and an unfilter: nothing here asks the motor. */
async function restyleOnly(made: Desk): Promise<void> {
  const { studio } = made;
  const calls = studio.store.get().layoutCalls;
  const filter = studio.store.get().runFilter;
  assert.ok(calls > 0, "the opening graph was laid out");
  await studio.dispatch("theme", { name: "light" });
  await studio.dispatch("scale", { factor: 2 });
  await studio.dispatch("filter", { text: "graph" });
  await studio.dispatch("unfilter");
  assert.equal(studio.store.get().layoutCalls, calls, "the layout count does not move");
  assert.equal(studio.store.get().runFilter, filter, "and neither does the filter the last run was under");
}

/** A `relayout` filter is one call, once; asking again with the same one is "already drawn". */
async function relayoutOnce(made: Desk): Promise<void> {
  const { studio, pipeline } = made;
  const before = studio.store.get().layoutCalls;
  const settings = studio.store.get().settings;
  const asked = { ...settings, filter: { ...settings.filter, text: "graph", relayout: true } };
  await pipeline.apply(asked);
  const after = studio.store.get();
  assert.equal(after.layoutCalls, before + 1, "one filter change is exactly one layout call");
  assert.equal(after.runFilter, JSON.stringify(asked.filter), "and the run remembers the filter");
  assert.equal(after.runFilter.includes("\"relayout\":true"), true, "which is the filter that asked for it");
  const again = await pipeline.apply({ ...after.settings, filter: asked.filter });
  assert.equal(studio.store.get().layoutCalls, before + 1, "the same filter is not laid out twice");
  assert.match(again.message, /already drawn/);
}

test("the look and the filters restyle without a new layout", { skip: SKIP }, async () => {
  const { studio, seen } = await started();
  const digest = studio.store.get().run?.digest;
  await studio.dispatch("theme", { name: "light" });
  assert.deepEqual(seen.themes.at(-1), LIGHT_THEME);
  const radii = Array.from(seen.styles.at(-1)?.radius ?? []);
  await studio.dispatch("scale", { factor: 2 });
  assert.deepEqual(Array.from(seen.styles.at(-1)?.radius ?? []), radii.map((radius) => radius * 2));
  await studio.dispatch("filter", { text: "graph" });
  const hidden = seen.styles.at(-1)?.hidden ?? new Uint8Array(0);
  assert.ok(hidden.includes(1) && hidden.includes(0));
  await studio.dispatch("unfilter");
  assert.equal(seen.styles.at(-1)?.hidden, null);
  assert.equal((await studio.dispatch("scale", { factor: 9 })).ok, false);
  assert.deepEqual([seen.frames.length, studio.store.get().run?.digest], [1, digest]);
  assert.equal(studio.store.get().settings.appearance.nodeScale, 2);
});

test("a relayout filter is one call, over the real motor too", { skip: SKIP }, async () => {
  await relayoutOnce(await started());
});

test("fitting the results frames what the search highlighted", { skip: SKIP }, async () => {
  const made = await started();
  const fit = made.pipeline.fitResults();
  assert.match(fit.message, /fitted to \d+ of 400 nodes/);
  assert.equal(made.seen.cameras.length, 1, "the camera moved once");
  const [camera] = made.seen.cameras;
  assert.ok((camera?.scale ?? 0) > 0, "and not to nowhere");
});

test("colouring by an analysis that never ran is refused", { skip: SKIP }, async () => {
  const { studio } = await started();
  const entry = await studio.dispatch("colour", { by: "analysis" });
  assert.deepEqual([entry.ok, studio.store.get().settings.appearance.colourBy], [false, "group"]);
});

test("a click selects, focus finds a node by its name, and a reload forgets both", { skip: SKIP }, async () => {
  const { studio, seen, choose } = await started();
  choose(7);
  assert.equal(studio.store.get().selected, 7);
  const label = studio.store.get().meta?.labels[12] ?? "";
  assert.equal((await studio.dispatch("focus", { node: label })).ok, true);
  assert.equal(seen.calls.at(-1), "focus 12");
  assert.equal((await studio.dispatch("focus", { node: "no such node" })).ok, false);
  assert.ok(studio.neighbours(7).length > 0);
  await studio.dispatch("synthetic", { nodes: 50 });
  assert.equal(studio.store.get().selected, -1);
});

test("the exported snapshot is the bytes that were drawn", { skip: SKIP }, async () => {
  const { studio, saved } = await started();
  assert.equal((await studio.dispatch("snapshot")).ok, true);
  const [file] = saved;
  assert.match(file?.name ?? "", /^graph-[0-9a-f]{8}\.gmsn$/);
  const bytes = new Uint8Array(await (file?.data ?? new Blob()).arrayBuffer());
  assert.equal(await sha256Hex(bytes), studio.store.get().run?.digest);
  assert.equal((await studio.dispatch("png")).ok, true);
  assert.equal(saved.at(-1)?.data.type, "image/png");
});

test("a recipe replays to the same digest in a new studio; a changed one is refused", { skip: SKIP }, async () => {
  const first = await started();
  await first.studio.dispatch("layout", { id: "spectral" });
  await first.studio.dispatch("recipe");
  const text = await (first.saved[0]?.data ?? new Blob()).text();
  const second = desk(realClient());
  const replayed = await second.studio.dispatch("replay", { text });
  assert.equal(replayed.ok, true, replayed.message);
  assert.equal(replayed.digest, first.studio.store.get().run?.digest);
  assert.deepEqual(second.studio.store.get().settings, first.studio.store.get().settings);
  const changed = text.replace('"layout.spectral"', '"layout.grid"');
  assert.notEqual(changed, text);
  const refused = await desk(realClient()).studio.dispatch("replay", { text: changed });
  assert.deepEqual([refused.ok, refused.error?.title], [false, "RecipeMismatch"]);
});

test("a fixture and a pasted document open like a generated graph", { skip: SKIP }, async () => {
  const { studio, seen } = await started();
  const fixture = await studio.dispatch("fixture", { path: "analysis/star.json" });
  assert.equal(fixture.ok, true, fixture.message);
  assert.ok(fixture.notes.some((note) => note.includes("bare id")));
  assert.deepEqual(studio.store.get().settings.source, { kind: "fixture", path: "analysis/star.json" });
  const text = '{"nodes":[{"id":"a","label":"Alpha"},{"id":"b","label":"Beta"}],"edges":[{"id":"e","source":"a","target":"b"}]}';
  const pasted = await studio.dispatch("document", { name: "two.json", text });
  assert.equal(pasted.ok, true, pasted.message);
  assert.deepEqual(seen.styles.at(-1)?.labels, ["Alpha", "Beta"]);
  assert.ok(pasted.command.length < 200);
  const broken = await studio.dispatch("document", { name: "bad.json", text: "{" });
  assert.deepEqual([broken.ok, broken.error?.title], [false, "IngestRefusal"]);
  assert.equal(studio.store.get().graph?.name, "two.json");
});

test("an edge pass is drawn over the layout, and off gives the layout's edges back", { skip: SKIP }, async () => {
  const { studio, seen } = await started();
  const plain = studio.store.get().run?.digest;
  const entry = await studio.dispatch("edges", { id: "bezier" });
  assert.equal(entry.ok, true, entry.message);
  assert.deepEqual([studio.store.get().settings.edges, seen.frames.at(-1)?.frame.edgeKind], ["post.style.bezier", "Curve"]);
  await studio.dispatch("edges", { id: "off" });
  assert.deepEqual([studio.store.get().settings.edges, studio.store.get().run?.digest], [null, plain]);
});
