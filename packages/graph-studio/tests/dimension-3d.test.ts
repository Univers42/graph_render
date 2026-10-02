// A `dim` of 1 is drawn, not refused: the renderer reads the z column and projects it
// (`docs/decisions/studio-3d.md`). What is left of the old refusal-by-name is the dim this
// reader cannot express, and it is still named rather than read past.
//
// A `dim` past 1 is a reserved value, not a 3D snapshot: reading it as one would put the
// columns at offsets nothing in the contract says they are at. So the row here is a refusal
// with its own code, and the negative control is the same flow over a `dim` of 1, which must
// draw — a reader that refused every non-zero dim would pass this file and be useless.
import assert from "node:assert/strict";
import { test } from "node:test";

import { decodeSnapshot } from "../../graph-render/src/snapshot/decode.ts";
import { createClient, type MotorClient } from "../src/motor/client.ts";
import { createPump } from "../src/motor/pump.ts";
import type { Envelope, Port, Result, RunReport } from "../src/motor/protocol.ts";
import { type MotorLike, type Session, createSession } from "../src/motor/session.ts";
import { SCRIPTED_META, desk, savedBytes, scriptBytes, spaceBytes } from "./desk.ts";

/** The 3D bytes with the dim byte set to `dim`, which is all a reserved value is. */
function atDim(dim: number): Uint8Array {
  const bytes = spaceBytes();
  bytes[14] = dim;
  return bytes;
}

/** The scripted motor, handing back each of `runs` in turn; the last one repeats. */
function bytesClient(runs: readonly Uint8Array[]): MotorClient {
  const base = scripted();
  let at = 0;
  return {
    ...base,
    layout: (layoutId: string, postId: string | null): Promise<RunReport> => {
      const bytes = runs[Math.min(at, runs.length - 1)] ?? scriptBytes();
      at += 1;
      return Promise.resolve({
        layoutId, postId, postError: null, bytes, digest: null,
        layoutMs: 1, postMs: 0, meta: SCRIPTED_META,
      });
    },
  };
}

function scripted(): MotorClient {
  return {
    catalog: () => Promise.resolve({ layouts: ["layout.grid"], posts: [], analyses: [] }),
    load: () => Promise.resolve({ name: "vault seed 1", nodeCount: 3, edgeCount: 2, notes: [], buildMs: 1 }),
    layout: (layoutId, postId) => Promise.resolve({
      layoutId, postId, postError: null, bytes: scriptBytes(), digest: null,
      layoutMs: 1, postMs: 0, meta: SCRIPTED_META,
    }),
    analysis: () => Promise.reject(new Error("this test client measures nothing")),
    cancel: () => false,
    busy: () => false,
    close: () => undefined,
  };
}

/** The motor whose runs carry `dim`, behind this thread's port instead of a worker. */
function dimMotor(dim: number): MotorLike<number> {
  return {
    layouts: () => ["layout.grid"], posts: () => [], analyses: () => [],
    build: () => 1, layout: () => undefined, post: () => undefined,
    analysis: () => {
      throw new Error("this test motor analyses nothing");
    },
    toBytes: () => atDim(dim), release: () => undefined,
  };
}

function dimSession(dim: number): Session {
  return createSession({
    motorFrom: () => Promise.resolve(dimMotor(dim)),
    fetchText: () => Promise.reject(new Error("this test motor fetches nothing")),
    digest: () => Promise.resolve(null),
    now: () => 0,
  });
}

/** The real session and the real client over a port answered on this thread. */
function dimClient(dim: number): MotorClient {
  let handler: ((message: Envelope<Result>) => void) | null = null;
  const spawn = (): Port => ({
    send: createPump(dimSession(dim), (message) => handler?.(message)),
    listen: (next) => {
      handler = next;
    },
    close: () => {
      handler = null;
    },
  });
  return createClient(spawn, { wasmUrl: "unused", fixturesUrl: "fixtures:/" });
}

test("a reserved dim is refused by name at the pipeline, and nothing is drawn of it", async () => {
  const { studio, seen, pipeline } = desk(bytesClient([atDim(2)]));
  const entry = await studio.start();
  assert.deepEqual([entry.ok, entry.error?.title, entry.error?.code], [false, "SnapshotRefusal", "reserved-dim"]);
  assert.match(entry.error?.detail ?? "", /found 2/);
  assert.match(entry.error?.detail ?? "", /reserved/, "and it says the value is reserved");
  const state = studio.store.get();
  assert.deepEqual([state.run, state.meta, state.error?.code], [null, null, "reserved-dim"]);
  assert.equal(seen.frames.at(-1)?.frame.nodeCount, 0);
  assert.equal(pipeline.bytes(), null);
});

test("a reserved dim is refused over the wire too, before the drawing is decoded", async () => {
  // The session path, over the real port: a refusal here is the motor's, and it has to reach
  // the studio with the same code the decoder would have given.
  const { studio, seen } = desk(dimClient(3));
  const entry = await studio.start();
  assert.deepEqual([entry.ok, entry.error?.title, entry.error?.code], [false, "SnapshotRefusal", "reserved-dim"]);
  assert.equal(studio.store.get().graph?.name, "vault seed 1", "the graph loaded; the layout did not");
  assert.equal(studio.store.get().run, null);
  assert.equal(seen.frames.at(-1)?.frame.nodeCount, 0);
});

test("every dim past 1 is refused, each naming its own value", async () => {
  for (const dim of [2, 3, 127, 255]) {
    const { studio } = desk(bytesClient([atDim(dim)]));
    const entry = await studio.start();
    assert.deepEqual([entry.ok, entry.error?.code], [false, "reserved-dim"], `dim ${dim}`);
    assert.match(entry.error?.detail ?? "", new RegExp(`found ${dim}`), `dim ${dim}`);
  }
});

test("export.snapshot of a refused run is refused and saves nothing", async () => {
  const { studio, saved } = desk(bytesClient([atDim(2)]));
  await studio.start();
  const entry = await studio.dispatch("export.snapshot");
  assert.deepEqual([entry.ok, entry.error?.title, entry.error?.code], [false, "ActionRefusal", "unavailable"]);
  assert.match(entry.error?.detail ?? "", /nothing is drawn/);
  assert.deepEqual(saved, []);
  // The refusal stayed in the log under the reader's own name, and nothing was saved.
  assert.deepEqual(studio.store.get().log.map((line) => line.error?.code), ["reserved-dim", "unavailable"]);
});

test("a reserved dim over a 2D drawing leaves the 2D bytes as the ones on screen", async () => {
  // A refused layout must not cost the user the drawing they had: the studio keeps what it
  // was showing, and the saved snapshot is the 2D one, byte for byte.
  const { studio, saved } = desk(bytesClient([scriptBytes(), atDim(2)]));
  assert.equal((await studio.start()).ok, true);
  assert.equal((await studio.dispatch("layout", { id: "layout.grid" })).error?.code, "reserved-dim");
  assert.equal((await studio.dispatch("export.snapshot")).ok, true);
  const written = await savedBytes(saved);
  assert.equal(written[14], 0);
  assert.equal(decodeSnapshot(written).nodeCount, 3);
});

test("a dim of 1 is not a reserved dim: the same flow draws it", async () => {
  // The negative control. A reader that refused every non-zero dim would pass every row above
  // and leave the 3D layouts unusable, which is the whole thing this file used to prevent.
  const { studio, seen, saved } = desk(bytesClient([atDim(1)]));
  const entry = await studio.start();
  assert.deepEqual([entry.ok, entry.error], [true, null]);
  assert.equal(studio.store.get().run?.dim, 1);
  assert.equal(seen.frames.at(-1)?.frame.z?.length, 3, "the painter is handed a z column");
  assert.equal((await studio.dispatch("export.snapshot")).ok, true);
  assert.equal((await savedBytes(saved))[14], 1);
});

test("a dim of 0 is not a reserved dim either: the 2D flow is unchanged", async () => {
  const { studio, saved } = desk(bytesClient([scriptBytes()]));
  assert.equal((await studio.start()).ok, true);
  assert.equal(studio.store.get().run?.dim, 0);
  assert.equal((await studio.dispatch("export.snapshot")).ok, true);
  const written = await savedBytes(saved);
  assert.equal(written.byteLength, scriptBytes().byteLength);
});
