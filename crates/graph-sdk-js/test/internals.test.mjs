// The SDK's internal seams, over the findings whose subject is a helper rather than a `Motor`
// method: the framed-buffer arithmetic, the staging buffer's lifetime, the registry scan's
// error class, the registry map that used to escape by reference, and the two package-level
// promises (the `exports` map resolving, and the ABI version test pinning a literal).
//
// Run: node --test --experimental-strip-types crates/graph-sdk-js/test/

import assert from "node:assert/strict";
import { test } from "node:test";

import { AbiContractError, AnalysisRefusedError, BuildRefusedError, PostRefusedError, RunRefusedError } from "../src/errors.ts";
import { frame, readRegistry } from "../src/calls.ts";
import { Registries } from "../src/registries.ts";
import { buildStaged } from "../src/staging.ts";
import { ABI_VERSION } from "../src/wasm.ts";
import { mergeParams } from "../src/force-params.ts";
import { ColumnId } from "../src/types.ts";
import { ColumnViews, columnApplies } from "../src/views.ts";

/** A `WebAssembly.Memory`-shaped stand-in with `len` bytes of room and a framed buffer at
 *  `ptr`. Only what these helpers actually read is provided, and the memory never grows. */
function stubMemory(ptr, declared) {
  const buffer = new ArrayBuffer(64);
  if (declared !== undefined) new DataView(buffer).setUint32(ptr, declared, true);
  return { buffer };
}

const exportsAt = (ptr, declared) => ({
  memory: stubMemory(ptr, declared),
  gm_alloc: () => 8,
  gm_free: () => {},
  gm_build: () => 1,
  gm_build_contract: () => 1,
  gm_last_error: () => 0,
});

// --- m6: a framed buffer's own arithmetic ------------------------------------------------

test("m6: a frame at 0 is refused rather than read as an empty length", () => {
  assert.throws(() => frame(exportsAt(0, 0), 0), (e) => e instanceof AbiContractError && /does not fit/.test(e.message));
});

test("m6: a frame whose header runs past the end of memory is refused", () => {
  assert.throws(() => frame(exportsAt(0, 0), 62), (e) => e instanceof AbiContractError && /does not fit/.test(e.message));
});

test("m6: a frame whose declared length runs past the end is refused, naming both", () => {
  const exports = exportsAt(8, 1000);
  assert.throws(
    () => frame(exports, 8),
    (e) => e instanceof AbiContractError && /declares 1000 bytes/.test(e.message) && /past the end of 64/.test(e.message),
  );
});

test("m6: a legal frame is copied out, and the copy does not alias linear memory", () => {
  const exports = exportsAt(8, 3);
  new Uint8Array(exports.memory.buffer, 12, 3).set([1, 2, 3]);
  const bytes = frame(exports, 8);
  assert.deepEqual([...bytes], [1, 2, 3]);
  new Uint8Array(exports.memory.buffer, 12, 3).set([9, 9, 9]);
  assert.deepEqual([...bytes], [1, 2, 3], "the frame aliased the module's own buffer");
});

// --- m7: a registry scan names its own id space ------------------------------------------

/** A module whose registry count is 1 and whose id export answers `0` (the refusal). */
function refusingRegistry(countExport, idExport) {
  const exports = { memory: stubMemory(8, 0), [countExport]: () => 1, [idExport]: () => 0, gm_last_error: () => 4 };
  return exports;
}

test("m7: a POST registry scan that refuses is a PostRefusedError, not a RunRefusedError", () => {
  const exports = refusingRegistry("gm_post_count", "gm_post_id");
  assert.throws(
    () => readRegistry(exports, "gm_post_count", "gm_post_id", (m, c) => new PostRefusedError(m, c)),
    (e) => e instanceof PostRefusedError && !(e instanceof RunRefusedError),
  );
});

test("m7: the layout and analysis scans take the class their own caller passes", () => {
  const layout = refusingRegistry("gm_layout_count", "gm_layout_id");
  assert.throws(
    () => readRegistry(layout, "gm_layout_count", "gm_layout_id", (m, c) => new RunRefusedError(m, c)),
    (e) => e instanceof RunRefusedError,
  );
  const analysis = refusingRegistry("gm_analysis_count", "gm_analysis_id");
  assert.throws(
    () => readRegistry(analysis, "gm_analysis_count", "gm_analysis_id", (m, c) => new AnalysisRefusedError(m, c)),
    (e) => e instanceof AnalysisRefusedError,
  );
});

// --- m16: the cached registry map does not leave `Registries` ----------------------------

/** A module with two layout ids, framed at 16 and 48. */
function twoLayouts() {
  const buffer = new ArrayBuffer(256);
  const exports = {
    memory: { buffer },
    gm_layout_count: () => 2,
    gm_layout_id: (i) => 16 + i * 32,
    gm_last_error: () => 0,
  };
  const view = new DataView(buffer);
  for (const [i, id] of ["layout.grid", "layout.circle"].entries()) {
    const bytes = new TextEncoder().encode(id);
    view.setUint32(16 + i * 32, bytes.length, true);
    new Uint8Array(buffer, 16 + i * 32 + 4, bytes.length).set(bytes);
  }
  return exports;
}

test("m16: a registry hands out ids, never the mutable map behind them", () => {
  const registries = new Registries();
  const ids = registries.layouts(twoLayouts());
  assert.ok(Array.isArray(ids), "the id -> index map escaped, and a Map is mutable");
  assert.deepEqual([...ids], ["layout.grid", "layout.circle"]);
  assert.equal(registries.layoutIndex(twoLayouts(), "layout.grid"), 0);
  assert.equal(registries.layoutIndex(twoLayouts(), "layout.circle"), 1);
  assert.throws(() => registries.layoutIndex(twoLayouts(), "layout.posted"), (e) => e instanceof RunRefusedError);
});

// --- m4 / m5: the staging buffer's own two guarantees ------------------------------------

/** The minimal `Loaded` `buildStaged` reads: the two views the epoch bump needs. */
function loaded(exports) {
  return { exports, views: new ColumnViews(exports) };
}

const INGEST = JSON.stringify({ version: 1, nodes: [], edges: [] });

test("m4: a non-string document is refused as a document, naming its type", () => {
  const exports = exportsAt(8, 0);
  const spec = {
    buffer: "ingest",
    call: "gm_build",
    refusal: "gm_build refused the ingest buffer",
    refuse: (message, code) => new BuildRefusedError(message, code),
  };
  for (const bad of [undefined, null, 42, {}, []]) {
    assert.throws(
      () => buildStaged(loaded(exports), bad, spec),
      (e) => e instanceof BuildRefusedError && /must be a string/.test(e.message) && !/gm_build refused/.test(e.message),
      `build(${JSON.stringify(bad) ?? String(bad)}) was not refused`,
    );
  }
});

test("m5: a trap inside gm_build surfaces as the trap, never as gm_free's refusal", () => {
  const trapped = {
    memory: stubMemory(8, 0),
    gm_alloc: () => 8,
    gm_free: () => {
      throw new Error("gm_free must not be what the caller learns about");
    },
    gm_build: () => {
      throw new WebAssembly.RuntimeError("unreachable");
    },
    gm_last_error: () => 0,
  };
  const spec = {
    buffer: "ingest",
    call: "gm_build",
    refusal: "gm_build refused the ingest buffer",
    refuse: (message, code) => new BuildRefusedError(message, code),
  };
  assert.throws(
    () => buildStaged(loaded(trapped), INGEST, spec),
    (e) => e.constructor.name === "MotorTrapError" && /gm_build trapped/.test(e.message),
  );
});

test("m5: a refusal still frees the staging buffer exactly once", () => {
  let freed = 0;
  const refusing = {
    memory: stubMemory(8, 0),
    gm_alloc: () => 8,
    gm_free: () => {
      freed += 1;
    },
    gm_build: () => 0,
    gm_last_error: () => 4,
  };
  const spec = {
    buffer: "ingest",
    call: "gm_build",
    refusal: "gm_build refused the ingest buffer",
    refuse: (message, code) => new BuildRefusedError(message, code),
  };
  assert.throws(() => buildStaged(loaded(refusing), INGEST, spec), (e) => e instanceof BuildRefusedError);
  assert.equal(freed, 1, "the staging buffer leaked or was freed twice");
});

// --- m12: the merge, on its own ----------------------------------------------------------

test("m12: an undefined field is omitted, an omitted field is omitted, a real value is kept", () => {
  const current = { charge: -300, theta: 0.9, gravity: 0, alpha_min: 0.001 };
  assert.equal(mergeParams(current, {}).theta, 0.9);
  assert.equal(mergeParams(current, { theta: undefined }).theta, 0.9);
  assert.equal(mergeParams(current, { theta: 1.2 }).theta, 1.2);
  assert.deepEqual(current, { charge: -300, theta: 0.9, gravity: 0, alpha_min: 0.001 }, "the merge mutated the motor's own values");
});

// --- m29: the ABI version is pinned to a literal, not to itself ---------------------------

test("m29: the ABI version this SDK speaks is 1 — a literal, not the constant it imports", () => {
  // `abi-version.test.mjs` imports `ABI_VERSION` and compares the module against it, which
  // pins the constant to itself: setting `wasm.ts`'s `ABI_VERSION` to `2` left both its tests
  // green. This one holds the number in the test instead, so changing the constant turns it
  // red, and the only other place `1` is pinned is the Rust side's
  // `crates/graph-wasm/src/errors/mirrors.rs`.
  assert.equal(ABI_VERSION, 1, "the SDK's ABI revision moved; the Rust mirror must move with it");
});

// --- m30: the package's own `exports` map resolves by name -------------------------------

test("m30: every subpath the docs name resolves through the package's exports map", async () => {
  const root = await import("@graph-motor/sdk-js");
  assert.equal(typeof root.createMotor, "function", "the entry point is not reachable by name");
  const rows = await import("@graph-motor/sdk-js/adapters/rows");
  assert.equal(typeof rows.rowsToIngest, "function");
  const notion = await import("@graph-motor/sdk-js/adapters/notion");
  assert.equal(typeof notion.notionToIngest, "function");
  assert.equal(typeof notion.typeToRole, "object", "m28: the mapping table is not exported by the barrel");
});

// --- m20: the reserved note columns need no arm of their own ------------------------------

test("m20: the reserved note columns are absent for every kind, and dim does not change it", () => {
  for (const nodeKind of ["Point", "Circle", "Box"]) {
    for (const edgeKind of ["Line", "Polyline", "Curve"]) {
      for (const dim of [0, 1]) {
        const where = `${nodeKind}/${edgeKind}/dim=${dim}`;
        assert.equal(columnApplies(nodeKind, edgeKind, ColumnId.NoteCode, dim), false, `NoteCode applied to ${where}`);
        assert.equal(columnApplies(nodeKind, edgeKind, ColumnId.NoteIndex, dim), false, `NoteIndex applied to ${where}`);
        // The control: NODE_X applies everywhere, so the two assertions above are not vacuous.
        assert.equal(columnApplies(nodeKind, edgeKind, ColumnId.NodeX, dim), true, `NODEX absent for ${where}`);
      }
    }
  }
});
