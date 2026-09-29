/** The real motor for the tests that need one: the SDK over the built wasm module. */
import { createMotor } from "../../../crates/graph-sdk-js/src/index.ts";
import { readFile } from "node:fs/promises";

import { FIXTURES } from "../src/source/fixtures.ts";
import { type Session, createSession, sha256Hex } from "../src/motor/session.ts";
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
    now: () => performance.now(),
  });
}

export const FIXTURES_URL = "fixtures:/";
export { FIXTURES };
