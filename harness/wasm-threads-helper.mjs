// One helper of harness/wasm-threads.mjs: a second instance of the threads artifact over the
// coordinator's shared memory, given its own stack and TLS block, parked in gm_thread_serve
// until gm_thread_close.
//
// A trap here (a Rust panic is an abort on wasm32) leaves the coordinator blocked inside its
// pass, and a blocked thread runs no event loop, so it would never see this worker's `error`
// event. So the helper ends the whole process itself: never a hang, never exit 0, 1 or 2.

import { workerData } from "node:worker_threads";
import { writeSync } from "node:fs";

const { module, memory, stackTop, tls } = workerData;
try {
  const { exports } = new WebAssembly.Instance(module, { env: { memory } });
  exports.__stack_pointer.value = stackTop;
  exports.__wasm_init_tls(tls);
  exports.gm_thread_serve();
} catch (error) {
  writeSync(2, `wasm-threads: a helper trapped (${error.message}); its pass cannot finish\n`);
  process.kill(process.pid, "SIGABRT");
}
