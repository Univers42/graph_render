/**
 * Row `host-api-types` (`docs/contract/host-api.md`, verdicts 1 and 9). The first half is checked
 * by `tsc`, which compiles this file with the rest of `tests/`: the element is an `HTMLElement`,
 * and the platform's `focus(options)` still works on it. Its break, `tests/breaks/focus-name.ts`,
 * puts the name `focus` back and must fail with TS2430 (`scripts/studio.sh`, `types`).
 * The second half reads the contract: what is outside the v1 promise says so.
 */
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

import type { GraphStudioElement, GraphStudioHost } from "../src/host/contract.ts";

/** Never called: it exists for `tsc`. */
export function stillAnElement(element: GraphStudioElement): GraphStudioHost {
  const plain: HTMLElement = element;
  plain.focus({ preventScroll: true });
  element.focus({ preventScroll: true });
  return element;
}

const INTERNAL = ["studio", "view", "stopMotor", "watchdogBoundMs"];

/** The members of `GraphStudioElement` whose doc comment opens with `@internal`. */
function internalMembers(source: string): string[] {
  const body = source.split("export interface GraphStudioElement")[1]?.split("\n}\n")[0] ?? "";
  const marked = body.matchAll(/\/\*\*\s*(?:\*\s*)?@internal[\s\S]*?\*\/\s*(?:readonly\s+)?(\w+)/g);
  return [...marked].map((match) => match[1] ?? "");
}

const CONTRACT = readFileSync(new URL("../src/host/contract.ts", import.meta.url), "utf8");

test("studio, view, stopMotor and watchdogBoundMs are marked @internal, and nothing in the host face is", () => {
  assert.deepEqual(internalMembers(CONTRACT), INTERNAL);
  const host = CONTRACT.split("export interface GraphStudioHost")[1]?.split("\n}\n")[0] ?? "";
  assert.doesNotMatch(host, /@internal/);
  for (const name of ["loadGraph", "focusNode", "selectNodes", "selectedIds", "resolve", "invalidate", "hostApi"]) {
    assert.match(host, new RegExp(`\\b${name}\\b`), name);
  }
});

test("negative control: with the markers taken out the same reading finds none", () => {
  assert.deepEqual(internalMembers(CONTRACT.replaceAll("@internal", "")), []);
});
