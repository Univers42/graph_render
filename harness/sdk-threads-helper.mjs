// One helper of harness/sdk-threads.mjs: the SDK's serveHelper over the coordinator's shared
// memory. A trap here leaves the coordinator blocked inside its pass, where it runs no event loop
// and never sees this worker's `error` event, so the helper ends the process itself.

import { workerData } from "node:worker_threads";
import { writeSync } from "node:fs";
import { serveHelper } from "../crates/graph-sdk-js/src/index.ts";

try {
  serveHelper(workerData);
} catch (error) {
  writeSync(2, `sdk-threads: a helper trapped (${error.message}); its pass cannot finish\n`);
  process.kill(process.pid, "SIGABRT");
}
