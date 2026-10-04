/** The real motor for the tests that need one: the SDK over the built wasm module. */
import { assembleColumns, createMotor } from "../../../crates/graph-sdk-js/src/index.ts";
import { readFile } from "node:fs/promises";

import { type MotorClient, createClient } from "../src/motor/client.ts";
import type { Envelope, Port, Result, RunReport } from "../src/motor/protocol.ts";
import { createPump } from "../src/motor/pump.ts";
import { type Session, createSession, sha256Hex } from "../src/motor/session.ts";
import { FIXTURES } from "../src/source/fixtures.ts";
import type { Source } from "../src/state/settings.ts";
import { wasmBytes } from "./support.ts";

const FIXTURE_ROOT = new URL("../../../fixtures/", import.meta.url);

export const WASM = await wasmBytes();
/** Passed to `test(..., { skip })`: a run without the module reports skips, never passes. */
export const SKIP = WASM === null ? "graph_wasm.wasm is not built (scripts/studio.sh build)" : false;

export function realSession(): Session {
  return createSession({
    motorFrom: () => createMotor(WASM ?? new Uint8Array(0)),
    fetchText: (url) => readFile(new URL(url.replace("fixtures:/", ""), FIXTURE_ROOT), "utf8"),
    digest: sha256Hex,
    assemble: assembleColumns,
    now: () => performance.now(),
  });
}

export const FIXTURES_URL = "fixtures:/";
export { FIXTURES };

/** The real session behind a port, answering on this thread. */
export function spawnReal(): Port {
  let handler: ((message: Envelope<Result>) => void) | null = null;
  const pump = createPump(realSession(), (message) => handler?.(message));
  return {
    send: pump,
    listen: (next) => {
      handler = next;
    },
    close: () => {
      handler = null;
    },
  };
}

export function realClient(): MotorClient {
  return createClient(spawnReal, { wasmUrl: "unused", fixturesUrl: FIXTURES_URL });
}

/** One real run of `layoutId` over `source`, for a scripted client to hand back. */
export async function realRun(source: Source, layoutId: string): Promise<RunReport> {
  const session = realSession();
  await session.open("unused");
  await session.load(source, FIXTURES_URL);
  return session.layout(layoutId, null);
}
