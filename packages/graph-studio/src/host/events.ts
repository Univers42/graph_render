/** How the element tells its host something happened (`docs/contract/host-api.md`, Events). */
import type { HostEvents } from "./contract.ts";

function frozen<Value>(value: Value): Value {
  if (typeof value === "object" && value !== null) {
    for (const inner of Object.values(value)) frozen(inner);
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
