/**
 * The embed bundle's entry (`app/vite.embed.config.ts`, `docs/deploy/service.md`): importing
 * it registers `<graph-studio>` with the defaults. A host that wants options defines its own
 * tag with the exported `defineGraphStudio`. The element resolves its `wasm` attribute
 * against the page, so a page outside `/embed/<version>/` names the module there.
 */
import { defineGraphStudio } from "../../packages/graph-studio/src/element.ts";

defineGraphStudio();

export * from "../../packages/graph-studio/src/element.ts";
