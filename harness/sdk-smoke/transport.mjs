// Tamper refusal and release semantics on the last layout run.

import {
  ColumnId,
  TamperedGeometryError,
} from "../../crates/graph-sdk-js/src/index.ts";
import { check } from "./lib.mjs";

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
  x[0] = Number.NaN;
  check("writing through a column view does not itself move the epoch", motor.epoch === epochBeforeTamper);
  let tamperRefused = false;
  try {
    motor.toJSON(handle);
  } catch (error) {
    tamperRefused = error instanceof TamperedGeometryError;
  }
  check("a NaN written through a column view refuses toJSON (D9)", tamperRefused);

  const epochBeforeRelease = motor.epoch;
  motor.release(handle);
  check("release moves the epoch forward", motor.epoch > epochBeforeRelease);
  let releasedRefuses = false;
  try {
    motor.nodeCount(handle);
  } catch {
    releasedRefuses = true;
  }
  check("a released handle is refused, not silently answered (C6)", releasedRefuses);
}
}
