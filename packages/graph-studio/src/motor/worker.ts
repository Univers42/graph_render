/** The worker's entry: the session behind a message pump. Imported only as a worker. */
import { createMotor } from "../../../../crates/graph-sdk-js/src/index.ts";
import { isRequest } from "./protocol.ts";
import { createPump } from "./pump.ts";
import { createSession, sha256Hex } from "./session.ts";

interface WorkerScope {
  onmessage: ((event: MessageEvent<unknown>) => void) | null;
  postMessage(message: unknown, transfer: Transferable[]): void;
}

function isWorkerScope(scope: unknown): scope is WorkerScope {
  return typeof scope === "object" && scope !== null && "postMessage" in scope && !("document" in scope);
}

async function fetchText(url: string): Promise<string> {
  const response = await fetch(url);
  if (!response.ok) throw new Error(`${url}: HTTP ${response.status}`);
  return response.text();
}

const scope: unknown = globalThis;
if (isWorkerScope(scope)) {
  const session = createSession({
    motorFrom: (wasmUrl) => createMotor(wasmUrl),
    fetchText,
    digest: sha256Hex,
    now: () => performance.now(),
  });
  const pump = createPump(session, (message, transfer) => scope.postMessage(message, transfer));
  scope.onmessage = (event) => {
    if (isRequest(event.data)) pump(event.data);
  };
}
