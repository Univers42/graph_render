/** A real worker behind the port the client speaks. */
import { type Port, isResult } from "./protocol.ts";

export function workerPort(worker: Worker): Port {
  return {
    send: (message) => worker.postMessage(message),
    listen: (handler) => {
      worker.onmessage = (event: MessageEvent<unknown>) => {
        if (isResult(event.data)) handler(event.data);
      };
    },
    close: () => worker.terminate(),
  };
}
