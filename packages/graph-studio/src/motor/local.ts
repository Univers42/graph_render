/**
 * The motor on the page's own thread, behind the same port as the worker. Every layout
 * then freezes the page for as long as it runs: this exists to measure that (the perf
 * gate's negative control) and for a host that forbids workers. Nothing can be stopped.
 */
import { assembleColumns, createMotor } from "../../../../crates/graph-sdk-js/src/index.ts";
import type { Envelope, Port, Result } from "./protocol.ts";
import { createPump } from "./pump.ts";
import { createSession, sha256Hex } from "./session.ts";

async function fetchText(url: string): Promise<string> {
  const response = await fetch(url);
  if (!response.ok) throw new Error(`${url}: HTTP ${response.status}`);
  return response.text();
}

export function spawnLocal(): Port {
  let handler: ((message: Envelope<Result>) => void) | null = null;
  const session = createSession({
    motorFrom: (wasmUrl) => createMotor(wasmUrl),
    fetchText,
    digest: sha256Hex,
    assemble: assembleColumns,
    now: () => performance.now(),
  });
  const pump = createPump(session, (message) => handler?.(message));
  return {
    send: pump,
    listen: (next) => {
      handler = next;
    },
    close: () => {
      handler = null;
    },
  };
}
