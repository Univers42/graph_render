/**
 * The embed example (`app/embed.html`): a host page with no React of its own that drives
 * `<graph-studio>` through the host API alone (`docs/contract/host-api.md`). It is also the page
 * `scripts/studio-embed.sh` drives, so everything it heard is kept on `window.__embed`.
 *
 * WHY each choice:
 *   - The element sits in a shadow root of the page's own. A `document` listener then hears its
 *     events only because they are `composed` (verdict 10); next to a light-DOM element the
 *     gate would hear a non-composed event too, and prove nothing.
 *   - `resolve` is set on the element before `defineGraphStudio` runs, the order a host that
 *     loads the studio late meets (verdict 12). The element takes it back when it upgrades.
 *   - The size goes through the CSSOM, never a style attribute: the page is served under the
 *     CSP a host is asked for (verdict 13), and that CSP refuses inline styles.
 */
import { HOST_API, type GraphStudioElement, type NodePreview, defineGraphStudio } from "../../packages/graph-studio/src/element.ts";

const EVENTS = ["graph-load", "node-select", "node-open", "node-hover", "graph-error"] as const;
const FIXTURE = "fixtures/force/clustered.json";

/** One event as the page's `document` listener heard it. */
interface Heard {
  readonly type: string;
  readonly detail: unknown;
  readonly bubbles: boolean;
  readonly composed: boolean;
  readonly frozen: boolean;
}

/** What the page heard and did, for the gate (`deploy/nav/embedpage.py`). */
interface EmbedState {
  readonly heard: Heard[];
  /** Every id the element asked `resolve` about, in order. */
  readonly asked: string[];
  /** How many events had been heard when the page called `loadGraph`; -1 before it did. */
  loadCalledAt: number;
  /** `pending`, `loaded <nodes>`, `refused <error name>` or `failed <message>`. */
  state: string;
}

function deepFrozen(value: unknown): boolean {
  if (typeof value !== "object" || value === null) return true;
  return Object.isFrozen(value) && Object.values(value).every(deepFrozen);
}

function openedId(detail: unknown): string {
  return typeof detail === "object" && detail !== null && "id" in detail && typeof detail.id === "string" ? detail.id : "?";
}

function listen(embed: EmbedState, opened: HTMLElement): void {
  for (const type of EVENTS) {
    document.addEventListener(type, (event) => {
      if (!(event instanceof CustomEvent)) return;
      const detail: unknown = event.detail;
      embed.heard.push({ type, detail, bubbles: event.bubbles, composed: event.composed, frozen: deepFrozen(detail) });
      if (type === "node-open") opened.textContent = openedId(detail);
    });
  }
}

/** The host's own record of a node: here, made up from its id. */
function answer(embed: EmbedState, id: string): Promise<NodePreview> {
  embed.asked.push(id);
  return Promise.resolve({ title: `Host record ${id}`, text: `What the host page knows about ${id}.`, icon: "#" });
}

function studioIn(frame: HTMLElement, embed: EmbedState): HTMLElement {
  const element = document.createElement("graph-studio");
  element.setAttribute("wasm", "graph_wasm.wasm");
  element.setAttribute("fixtures", "fixtures/");
  element.style.width = "100%";
  element.style.height = "100%";
  // Before the element is defined, so a plain property: what verdict 12 says is kept.
  Reflect.set(element, "resolve", (id: string) => answer(embed, id));
  frame.attachShadow({ mode: "open" }).append(element);
  return element;
}

/** A type guard, not a cast: a host checks the version it was written against. */
function isStudio(element: Element): element is GraphStudioElement {
  return "hostApi" in element && element.hostApi === HOST_API && "loadGraph" in element;
}

async function fixture(): Promise<object> {
  const response = await fetch(FIXTURE);
  if (!response.ok) throw new Error(`${FIXTURE}: HTTP ${response.status}`);
  const doc: unknown = await response.json();
  if (typeof doc !== "object" || doc === null) throw new Error(`${FIXTURE} is not a JSON object`);
  return doc;
}

async function main(embed: EmbedState): Promise<void> {
  const frame = document.getElementById("frame");
  const opened = document.getElementById("opened");
  if (frame === null || opened === null) throw new Error("embed.html has lost its #frame or #opened");
  listen(embed, opened);
  const element = studioIn(frame, embed);
  defineGraphStudio();
  await customElements.whenDefined("graph-studio");
  if (!isStudio(element)) throw new Error(`<graph-studio> does not speak host API ${HOST_API}`);
  const doc = await fixture();
  embed.loadCalledAt = embed.heard.length;
  try {
    embed.state = `loaded ${(await element.loadGraph(doc)).nodes}`;
  } catch (error) {
    embed.state = `refused ${error instanceof Error ? error.name : "a non-Error"}`;
  }
}

const embed: EmbedState = { heard: [], asked: [], loadCalledAt: -1, state: "pending" };
Reflect.set(window, "__embed", embed);
main(embed).catch((error: unknown) => {
  embed.state = `failed ${error instanceof Error ? error.message : "a non-Error"}`;
});
