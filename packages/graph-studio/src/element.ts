/**
 * <graph-studio>: the whole studio as one custom element, so a host is a tag and two
 * attributes. Everything it draws is inside its own shadow root: the host page's styles
 * do not reach the panels, and the panels' styles do not reach the host page.
 *
 *   <graph-studio wasm="/graph_wasm.wasm" fixtures="/fixtures/" keys="page" remember></graph-studio>
 *
 * `keys="page"` listens for shortcuts on the window; without it only while the element or
 * something in it has the focus, which is what a page that embeds it next to its own
 * inputs wants. `remember` reopens the last graph the page drew; without it the element
 * draws nothing until its host calls `loadGraph` (`docs/contract/host-api.md`).
 */
import type { View } from "../../graph-render/src/view.ts";
import { HOST_API, type GraphStudioElement, type LoadResult, type Resolve } from "./host/contract.ts";
import { SILENCE_MS } from "./motor/watchdog.ts";
import { type Mounted, type StudioElementOptions, mount, unmount } from "./mount.ts";
import type { Studio } from "./studio/studio.ts";

/** The host reads `?backend=` with this, so it never imports the renderer itself. */
export { backendOf } from "../../graph-render/src/view.ts";
export type { StudioElementOptions } from "./mount.ts";
/** The host API, from the one file a host may import (`app/eslint.config.js`, INSIDE). */
export { HOST_API, OPEN_VIAS } from "./host/contract.ts";
export type {
  GraphStudioElement, GraphStudioHost, HostEvents, LoadResult, NodePreview, OpenVia, Resolve,
} from "./host/contract.ts";

const NONE: readonly string[] = Object.freeze([]);

/** A type guard, not a cast: the host may hand anything to `resolve`, and only a function is kept. */
function isResolve(value: unknown): value is Resolve {
  return typeof value === "function";
}

function notConnected(): Promise<never> {
  return Promise.reject(new DOMException("<graph-studio> is not in a document", "InvalidStateError"));
}

/**
 * The element. Each `defineGraphStudio` call makes its own subclass, which only supplies the
 * options it was given: a custom element's constructor takes no arguments.
 */
class GraphStudio extends HTMLElement implements GraphStudioElement {
  #mounted: Mounted | null = null;
  #resolve: Resolve | null = null;
  readonly hostApi = HOST_API;

  /**
   * WHY: a host that set `resolve` before the element was defined set a plain property on the
   * element, which hides this class's accessor; it is taken back through the setter (verdict 12).
   */
  constructor() {
    super();
    if (!Object.hasOwn(this, "resolve")) return;
    const early: unknown = Reflect.get(this, "resolve");
    Reflect.deleteProperty(this, "resolve");
    this.resolve = isResolve(early) ? early : null;
  }

  protected get options(): StudioElementOptions {
    return {};
  }

  get studio(): Studio | null {
    return this.#mounted?.studio ?? null;
  }

  get view(): View | null {
    return this.#mounted?.view ?? null;
  }

  get resolve(): Resolve | null {
    return this.#resolve;
  }

  set resolve(value: Resolve | null) {
    this.#resolve = isResolve(value) ? value : null;
  }

  get selectedIds(): readonly string[] {
    return this.#mounted?.verbs.selectedIds() ?? NONE;
  }

  loadGraph(doc: object): Promise<LoadResult> {
    return this.#mounted === null ? notConnected() : this.#mounted.verbs.loadGraph(doc);
  }

  focusNode(id: string): Promise<boolean> {
    return this.#mounted?.verbs.focusNode(id) ?? Promise.resolve(false);
  }

  selectNodes(ids: readonly string[]): Promise<boolean> {
    return this.#mounted?.verbs.selectNodes(ids) ?? Promise.resolve(false);
  }

  invalidate(id: string): void {
    if (typeof id === "string") this.#mounted?.previews.invalidate(id);
  }

  stopMotor(): void {
    // `close`, not `destroy`: the studio and its chrome stay, so the page reads as a studio
    // that lost its motor rather than one that was taken down.
    this.#mounted?.client.close();
  }

  get watchdogBoundMs(): number {
    return SILENCE_MS;
  }

  connectedCallback(): void {
    this.#mounted ??= mount(this, this.options, () => this.#resolve);
  }

  disconnectedCallback(): void {
    unmount(this.#mounted);
    this.#mounted = null;
  }
}

/** Registers the element once; a second call, or a tag already taken, changes nothing. */
export function defineGraphStudio(options: StudioElementOptions = {}, tag = "graph-studio"): void {
  if (customElements.get(tag) !== undefined) return;
  customElements.define(tag, class extends GraphStudio {
    protected override get options(): StudioElementOptions {
      return options;
    }
  });
}
