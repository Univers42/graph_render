/** How the element tells its host something happened (`docs/contract/host-api.md`, Events). */
import type { HostEvents } from "./contract.ts";

/**
 * Deep-freezes the copy, so no listener can change what the next one reads.
 *
 * WHY a set of what was already walked: the clone keeps whatever cycles the caller's `detail`
 * had, and a depth-first walk without a visited set recurses through one forever — a cyclic
 * detail overflows the stack here, inside the dispatch, and throws into the host's listener.
 */
function frozen<Value>(value: Value, seen: WeakSet<object> = new WeakSet()): Value {
  if (typeof value === "object" && value !== null && !seen.has(value)) {
    seen.add(value);
    for (const inner of Object.values(value)) frozen(inner, seen);
    Object.freeze(value);
  }
  return value;
}

/**
 * Dispatches `name` on the host element.
 *
 * WHY a structured clone and not the caller's object: `detail` is handed to every listener on
 * the page, and a host may post it to a worker or a backend as it is; a copy holds only plain
 * data, and freezing it means no listener can change what the next one reads.
 * WHY `composed`: the element is often inside a shadow root of the host's own, and an event
 * that is not composed stops at that root's boundary, where a `document` listener never sees it.
 */
export function emit<Name extends keyof HostEvents>(target: EventTarget, name: Name, detail: HostEvents[Name]): void {
  target.dispatchEvent(new CustomEvent(name, { detail: frozen(structuredClone(detail)), bubbles: true, composed: true }));
}
