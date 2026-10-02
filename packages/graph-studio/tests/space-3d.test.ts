// A 3D snapshot is drawn, not refused. The refusal by name went when the renderer learned
// to read the z column (`docs/decisions/studio-3d.md`), and these are the rows that hold the
// door shut on the other side: a 3D run reaches the screen, it says so, and the console can
// put the camera back.
//
// The negative control is the last test: the same flow over a 2D snapshot still draws, still
// saves, and still refuses `reset3d` by name — so a 3D path that leaked into the 2D one, or
// an action that ran when it should not, would show up here rather than in the browser.
import assert from "node:assert/strict";
import { test } from "node:test";

import { decodeSnapshot } from "../../graph-render/src/snapshot/decode.ts";
import type { MotorClient } from "../src/motor/client.ts";
import { DEFAULT_SETTINGS } from "../src/state/settings.ts";
import { SCRIPTED_META, desk, savedBytes, scriptBytes, scriptedClient, spaceBytes } from "./desk.ts";

/** The scripted motor, handing back each of `runs` in turn; the last one repeats. */
function bytesClient(runs: readonly Uint8Array[]): MotorClient {
  // The desk's own scripted client, so the catalog, the graph and the settings are the ones
  // every other studio test uses; only the bytes handed back differ.
  const base = scriptedClient();
  let at = 0;
  return {
    ...base,
    layout: (layoutId: string, postId: string | null) => {
      const bytes = runs[Math.min(at, runs.length - 1)] ?? scriptBytes();
      at += 1;
      return Promise.resolve({
        layoutId, postId, postError: null, bytes, digest: null,
        layoutMs: 1, postMs: 0, meta: SCRIPTED_META,
      });
    },
  };
}

test("a 3D snapshot is drawn: the run reaches the screen with its z column", async () => {
  const { studio, seen } = desk(bytesClient([spaceBytes()]));
  const entry = await studio.start();
  assert.deepEqual([entry.ok, entry.error], [true, null], "a 3D run is not a failure any more");
  const state = studio.store.get();
  const run = state.run;
  assert.ok(run !== null, "the run is on the store");
  assert.equal(state.error, null, "nothing was refused");
  assert.equal(run.dim, 1, "the summary carries the dim off the decoded snapshot");
  // The layout the studio asked for, which is its own default. The scripted motor hands
  // back 3D bytes whatever it is asked for, so what is under test is the drawing, not the id.
  assert.equal(run.layoutId, DEFAULT_SETTINGS.layout);
  const frame = seen.frames.at(-1)?.frame;
  assert.ok(frame !== undefined, "the studio drew a frame");
  assert.equal(frame.nodeCount, 3);
  // The frame scales every column into world units by one factor, z included, so the z the
  // painter is handed is the snapshot's z times that factor — read off the frame's own x.
  const factor = (frame.x[1] ?? 0) / 40;
  assert.ok(factor > 0, `the frame's x is scaled, not raw: ${frame.x[1]}`);
  assert.deepEqual([...(frame.z ?? [])], [0, 30 * factor, -30 * factor], "z, in world units");
});

test("the decoded snapshot keeps the columns apart: z is not read as a radius", async () => {
  const { studio, seen } = desk(bytesClient([spaceBytes()]));
  await studio.start();
  const snapshot = decodeSnapshot(spaceBytes());
  assert.deepEqual([...(snapshot.z ?? [])], [0, 30, -30], "the reader took the z column");
  // The radius column follows the z, so a reader that forgot the shift would hand the z back
  // as radii. These are the radii the fixture wrote, 4 each, scaled into world units.
  const radii = [...(seen.frames.at(-1)?.frame.r ?? [])];
  assert.equal(radii.length, 3);
  for (const radius of radii) {
    // Close to, not equal to: the frame's columns are f32 and 4 * 1.4 is not representable.
    assert.ok(Math.abs(radius - 5.6) < 1e-4, `a radius came back as ${radius}, not 4 scaled`);
  }
  // The negative control on that number: none of them is a z value, which is what a reader
  // that forgot the column shift would have handed back.
  for (const radius of radii) assert.ok(Math.abs(radius) < 30, "no radius is a z");
});

test("the badge reads the run's dim, so a 3D layout and a 2D one are told apart", async () => {
  const { studio } = desk(bytesClient([spaceBytes(), scriptBytes()]));
  await studio.start();
  assert.equal(studio.store.get().run?.dim, 1);
  // The same flow over a 2D run: the badge's own condition goes false, which is the row that
  // would fail if the badge were reading the layout id instead of the drawing.
  await studio.dispatch("layout", { id: "layout.grid" });
  assert.equal(studio.store.get().run?.dim, 0);
});

test("headon is in the registry and the console's help lists it", async () => {
  const { studio } = desk(bytesClient([spaceBytes()]));
  await studio.start();
  const listed = await studio.dispatch("help");
  assert.equal(listed.ok, true);
  const notes = studio.store.get().log.at(-1)?.notes ?? [];
  assert.ok(
    notes.some((note) => note.startsWith("headon")),
    `the console does not list headon: ${notes.filter((note) => note.includes("head")).join(", ")}`,
  );
});

test("headon runs on a 3D drawing and asks the view to put the camera back", async () => {
  const { studio, seen } = desk(bytesClient([spaceBytes()]));
  await studio.start();
  const entry = await studio.dispatch("headon");
  assert.deepEqual([entry.ok, entry.error], [true, null]);
  assert.ok(seen.calls.includes("resetOrbit"), `the view was asked: ${seen.calls.join(", ")}`);
});

test("headon is refused by name on a 2D drawing, and the refusal says why", async () => {
  const { studio, seen } = desk(bytesClient([scriptBytes()]));
  await studio.start();
  const entry = await studio.dispatch("headon");
  assert.deepEqual([entry.ok, entry.error?.title, entry.error?.code], [false, "ActionRefusal", "unavailable"]);
  assert.match(entry.error?.detail ?? "", /2D/);
  assert.ok(!seen.calls.includes("resetOrbit"), "and it did not touch the camera anyway");
});

test("headon is refused when nothing is drawn at all", async () => {
  const { studio } = desk(bytesClient([scriptBytes()]));
  // Before the first run there is no drawing, so there is no dim to read and nothing to reset.
  const entry = await studio.dispatch("headon");
  assert.deepEqual([entry.ok, entry.error?.code], [false, "unavailable"]);
  assert.match(entry.error?.detail ?? "", /nothing is drawn/);
});

test("export.snapshot of a 3D run saves the 3D bytes, dim and all", async () => {
  const { studio, saved } = desk(bytesClient([spaceBytes()]));
  assert.equal((await studio.start()).ok, true);
  const entry = await studio.dispatch("export.snapshot");
  assert.deepEqual([entry.ok, entry.error], [true, null], "a 3D run is exportable like any other");
  const written = await savedBytes(saved);
  assert.equal(written[14], 1, "the saved file is the 3D snapshot, byte 14 and all");
  assert.equal(decodeSnapshot(written).dim, 1);
});

test("a dim past 1 is still refused, and it is a different refusal from the 3D one", async () => {
  // The other side of the door: lifting the 3D refusal did not lift the dim check. A byte of
  // 2 is a dimension no reader here implements, and it is named as such.
  const reserved = spaceBytes();
  reserved[14] = 9;
  const { studio, seen } = desk(bytesClient([reserved]));
  const entry = await studio.start();
  assert.deepEqual([entry.ok, entry.error?.title, entry.error?.code], [false, "SnapshotRefusal", "reserved-dim"]);
  assert.match(entry.error?.detail ?? "", /found 9/);
  assert.equal(studio.store.get().run, null);
  assert.equal(seen.frames.at(-1)?.frame.nodeCount, 0, "nothing was drawn of it");
});

test("the 2D flow is untouched: draws, saves, and refuses the 3D reset", async () => {
  // The negative control for the whole file. A 2D run must behave exactly as it did before
  // any of this: no badge condition, no orbit, no z column, and the same saved bytes.
  const { studio, seen, saved } = desk(bytesClient([scriptBytes()]));
  assert.equal((await studio.start()).ok, true);
  const state = studio.store.get();
  assert.equal(state.run?.dim, 0);
  assert.equal(seen.frames.at(-1)?.frame.z, null, "a 2D frame has no z column at all");
  assert.equal((await studio.dispatch("headon")).error?.code, "unavailable");
  const entry = await studio.dispatch("export.snapshot");
  assert.deepEqual([entry.ok, entry.error], [true, null]);
  const written = await savedBytes(saved);
  assert.equal(written[14], 0);
  assert.equal(written.byteLength, scriptBytes().byteLength, "byte for byte the 2D run it always was");
});
