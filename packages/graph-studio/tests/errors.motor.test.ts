// Errors reach the user as text, so the text is pinned. A degraded motor (the
// wasm module never loaded) and a refused run are different stories and must not
// read the same; a bare `Error` from anywhere else must still land in the banner
// rather than blanking the page.

import assert from "node:assert/strict";
import { test } from "node:test";

import {
  AnalysisRefusedError,
  BuildRefusedError,
  GraphMotorError,
  PostRefusedError,
  RunRefusedError,
  WasmUnavailableError,
} from "../../../crates/graph-sdk-js/src/index.ts";
import { describeError } from "../src/state/errors.ts";

test("a motor refusal shows its subclass name, its wire code and its message", () => {
  const shown = describeError(new RunRefusedError("gm_run refused (LayoutFailed)", 8));
  assert.equal(shown.title, "RunRefusedError");
  assert.equal(shown.code, "code 8 (LayoutFailed)");
  assert.equal(shown.detail, "gm_run refused (LayoutFailed)");
  assert.equal(shown.hint, "The layout refused to run on this graph. Pick another layout, or load a graph it accepts.");
});

test("a build refusal points at the ingest document, not at the layout", () => {
  const shown = describeError(new BuildRefusedError("gm_build refused the ingest buffer", 4));
  assert.equal(shown.title, "BuildRefusedError");
  assert.equal(shown.code, "code 4 (IngestInvalid)");
  assert.equal(shown.hint, "The ingest document was refused. Check the JSON, or regenerate the synthetic graph.");
});

test("an unavailable motor is reported as degraded, with the reason it carries", () => {
  const shown = describeError(new WasmUnavailableError("wasm module failed to load", new Error("404")));
  assert.equal(shown.title, "WasmUnavailableError");
  assert.equal(shown.code, null);
  assert.equal(shown.detail, "wasm module failed to load — 404");
  assert.equal(shown.hint, "The wasm motor did not load, so no layout can run. Run scripts/studio.sh wasm to build graph_wasm.wasm into app/public/, then reload.");
});

test("an unavailable motor with no reason keeps just its message", () => {
  assert.equal(describeError(new WasmUnavailableError("nope")).detail, "nope");
});

test("a plain Error is shown, not swallowed, and gets no fake code", () => {
  const shown = describeError(new TypeError("x is not a function"));
  assert.equal(shown.title, "TypeError");
  assert.equal(shown.code, null);
  assert.equal(shown.detail, "x is not a function");
  assert.equal(shown.hint, "Unexpected studio error — see the browser console for the stack.");
});

test("a non-Error throw is stringified rather than rendered as [object Object]", () => {
  const shown = describeError("boom");
  assert.equal(shown.title, "Error");
  assert.equal(shown.detail, "boom");
});

test("a post refusal says a pass needs a layout run, which is what NoGeometryYet means", () => {
  const shown = describeError(new PostRefusedError("handle 1 has no successful layout run to draw over", 10));
  assert.equal(shown.title, "PostRefusedError");
  assert.equal(shown.code, "code 10 (NoGeometryYet)");
  assert.match(shown.hint, /finished layout run/);
});

test("an analysis refusal is not confused with a post refusal", () => {
  const shown = describeError(new AnalysisRefusedError("gm_analysis_run refused (IndexOutOfRange)", 12));
  assert.equal(shown.title, "AnalysisRefusedError");
  assert.equal(shown.code, "code 12 (IndexOutOfRange)");
  assert.match(shown.hint, /function of the topology/);
});

test("every GraphMotorError subclass keeps its own name through describeError", () => {
  const error = new BuildRefusedError("x", 1);
  assert.ok(error instanceof GraphMotorError);
  assert.equal(describeError(error).title, "BuildRefusedError");
});
