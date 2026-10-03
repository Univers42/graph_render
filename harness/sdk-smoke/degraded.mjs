// A degraded motor fails predictably (kill switch and compile failure).

import {
  WasmUnavailableError,
  createMotor,
  resetForTests,
} from "../../crates/graph-sdk-js/src/index.ts";
import { check, refusedWith } from "./lib.mjs";

// The kill switch's own message, `wasm.ts:167`: the only thing that tells a refusal caused
// by the switch apart from one caused by a module that would not load. Nothing checked it,
// so pointing `createMotor` at a module with any import left all seven rows green (m96).
const KILL_SWITCH_MARK = "__GM_DISABLE_WASM__ is set";

// Every method the contract names as having to fail predictably on a degraded motor
// (`docs/contract/wasm-abi.md:537`, `:583-585`): the seven the section already covered plus
// `layout`, `column`, `toJSON`, `toBytes` and `release`. Five of the twelve had no coverage
// at all (M31). Each entry is called against a live motor, so a `degraded` of the wrong
// reason is caught by the message rather than by the class alone.
const PREDICTABLE = [
  ["build", (m) => m.build(INGEST)],
  ["layouts", (m) => m.layouts()],
  ["posts", (m) => m.posts()],
  ["analyses", (m) => m.analyses()],
  ["post", (m) => m.post(1, "post.style.straight")],
  ["layout", (m) => m.layout(1, "layout.grid")],
  ["column", (m) => m.column(1, 0)],
  ["toJSON", (m) => m.toJSON(1)],
  ["toBytes", (m) => m.toBytes(1)],
  ["analysis", (m) => m.analysis(1, "analysis.components.weak")],
  ["forceSession", (m) => m.forceSession(1)],
  ["release", (m) => m.release(1)],
];

/** The two-node fixture `build.mjs` built, restated so the degraded arms can call `build`. */
const INGEST = JSON.stringify({
  version: 1,
  nodes: [
    { id: "a", kind: "record", database_id: null, source: "s", label: "A", group: null, weight: 0.5, version: 0, has_note: false, icon: null },
    { id: "b", kind: "note", database_id: null, source: "s", label: "B", group: null, weight: 0.5, version: 0, has_note: false, icon: null },
  ],
  edges: [
    { id: "e", source: "a", target: "b", kind: "relation", label: "", strength: 0.5, directed: false, record_id: null },
  ],
});

/** Run one block with the kill switch set, and put the global back exactly as it was —
 *  by deleting it when it was absent, not by assigning `undefined` to it (m98). */
async function withKillSwitch(run) {
  const had = Object.hasOwn(globalThis, "__GM_DISABLE_WASM__");
  const prior = globalThis.__GM_DISABLE_WASM__;
  globalThis.__GM_DISABLE_WASM__ = true;
  try {
    return await run();
  } finally {
    if (had) globalThis.__GM_DISABLE_WASM__ = prior;
    else delete globalThis.__GM_DISABLE_WASM__;
  }
}

/** Every predicted refusal on `motor`, each one naming its method. `switched` says the
 *  kill switch is held for the whole block; either way the global is left as it was found,
 *  so a block cannot leak the switch into the next one and make it pass for the wrong
 *  reason (which is exactly what the first version of this helper did). */
async function checkPredictable(motor, suffix, switched) {
  const block = async () => {
    for (const [method, call] of PREDICTABLE) {
      check(
        `degraded_motor_${method}_fails_predictably_${suffix}`,
        await refusedWith(WasmUnavailableError, () => call(motor)),
      );
    }
  };
  return switched ? withKillSwitch(block) : block();
}

/** The message of the refusal `call` makes, or `""` when it makes none. */
async function refusalMessage(call) {
  try {
    await call();
    return "";
  } catch (error) {
    return String(error?.message ?? error);
  }
}

export async function runDegradedSection(ctx) {
  const { bytes } = ctx;
// Regression (review finding, MAJOR): createMotor() must never throw on a load failure
// (prompt.md §3.2, phase-04-wasm-sdk.md step 5: "warn-and-degrade rather than throw ...
// A motor that throws on load takes the host page down with it") — it must resolve to a
// degraded Motor whose own calls fail predictably instead, never fabricated data.
{
  await withKillSwitch(async () => {
    let threw = false;
    let degraded;
    try {
      degraded = await createMotor(bytes);
    } catch {
      threw = true;
    }
    check("createMotor_never_throws_on_kill_switch", !threw);
    // A degraded motor must not answer `layouts()` with an empty registry: "this module has
    // no layouts" and "this module never loaded" are different facts, and only one of them
    // is true. Every method that needs the module refuses, and says the switch is why.
    await checkPredictable(degraded, "kill_switch", true);
    const why = await refusalMessage(() => degraded.layouts());
    check(
      "every refusal names the kill switch, not an unrelated load failure",
      why.includes(KILL_SWITCH_MARK),
      why,
    );
  });
  check(
    "the kill switch is restored exactly: absent stays absent",
    !Object.hasOwn(globalThis, "__GM_DISABLE_WASM__") || globalThis.__GM_DISABLE_WASM__ !== true,
  );
}
{
  resetForTests();
  let threw = false;
  let degraded;
  try {
    degraded = await createMotor(new Uint8Array([0, 1, 2, 3]));
  } catch {
    threw = true;
  }
  check("createMotor_never_throws_on_compile_failure", !threw);
  // The whole set, not just `build`: a motor degraded for the wrong reason (a module that
  // loaded but is stale) would pass a `build`-only block (m97). The switch is absent here,
  // so `checkPredictable` does not set it.
  await checkPredictable(degraded, "compile_failure", false);
  resetForTests();
}
// A module older than the SDK (a stale staged wasm) is refused at load and named, rather than
// failing later as `exports.gm_dim is not a function`. This one exports `memory` only.
{
  resetForTests();
  const memoryOnly = new Uint8Array([
    0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, 0x05, 0x03, 0x01, 0x00, 0x01,
    0x07, 0x0a, 0x01, 0x06, 0x6d, 0x65, 0x6d, 0x6f, 0x72, 0x79, 0x02, 0x00,
  ]);
  const degraded = await createMotor(memoryOnly);
  let cause = "";
  try {
    degraded.build(INGEST);
  } catch (error) {
    cause = error instanceof WasmUnavailableError ? String(error.reason?.message) : "";
  }
  check("stale_module_refused_at_load_naming_gm_dim", cause.includes("gm_dim"));
  resetForTests();
}
}