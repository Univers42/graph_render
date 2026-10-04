/**
 * Row `host-api-escape` (`docs/contract/host-api.md`, verdict 5): what a host or a document says
 * about a node reaches the page as text. Hostile strings go through the preview body, the
 * fallback made of the node's own fields, and the inspector, and no tag, handler or link may come
 * out. `HOST_API_ESCAPE_BREAK=1` renders the body through `dangerouslySetInnerHTML` instead
 * (`raw-html.tsx`), and this file must then fail.
 */
import assert from "node:assert/strict";
import { test } from "node:test";
import { createElement } from "react";

import type { NodePreview } from "../../src/host/contract.ts";
import { metaOf } from "../../src/source/meta.ts";
import { previewOrOwn } from "../../src/ui/HoverCard.tsx";
import { Inspector } from "../../src/ui/Inspector.tsx";
import { PreviewBody } from "../../src/ui/Preview.tsx";
import { node } from "../support.ts";
import { DRAWN, fakeView, markup, studioWith } from "./desk.ts";
import { RawPreview } from "./raw-html.tsx";

const Body = process.env.HOST_API_ESCAPE_BREAK === "1" ? RawPreview : PreviewBody;

const HOSTILE = [
  "<img src=x onerror=alert(1)>",
  "javascript:alert(1)",
  "data:text/html,<script>alert(1)</script>",
  '"><script>alert(1)</script>',
];

/**
 * The tags React wrote. Escaped text holds no `<`, so every `<` in the markup opens a real tag,
 * and a hostile string that shows up in one was parsed rather than escaped.
 */
function tagsOf(html: string): string[] {
  return html.match(/<[^>]*>/g) ?? [];
}

function assertInert(html: string, what: string): void {
  for (const tag of tagsOf(html)) {
    assert.doesNotMatch(tag, /^<\/?(img|script|a|iframe|object)\b/i, `${what}: ${tag}`);
    assert.doesNotMatch(tag, /\son\w+\s*=/i, `${what}: ${tag}`);
    assert.doesNotMatch(tag, /\b(href|src)\s*=/i, `${what}: ${tag}`);
  }
}

test("a host's preview renders as text, whatever its title, text and icon hold", () => {
  for (const hostile of HOSTILE) {
    const preview: NodePreview = { title: hostile, text: hostile, icon: hostile };
    const html = markup(createElement(Body, { preview }));
    assertInert(html, hostile);
    assert.ok(html.includes("&lt;") || !hostile.includes("<"), `${hostile} is shown, escaped`);
  }
});

const META = metaOf(HOSTILE.map((hostile, at) => node(`n${at}`, { label: hostile, path: hostile })), ["n0", "n1", "n2", "n3"], {
  source: Uint32Array.of(0), target: Uint32Array.of(1),
});

test("with no preview the card shows the node's own label and path, as text too", () => {
  for (let at = 0; at < HOSTILE.length; at += 1) {
    assertInert(markup(createElement(Body, { preview: previewOrOwn(META, at, null) })), `node ${at}`);
  }
});

test("the inspector shows a hostile node and a hostile preview as text", () => {
  const state = { ...DRAWN, meta: META, selected: 0, selection: [0] };
  const { studio } = studioWith(state);
  const preview: NodePreview = { title: HOSTILE[3] ?? "", text: HOSTILE[0] ?? "" };
  const html = markup(createElement(Inspector, {
    studio, meta: META, selected: 0, analysis: null, selection: [0], view: fakeView(), preview,
  }));
  assertInert(html, "inspector");
  assert.ok(html.includes("&lt;img"), "the hostile label is there, escaped");
});
