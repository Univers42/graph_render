// A third party's own smoke test of the published SDK: imports only
// `crates/graph-sdk-js/src/index.ts`'s public surface — never a raw wasm export, never
// `wasm.ts`/`views.ts` directly, the same way a real consumer would. Exercises a full
// build/layout/column/toJSON/release round trip, **every layout the module's own registry
// names** (phase step 7: "runs each gated layout, and prints node counts and bounds" — the
// list comes from `Motor#layouts`, i.e. `gm_layout_count`/`gm_layout_id`, never a literal
// name, so a layout registered after this script was written is covered with no edit here),
// the contract's own column-presence table restated kind by kind (C3: the reserved Circle
// `r` / Box `w,h` and Polyline columns), a NaN written through a zero-copy column view
// refusing at `toJSON` (C8's D9 re-validation), a released handle staying refused rather
// than silently answered (C6), and `options` acceptance (C16).
//
//   node --experimental-strip-types harness/sdk-smoke.mjs <graph_wasm.wasm>
//   node --experimental-strip-types harness/sdk-smoke.mjs --adapter-convergence [<wasm>]
//
// With a wasm path, `--adapter-convergence` runs the **whole** phase proof in this one
// process: two adapters → one contract document → `Motor#buildContract` → the committed
// `expected-graph.json` graph, plus the refusals for four mutated documents. Without one
// it checks the adapter half alone, which is how the pre-`gm_build_contract` gate row is
// written and keeps working.
//
// Exit codes follow graph-cli: 0 pass, 1 ran and failed, 2 could not run.

import { readFile } from "node:fs/promises";
import { ColumnId, InvalidOptionsError, createMotor } from "../crates/graph-sdk-js/src/index.ts";
import { check, columnIdProblems, fail, finish, refusedWith } from "./sdk-smoke/lib.mjs";
import { runConvergence } from "./sdk-smoke/convergence.mjs";
import { runBuildSection } from "./sdk-smoke/build.mjs";
import { runLayoutSection } from "./sdk-smoke/layouts.mjs";
import { runPostSection } from "./sdk-smoke/post.mjs";
import { runAnalysisSection } from "./sdk-smoke/analysis.mjs";
import { runTransportSection } from "./sdk-smoke/transport.mjs";
import { runForceSection } from "./sdk-smoke/force.mjs";
import { runDegradedSection } from "./sdk-smoke/degraded.mjs";

const args = process.argv.slice(2);
const convergenceOnly = args.includes("--adapter-convergence");
const [wasmPath] = args.filter((arg) => !arg.startsWith("--"));

// The header's table promises 2 for "could not run". Every load below — the file, the
// adapters, `createMotor` — can throw, and an uncaught one is exit 1 with a stack: the
// documented 2 is what a gate row reads to tell "the arm could not run" from "the arm ran
// and the SDK is wrong" (m88). `finish()` sets `process.exitCode` rather than calling
// `process.exit`, so stdout drains first (m89).
try {
  await smoke();
} catch (error) {
  fail(error && error.message ? error.message : String(error));
}

async function smoke() {
  if (convergenceOnly) {
    await runConvergence(wasmPath);
    // `return`, not a fall-through: `finish()` now sets `process.exitCode` instead of
    // calling `process.exit` (m89), so nothing stops the walk into the full smoke.
    finish();
    return;
  }

  if (!wasmPath) fail("usage: sdk-smoke.mjs <wasm> | --adapter-convergence");

  // C3 is append-only, never renumbered: the numbers are the contract's, so they are
  // restated as literals here and compared (the sections' own tables are keyed by those
  // literals, so a renumbering would otherwise be agreed with by both sides at once).
  const idProblems = columnIdProblems(ColumnId);
  check("the SDK's ColumnId spells the contract's ids, renumbered never", idProblems.length === 0, idProblems.join("; "));

  const bytes = await readFile(wasmPath);

  // C16: options this phase accept only {} and { exec: "auto" }; anything else is refused
  // before the module is even asked to load.
  await createMotor(bytes, {});
  await createMotor(bytes, { exec: "auto" });
  check("an unknown options key is refused", await refusedWith(InvalidOptionsError, () => createMotor(bytes, { bogus: true })));
  check('options.exec other than "auto" is refused', await refusedWith(InvalidOptionsError, () => createMotor(bytes, { exec: "gpu" })));

  const motor = await createMotor(bytes);
  if (!motor.available) fail("the module did not load, so nothing below could be measured against it");

  const ctx = { bytes, motor };
  await runBuildSection(ctx);
  await runLayoutSection(ctx);
  await runPostSection(ctx);
  await runAnalysisSection(ctx);
  await runTransportSection(ctx);
  await runForceSection(ctx);
  await runDegradedSection(ctx);
  finish();
}
