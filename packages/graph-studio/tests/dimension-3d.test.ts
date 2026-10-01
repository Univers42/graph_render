// The studio is 2D-only: a 3D snapshot is refused by name on the way in, at both the
// session and the pipeline, and `export.snapshot` never writes one. The negative control is
// the last test: the same flow over a 2D snapshot still draws and still saves.
//
// A 3D snapshot is a 2D one with header byte 14 set to 1: the reader refuses at byte 14,
// before any geometry tag, so nothing past the header has to be meaningful.
import assert from "node:assert/strict";
import { test } from "node:test";

import { decodeSnapshot } from "../../graph-render/src/snapshot/decode.ts";
import { createClient, type MotorClient } from "../src/motor/client.ts";
import { createPump } from "../src/motor/pump.ts";
import type { Envelope, Port, Result, RunReport } from "../src/motor/protocol.ts";
import { type MotorLike, type Session, createSession } from "../src/motor/session.ts";
import { SCRIPTED_META, desk, scriptBytes, scriptedClient } from "./desk.ts";

function threeD(): Uint8Array {
  const bytes = scriptBytes();
  bytes[14] = 1;
  return bytes;
}

/** The scripted motor, handing back each of `runs` in turn; the last one repeats. */
function bytesClient(runs: readonly Uint8Array[]): MotorClient {
  const base = scriptedClient();
  let at = 0;
  const layout = (layoutId: string, postId: string | null): Promise<RunReport> => {
    const bytes = runs[Math.min(at, runs.length - 1)] ?? scriptBytes();
    at += 1;
    return Promise.resolve({
      layoutId, postId, postError: null, bytes, digest: null,
      layoutMs: 1, postMs: 0, meta: SCRIPTED_META,
    });
  };
  return { ...base, layout };
}

/** The motor whose runs are 3D, behind this thread's port instead of a worker. */
function threeDMotor(): MotorLike<number> {
  return {
    layouts: () => ["layout.grid"], posts: () => [], analyses: () => [],
    build: () => 1, layout: () => undefined, post: () => undefined,
    analysis: () => {
      throw new Error("the 3D test motor analyses nothing");
    },
    toBytes: () => threeD(), release: () => undefined,
  };
}

function threeDSession(): Session {
  return createSession({
    motorFrom: () => Promise.resolve(threeDMotor()),
    fetchText: () => Promise.reject(new Error("the 3D test motor fetches nothing")),
    digest: () => Promise.resolve(null),
    now: () => 0,
  });
}

/** The real session and the real client over a port answered on this thread. */
function threeDClient(): MotorClient {
  let handler: ((message: Envelope<Result>) => void) | null = null;
  const spawn = (): Port => ({
    send: createPump(threeDSession(), (message) => handler?.(message)),
    listen: (next) => {
      handler = next;
    },
    close: () => {
      handler = null;
    },
  });
  return createClient(spawn, { wasmUrl: "unused", fixturesUrl: "fixtures:/" });
}

test("a 3D snapshot is refused by name at the pipeline, and nothing is drawn of it", async () => {
  const { studio, seen, pipeline } = desk(bytesClient([threeD()]));
  const entry = await studio.start();
  const state = studio.store.get();
  assert.deepEqual([entry.ok, entry.error?.title, entry.error?.code], [false, "SnapshotRefusal", "dimension-3d"]);
  assert.match(entry.error?.detail ?? "", /found 1, only 0 is read/);
  assert.deepEqual([state.run, state.meta, state.error?.code], [null, null, "dimension-3d"]);
  assert.equal(seen.frames.at(-1)?.frame.nodeCount, 0);
  assert.equal(pipeline.bytes(), null);
});

test("a 3D snapshot is refused by name in the session, over the wire, before it is drawn", async () => {
  const { studio, seen, pipeline } = desk(threeDClient());
  const entry = await studio.start();
  const state = studio.store.get();
  assert.deepEqual([entry.ok, entry.error?.title, entry.error?.code], [false, "SnapshotRefusal", "dimension-3d"]);
  assert.deepEqual([state.graph?.name, state.run, state.error?.code], ["vault seed 1", null, "dimension-3d"]);
  assert.equal(seen.frames.at(-1)?.frame.nodeCount, 0);
  assert.equal(pipeline.bytes(), null);
});

test("export.snapshot of a 3D run is refused and saves nothing", async () => {
  const { studio, saved } = desk(bytesClient([threeD()]));
  await studio.start();
  const entry = await studio.dispatch("export.snapshot");
  assert.deepEqual([entry.ok, entry.error?.title, entry.error?.code], [false, "ActionRefusal", "unavailable"]);
  assert.match(entry.error?.detail ?? "", /nothing is drawn/);
  assert.deepEqual(saved, []);
  // The refusal stayed in the log under the reader's own name, and nothing was saved.
  assert.deepEqual(studio.store.get().log.map((line) => line.error?.code), ["dimension-3d", "unavailable"]);
});

test("a 3D run over a 2D drawing saves the 2D bytes, never the 3D ones", async () => {
  const { studio, saved } = desk(bytesClient([scriptBytes(), threeD()]));
  assert.equal((await studio.start()).ok, true);
  assert.equal((await studio.dispatch("layout", { id: "layout.grid" })).error?.code, "dimension-3d");
  const entry = await studio.dispatch("export.snapshot");
  assert.equal(entry.ok, true);
  const written = await saved[0]?.data.arrayBuffer();
  assert.ok(written !== undefined);
  assert.equal(new Uint8Array(written)[14], 0);
  assert.equal(decodeSnapshot(new Uint8Array(written)).nodeCount, 3);
});

test("the same flow over a 2D snapshot draws and saves: the refusal is not vacuous", async () => {
  const { studio, saved } = desk(bytesClient([scriptBytes()]));
  assert.equal((await studio.start()).ok, true);
  assert.equal(studio.store.get().run?.nodeKind, "Circle");
  const entry = await studio.dispatch("export.snapshot");
  assert.deepEqual([entry.ok, entry.error, studio.store.get().error], [true, null, null]);
  assert.match(saved[0]?.name ?? "", /^graph-(undigested|[0-9a-f]{8})\.gmsn$/);
  const written = await saved[0]?.data.arrayBuffer();
  assert.ok(written !== undefined);
  assert.equal(new Uint8Array(written).byteLength, scriptBytes().byteLength);
});
