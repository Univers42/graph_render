// The console's line shows what Tab would add, before Tab is pressed. The field is
// rendered, not driven, here: the text of the ghost is decided by console/complete.ts,
// which the unit tests read directly; what is pinned here is where it is shown.
import assert from "node:assert/strict";
import { test } from "node:test";
import { createElement } from "react";

import { Console } from "../../src/ui/Console.tsx";
import { DRAWN, markup, studioWith } from "./desk.ts";

function console_(): string {
  const { studio } = studioWith(DRAWN);
  return markup(createElement(Console, { studio, state: DRAWN, onClose: () => undefined }));
}

test("the line shows what Tab would add, over the field being typed", () => {
  const html = console_();
  const ghost = html.match(/<span class="gs-ghost" aria-hidden="true" style="([^"]*)">([^<]*)<\/span>/);
  assert.ok(ghost !== null, "the ghost is rendered, empty or not");
  assert.equal(ghost[2] ?? "", "", "nothing is offered before a word is typed");
  assert.match(ghost[1] ?? "", /left:calc\(7px \+ 0ch\)/, "it stands where the text ends");
  assert.ok(html.indexOf("</input>") < html.indexOf('class="gs-ghost"'), "it paints over the input");
});
