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
    onFail: (handler) => {
      // An uncaught throw in the worker reaches the page as an `error` event and nowhere
      // else, so this is the only place a worker failure can be named.
      worker.onerror = (event: ErrorEvent) => handler(event.message);
      return () => {
        worker.onerror = null;
      };
    },
  };
}
