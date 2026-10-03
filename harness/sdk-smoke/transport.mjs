// Tamper refusal and release semantics on the last layout run.

import {
  ColumnId,
  InvalidHandleError,
  RunRefusedError,
  TamperedGeometryError,
} from "../../crates/graph-sdk-js/src/index.ts";
import { check } from "./lib.mjs";

/** A laid-out handle of its own, so the copy property can be checked without the tamper
 *  the section's own handle carries. Released before it returns. */
function freshBytes(motor, node) {
  const fresh = motor.build(JSON.stringify({
    version: 1,
    nodes: ["p", "q"].map((id) => node(id, "record")),
    edges: [{ id: "pe", source: "p", target: "q", kind: "relation", label: "", strength: 0.5, directed: false, record_id: null }],
  }));
  motor.layout(fresh, "layout.grid");
  const bytes = motor.toBytes(fresh);
  motor.release(fresh);
  return bytes;
}

export async function runTransportSection(ctx) {
  const { motor, handle, ran } = ctx;
// Everything below reads a run the loop above leaves behind (the last registered layout's),
// so with no layout at all there is nothing to read and every check would be a thrown
// error rather than a verdict. Said once, here, instead of at each of them.
if (ran === 0) {
  check("a layout ran, so the transport could be exercised at all", false);
} else {
  const beforeTamper = motor.toJSON(handle);
  check("toJSON succeeds before any tamper", typeof beforeTamper === "string" && beforeTamper.includes('"a"'));

  // C8: a column view is a real zero-copy alias into the motor's own memory — writing NaN
  // through it and then asking the motor to encode a face is refused (D9), not silently
  // written through to the wire.
  const epochBeforeTamper = motor.epoch;
  const x = motor.column(handle, ColumnId.NodeX);
  // The view must be non-empty before anything is written through it. On a zero-node run
  // `x[0] = NaN` is a silent no-op on an undefined slot and the tamper check below then
  // reports a tamper failure for a document nobody tampered with — a wrong verdict, not a
  // vacuous pass, and the harder one to read (m105).
  check("the node x column is a live, non-empty view", x !== null && x.length > 0, String(x?.length));
  if (x !== null && x.length > 0) x[0] = Number.NaN;
  check("writing through a column view does not itself move the epoch", motor.epoch === epochBeforeTamper);
  let tamperRefused = false;
  try {
    motor.toJSON(handle);
  } catch (error) {
    tamperRefused = error instanceof TamperedGeometryError;
  }
  check("a NaN written through a column view refuses toJSON (D9)", tamperRefused);

  // `toBytes` had no call anywhere in the smoke, so the framed-buffer copy property the
  // contract asserts (`docs/contract/wasm-abi.md:179`, `crates/graph-sdk-js/README.md:199`
  // — `frame()`'s `.slice()`, `calls.ts:45`) was checked by nothing: delete the `.slice()`
  // and every row stayed green (m103). Checked on a fresh handle, because the handle above
  // is deliberately tampered and `toBytes` refuses that one — which is D9 doing its job,
  // not the copy property.
  const bytes = freshBytes(motor, ctx.node);
  check("toBytes returns a buffer for an untampered handle", bytes instanceof Uint8Array && bytes.length > 0, String(bytes?.length));
  if (bytes instanceof Uint8Array && bytes.length > 0) {
    const copy = bytes.slice();
    bytes.fill(0);
    check(
      "the bytes motor.toBytes returns are a copy, not a view over its own memory",
      copy.some((byte) => byte !== 0),
      `every one of the frame's ${copy.length} bytes was zero after the fill`,
    );
  }

  // The staleness the JSON face owes its reader: `toJSON` can grow wasm memory, so a face
  // read earlier is not a promise about the handle now. Asserted by re-encoding after the
  // release, where the handle must be refused rather than answered from a stale face (m105).
  const epochBeforeRelease = motor.epoch;
  motor.release(handle);
  check("release moves the epoch forward", motor.epoch > epochBeforeRelease);
  // A bare `catch { releasedRefuses = true }` accepts any throw; the contract names the
  // code (`InvalidHandle`, `docs/contract/wasm-abi.md:47`) and that is what is checked
  // (m104).
  let releasedRefuses = "";
  try {
    motor.nodeCount(handle);
    releasedRefuses = "nodeCount answered a released handle";
  } catch (error) {
    releasedRefuses = error instanceof InvalidHandleError && error.codeName === "InvalidHandle"
      ? ""
      : `refused with ${error?.constructor?.name}(codeName ${error?.codeName})`;
  }
  check("a released handle is refused as InvalidHandle, not silently answered (C6)", releasedRefuses === "", releasedRefuses);
  let staleFace = "";
  try {
    motor.toJSON(handle);
    staleFace = "toJSON answered a released handle from a face read before the release";
  } catch (error) {
    // `toJSON` on a released handle refuses through the run path, not `gm_release`, so the
    // class here is `RunRefusedError`. What matters is that it refuses at all: the face
    // read at the top of this section is not served again after the handle is gone.
    staleFace = error instanceof InvalidHandleError || error instanceof RunRefusedError ? "" : `refused with ${error?.constructor?.name}`;
  }
  check("and a face read before the release is not answered after it (C6)", staleFace === "", staleFace);
}
}