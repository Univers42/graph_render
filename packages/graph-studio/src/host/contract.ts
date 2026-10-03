/**
 * The host API of `<graph-studio>` (`docs/contract/host-api.md`): what a page that embeds the
 * element may call, read and listen to. Every id that crosses it is the host's own string; the
 * dense index never leaves the element.
 *
 * WHY no `focus`, `select` or `load`: `HTMLElement.focus(options?)` is the platform's, and an
 * interface that redeclares it as `focus(id)` does not compile (TS2430), so every verb carries
 * the noun it acts on (verdict condition 1).
 */
import type { View } from "../../../graph-render/src/view.ts";
import type { ShownError } from "../state/errors.ts";
import type { Studio } from "../studio/studio.ts";

/** Bumped on a breaking change only; an addition is detected with `"name" in el`. */
export const HOST_API = 1;

/** How the user asked to open a node. The element opens nothing itself. */
export const OPEN_VIAS = ["dblclick", "enter", "inspector"] as const;
export type OpenVia = (typeof OPEN_VIAS)[number];

/** What a host says about one of its nodes, for the hover card and the inspector. */
export interface NodePreview {
  /** At most 256 code points; a longer one is cut when it enters the cache. */
  readonly title: string;
  /** At most 4096 code points. */
  readonly text?: string;
  /** Plain text of at most 16 code points: never fetched, never an image. */
  readonly icon?: string;
}

/** The host's lookup; `signal` aborts when the pointer or the selection moves on. */
export type Resolve = (id: string, signal: AbortSignal) => Promise<NodePreview>;

export interface LoadResult {
  readonly nodes: number;
  readonly edges: number;
  /** What the normaliser filled in or dropped, the first few and a count of the rest. */
  readonly notes: readonly string[];
}

/** Every event the element sends, by name, with the `detail` it carries. */
export interface HostEvents {
  readonly "graph-load": LoadResult;
  readonly "node-select": { readonly ids: readonly string[] };
  readonly "node-open": { readonly id: string; readonly via: OpenVia };
  readonly "node-hover": { readonly id: string | null };
  readonly "graph-error": { readonly error: string; readonly message: string };
}

export interface GraphStudioHost {
  readonly hostApi: typeof HOST_API;
  /** Replaces the whole graph; resolves once its frame is set, rejects when overtaken. */
  loadGraph(doc: object): Promise<LoadResult>;
  /** Centres and selects the node with exactly this id; false, and nothing moves, when none has it. */
  focusNode(id: string): boolean;
  /** Selects exactly these ids, the last one primary; `[]` clears. False, and nothing changes, when one is unknown. */
  selectNodes(ids: readonly string[]): boolean;
  readonly selectedIds: readonly string[];
  /** Set by the host; `null` shows the node's own label, kind and path. */
  resolve: Resolve | null;
  /** Forgets the cached preview of `id`, and asks again where it is shown. */
  invalidate(id: string): void;
}

export interface GraphStudioElement extends HTMLElement, GraphStudioHost {
  /**
   * @internal Outside the v1 promise: the studio speaks dense indices. `null` while the element
   * is not in a document.
   */
  readonly studio: Studio | null;
  /** @internal The view the studio draws on; dense indices again. `null` while not in a document. */
  readonly view: View | null;
  /**
   * @internal Stops the motor worker where it stands and lets nothing replace it, so a gate can
   * watch a dead worker from the outside; the next layout opens a new worker as usual.
   */
  stopMotor(): void;
  /** @internal How long the watchdog waits before it calls a silent worker dead, in ms. */
  readonly watchdogBoundMs: number;
}

/** The name a host sees for a failure: the code the UI shows, else the error's own name. */
export function wireError(shown: ShownError): string {
  return shown.code ?? shown.title;
}
