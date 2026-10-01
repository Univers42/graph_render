/** The worker's entry: the session behind a message pump. Imported only as a worker. */
import { createMotor } from "../../../../crates/graph-sdk-js/src/index.ts";
import { createForceHost } from "./liveLoop.ts";
import { UNSOLICITED, isRequest } from "./protocol.ts";
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

/**
 * Ponytail: a worker has no `requestAnimationFrame`, so the loop is paced by a timer. This is
 * a fixed period rather than the page's vsync, so a busy main thread delays a frame rather
 * than skipping one, and the loop's own budget drops ticks instead. Failing input: a tab in
 * the background has its timers clamped to about 1 Hz, so the settle crawls there.
 * Direction: 16 ms, the period a 60 Hz display would give. Escape hatch: the `now()` the loop
 * budgets against is `performance.now()`, so a clamped timer shows up as a long frame.
 */
const FRAME_MS = 16;

function pacedFrame(run: () => void): () => void {
  const timer = setTimeout(run, FRAME_MS);
  return () => clearTimeout(timer);
}

const scope: unknown = globalThis;
if (isWorkerScope(scope)) {
  const session = createSession({
    motorFrom: (wasmUrl) => createMotor(wasmUrl),
    fetchText,
    digest: sha256Hex,
    now: () => performance.now(),
  });
  const forces = createForceHost(() => session.forces(), {
    schedule: pacedFrame,
    now: () => performance.now(),
    // A frame is unsolicited: it has no request of its own to be the answer to.
    emit: (result, transfer) => scope.postMessage({ seq: UNSOLICITED, body: result }, transfer),
  });
  const pump = createPump(session, (message, transfer) => scope.postMessage(message, transfer), forces);
  scope.onmessage = (event) => {
    if (isRequest(event.data)) pump(event.data);
  };
}
