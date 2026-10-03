// The three id -> index registries a module exposes (layouts, POST capabilities, analyses),
// each scanned once per motor (C1) and resolved by name, never by a hard-coded index.
//
// The map does not leave this class. It used to be returned by reference, and a `Map` is
// mutable: a holder could `set("post.route.grid", 0)` into the layout map and resolve a
// caller's layout id to an index the module never issued for it. Nothing needed the map
// itself — the two callers want keys and lookups — so the accessors below answer those and the
// map stays where it is written.

import { AnalysisRefusedError, PostRefusedError, RunRefusedError } from "./errors.ts";
import { readRegistry } from "./calls.ts";
import type { RawExports } from "./wasm.ts";

export class Registries {
  #layoutIds: Map<string, number> | null = null;
  #postIds: Map<string, number> | null = null;
  #analysisIds: Map<string, number> | null = null;

  /** The layout ids, in registry order — read once per motor (C1). */
  layouts(exports: RawExports): readonly string[] {
    return [...this.#layouts(exports).keys()];
  }

  layoutIndex(exports: RawExports, layoutId: string): number {
    const index = this.#layouts(exports).get(layoutId);
    if (index === undefined) throw new RunRefusedError(`unknown layout id "${layoutId}"`);
    return index;
  }

  posts(exports: RawExports): readonly string[] {
    return [...this.#posts(exports).keys()];
  }

  postIndex(exports: RawExports, postId: string): number {
    const index = this.#posts(exports).get(postId);
    if (index === undefined) throw new PostRefusedError(`unknown post id "${postId}"`);
    return index;
  }

  analyses(exports: RawExports): readonly string[] {
    return [...this.#analyses(exports).keys()];
  }

  analysisIndex(exports: RawExports, analysisId: string): number {
    const index = this.#analyses(exports).get(analysisId);
    if (index === undefined) throw new AnalysisRefusedError(`unknown analysis id "${analysisId}"`);
    return index;
  }

  // A scan that refuses names its own id space: `Motor#layout` is the caller for the layout
  // scan, and a caller debugging a dead layout wants the layout codes, not a generic one.
  #layouts(exports: RawExports): Map<string, number> {
    this.#layoutIds ??= readRegistry(exports, "gm_layout_count", "gm_layout_id", refusedAs(RunRefusedError));
    return this.#layoutIds;
  }

  #posts(exports: RawExports): Map<string, number> {
    this.#postIds ??= readRegistry(exports, "gm_post_count", "gm_post_id", refusedAs(PostRefusedError));
    return this.#postIds;
  }

  #analyses(exports: RawExports): Map<string, number> {
    this.#analysisIds ??= readRegistry(exports, "gm_analysis_count", "gm_analysis_id", refusedAs(AnalysisRefusedError));
    return this.#analysisIds;
  }
}

/** The `(message, code) => Error` shape `readRegistry` refuses with, bound to one class.
 *  Written once so the three scans name their own id space and none of them falls back to
 *  the layout one by accident. */
function refusedAs(errorClass: new (message: string, code: number) => Error) {
  return (message: string, code: number): Error => new errorClass(message, code);
}
