/**
 * The break of row `host-api-types` (verdict 1): the contract with the name `focus` put back.
 * It must NOT compile: `HTMLElement.focus(options?)` is the platform's, and `focus(id)` does not
 * extend it, so `tsc -p tests/breaks` fails with TS2430 and nothing else (`scripts/studio.sh`).
 */
import type { GraphStudioHost } from "../../src/host/contract.ts";

export interface FocusNamedElement extends HTMLElement, Omit<GraphStudioHost, "focusNode"> {
  focus(id: string): boolean;
}
