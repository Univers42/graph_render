// A degraded motor fails predictably (kill switch and compile failure).

import {
  WasmUnavailableError,
  createMotor,
  resetForTests,
} from "../../crates/graph-sdk-js/src/index.ts";
import { check, refusedWith } from "./lib.mjs";

export async function runDegradedSection(ctx) {
  const { bytes, ingest } = ctx;
// Regression (review finding, MAJOR): createMotor() must never throw on a load failure
// (prompt.md §3.2, phase-04-wasm-sdk.md step 5: "warn-and-degrade rather than throw ...
// A motor that throws on load takes the host page down with it") — it must resolve to a
// degraded Motor whose own calls fail predictably instead, never fabricated data.
{
  const priorKillSwitch = globalThis.__GM_DISABLE_WASM__;
  globalThis.__GM_DISABLE_WASM__ = true;
  let threw = false;
  let degraded;
  try {
    degraded = await createMotor(bytes);
  } catch {
    threw = true;
  }
  check("createMotor_never_throws_on_kill_switch", !threw);
  let buildRefused = false;
  try {
    degraded?.build(ingest);
  } catch (error) {
    buildRefused = error instanceof WasmUnavailableError;
  }
  check("degraded_motor_build_fails_predictably_kill_switch", buildRefused);
  // A degraded motor must not answer `layouts()` with an empty registry: "this module has
  // no layouts" and "this module never loaded" are different facts, and only one of them
  // is true. It refuses like every other method that needs the module.
  check(
    "degraded_motor_layouts_fails_predictably_kill_switch",
    await refusedWith(WasmUnavailableError, () => degraded?.layouts()),
  );
  // Same for the two registries added after it: an empty list would read as "this module
  // has no post capabilities" / "no analyses", which is a different fact from "this
  // module never loaded", and only one of them is true.
  check(
    "degraded_motor_posts_fails_predictably_kill_switch",
    await refusedWith(WasmUnavailableError, () => degraded?.posts()),
  );
  check(
    "degraded_motor_analyses_fails_predictably_kill_switch",
    await refusedWith(WasmUnavailableError, () => degraded?.analyses()),
  );
  check("degraded_motor_post_fails_predictably_kill_switch", await refusedWith(WasmUnavailableError, () => degraded?.post(1, "post.style.straight")));
  check("degraded_motor_analysis_fails_predictably_kill_switch", await refusedWith(WasmUnavailableError, () => degraded?.analysis(1, "analysis.components.weak")));
  globalThis.__GM_DISABLE_WASM__ = priorKillSwitch;
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
  let buildRefused = false;
  try {
    degraded?.build(ingest);
  } catch (error) {
    buildRefused = error instanceof WasmUnavailableError;
  }
  check("degraded_motor_build_fails_predictably_compile_failure", buildRefused);
  resetForTests();
}
}
