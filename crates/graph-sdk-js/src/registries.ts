// The three id -> index registries a module exposes (layouts, POST capabilities, analyses),
// each scanned once per motor (C1) and resolved by name, never by a hard-coded index.

import { AnalysisRefusedError, PostRefusedError, RunRefusedError } from "./errors.ts";
import { readRegistry } from "./calls.ts";
import type { RawExports } from "./wasm.ts";

export class Registries {
  #layoutIds: Map<string, number> | null = null;
  #postIds: Map<string, number> | null = null;
  #analysisIds: Map<string, number> | null = null;

  /** The layout registry's id -> index map, read once per motor (C1). */
  layouts(exports: RawExports): Map<string, number> {
    if (this.#layoutIds === null) {
      this.#layoutIds = readRegistry(exports, "gm_layout_count", "gm_layout_id");
    }
    return this.#layoutIds;
  }

  layoutIndex(exports: RawExports, layoutId: string): number {
    const index = this.layouts(exports).get(layoutId);
    if (index === undefined) throw new RunRefusedError(`unknown layout id "${layoutId}"`);
    return index;
  }

  posts(exports: RawExports): Map<string, number> {
    if (this.#postIds === null) {
      this.#postIds = readRegistry(exports, "gm_post_count", "gm_post_id");
    }
    return this.#postIds;
  }

  postIndex(exports: RawExports, postId: string): number {
    const index = this.posts(exports).get(postId);
    if (index === undefined) throw new PostRefusedError(`unknown post id "${postId}"`);
    return index;
  }

  analyses(exports: RawExports): Map<string, number> {
    if (this.#analysisIds === null) {
      this.#analysisIds = readRegistry(exports, "gm_analysis_count", "gm_analysis_id");
    }
    return this.#analysisIds;
  }

  analysisIndex(exports: RawExports, analysisId: string): number {
    const index = this.analyses(exports).get(analysisId);
    if (index === undefined) throw new AnalysisRefusedError(`unknown analysis id "${analysisId}"`);
    return index;
  }
}
