/** One helper of the motor's thread pool: an instance of the threads artifact, parked until the pool closes. Started only by `worker.ts`. */
import { type HelperStart, serveHelper } from "../../../../crates/graph-sdk-js/src/index.ts";

function isHelperStart(value: unknown): value is HelperStart {
  if (typeof value !== "object" || value === null) return false;
  if (!("module" in value && "memory" in value && "stackTop" in value && "tls" in value)) return false;
  return value.module instanceof WebAssembly.Module && value.memory instanceof WebAssembly.Memory
    && typeof value.stackTop === "number" && typeof value.tls === "number";
}

// One message, then the serve loop holds this thread for the life of the motor worker.
globalThis.addEventListener("message", (event: MessageEvent<unknown>) => {
  if (isHelperStart(event.data)) serveHelper(event.data);
});
