/** The worker's entry: the session behind a message pump. Imported only as a worker. */
import { type HelperStart, type Motor, createMotor } from "../../../../crates/graph-sdk-js/src/index.ts";
import { createForceHost, type ForceHost } from "./liveLoop.ts";
import { UNSOLICITED, isRequest } from "./protocol.ts";
import { createPump } from "./pump.ts";
import { createSession, sha256Hex } from "./session.ts";
import { threadsFor } from "./threads.ts";

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

function spawnHelper(start: HelperStart): void {
  new Worker(new URL("./helper.ts", import.meta.url), { type: "module" }).postMessage(start);
}

/**
 * The threads artifact is staged beside the serial one (`scripts/studio.sh`). Caveat: a threads
 * artifact that fails to load falls back to the serial one without a word to the page; the
 * Workers it started are the evidence (`deploy/perf/live-tick.py` counts them).
 */
async function motorFrom(wasmUrl: string, asked: number | undefined): Promise<Motor> {
  const threads = threadsFor(asked, navigator.hardwareConcurrency, crossOriginIsolated);
  if (threads > 1) {
    const threadsUrl = new URL("graph_wasm_threads.wasm", wasmUrl);
    const motor = await createMotor(threadsUrl, { threads: { helpers: threads - 1, spawn: spawnHelper } });
    if (motor.available) return motor;
  }
  return createMotor(wasmUrl);
}

const scope: unknown = globalThis;
if (isWorkerScope(scope)) {
  // The session is made before the host that could stop its loop, so the notice runs over
  // one cell: a graph replaced mid-settle must not leave the loop stepping a dead session.
  const notice: { host: ForceHost | null } = { host: null };
  const session = createSession({
    motorFrom,
    fetchText,
    digest: sha256Hex,
    now: () => performance.now(),
    onForget: () => notice.host?.forget(),
  });
  const forces = createForceHost(() => session.forces(), {
    schedule: pacedFrame,
    now: () => performance.now(),
    // A frame is unsolicited: it has no request of its own to be the answer to.
    emit: (result, transfer) => scope.postMessage({ seq: UNSOLICITED, body: result }, transfer),
  });
  notice.host = forces;
  const pump = createPump(session, (message, transfer) => scope.postMessage(message, transfer), forces);
  scope.onmessage = (event) => {
    if (isRequest(event.data)) pump(event.data);
  };
}
