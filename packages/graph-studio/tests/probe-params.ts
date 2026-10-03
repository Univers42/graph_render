import { createMotor } from "../../../crates/graph-sdk-js/src/index.ts";
import { readFile } from "node:fs/promises";
import { createSession, sha256Hex } from "../src/motor/session.ts";
import { FIXTURES } from "../src/source/fixtures.ts";
import { OPENING_SOURCE } from "../src/state/settings.ts";
import { wasmBytes } from "./support.ts";

const WASM = (await wasmBytes()) ?? new Uint8Array(0);
const FIXTURE_ROOT = new URL("../../../fixtures/", import.meta.url);
const session = createSession({
  motorFrom: async () => {
    const motor = await createMotor(WASM);
    const run = motor.run.bind(motor);
    motor.run = (handle, layoutId, options) => {
      console.log("SDK run:", layoutId, JSON.stringify(options?.params));
      return run(handle, layoutId, options);
    };
    return motor;
  },
  fetchText: (url) => readFile(new URL(url.replace("fixtures:/", ""), FIXTURE_ROOT), "utf8"),
  digest: sha256Hex,
  now: () => performance.now(),
});
await session.open("unused");
await session.load(OPENING_SOURCE, "fixtures:/");
void FIXTURES;
const sha = async (bytes) => Buffer.from(await globalThis.crypto.subtle.digest("SHA-256", bytes)).toString("hex").slice(0, 16);

const plain = await session.layout("layout.force.graphopt", null);
console.log("plain  ", await sha(plain.bytes));
const moved = await session.layout("layout.force.graphopt", null, { niter: 3 });
console.log("niter 3", await sha(moved.bytes));
const wide = await session.layout("layout.force.graphopt", null, { node_charge: 5000, spring_length: 500 });
console.log("charge ", await sha(wide.bytes));
try {
  await session.layout("layout.force.graphopt", null, { niter: 10001 });
  console.log("niter 10001 accepted");
} catch (error) {
  console.log("niter 10001:", String(error));
}
