/**
 * app/vite.pack.config.ts — the two builds the pack needs, in one config.
 *
 *   PACK_MODE=pack (the default)  packages/graph-studio/src/element.ts -> one ESM file,
 *                                 `graph-studio.js`, for a host to import
 *   PACK_MODE=host                app/embed.html with its one element import pointed at a built
 *                                 pack, which is the embed gate's host page over the pack
 *
 * Both take the `resolve` block and the worker format from `app/vite.config.ts` rather than
 * copying them: one React on a page, and the motor worker a module. `scripts/studio-pack.sh` and
 * `scripts/studio-embed.sh` pass the two directories in `PACK_OUT_DIR` and `PACK_DIR`.
 *
 * `rollupOptions.input` with `preserveEntrySignatures: "exports-only"`, not `build.lib`: the
 * element's exports must survive for the host, and the motor worker must stay a real file of its
 * own. A library build is free to fold that worker into the entry as a blob URL, and
 * `worker-src 'self'` — the CSP a host is asked for (docs/contract/packaging.md) — refuses a
 * blob: worker. Every chunk here is emitted under the flat names the pack verify checks.
 *
 * The pack is not minified: a host audits the bytes it ships, and every check in
 * scripts/studio-pack.sh is a line of the bundle. React is bundled (no `external`): an embed host
 * has no React of its own.
 */
import react from "@vitejs/plugin-react";
import { fileURLToPath } from "node:url";
import type { UserConfig } from "vite";
import { defineConfig } from "vite";

import studio from "./vite.config.ts";

const host: UserConfig = studio;
const ELEMENT = fileURLToPath(new URL("../packages/graph-studio/src/element.ts", import.meta.url));
const EMBED_PAGE = fileURLToPath(new URL("./embed.html", import.meta.url));
const ENTRY = "graph-studio.js";

/** The host config's own resolve block as alias entries, with `extra` appended. */
function resolveWith(extra: { find: RegExp; replacement: string }[]): UserConfig["resolve"] {
  const own = host.resolve?.alias ?? {};
  const entries = Object.entries(own).map(([find, replacement]) => ({ find, replacement }));
  return { ...host.resolve, alias: [...entries, ...extra] };
}

/** One directory per worker chunk, like the entry: a worker beside the file that spawns it. */
const flatWorker = { ...host.worker, rollupOptions: { output: { format: "es" as const, entryFileNames: "[name].js", chunkFileNames: "[name].js" } } };

function packBuild(out: string): UserConfig {
  return {
    base: "./",
    plugins: [react()],
    resolve: resolveWith([]),
    worker: flatWorker,
    // The pack carries the two wasm files the host serves; nothing else from app/public.
    publicDir: false,
    build: {
      outDir: out,
      // Explicit, because the outdir is outside this package's root: vite empties an outdir it
      // cannot recognise as inside the root only when told to, and the pack must not inherit a
      // file from an earlier build (scripts/studio-pack.sh builds the same tree twice).
      emptyOutDir: true,
      target: "es2022",
      minify: false,
      rollupOptions: {
        preserveEntrySignatures: "exports-only",
        input: ELEMENT,
        output: { format: "es", entryFileNames: ENTRY, chunkFileNames: "[name].js" },
      },
    },
  };
}

function hostBuild(out: string, pack: string): UserConfig {
  // External, so the served page really imports the pack's own file: bundling it would measure
  // this repository's sources instead of the artifact a host downloads. The whole relative
  // specifier is matched (`app/src/embed.ts` imports the element by path), so the replacement is
  // the absolute path of the pack's entry and not the tail of the old one.
  const packed = `${pack}/${ENTRY}`;
  return {
    base: "./",
    plugins: [react()],
    resolve: resolveWith([{ find: /^.*graph-studio\/src\/element\.ts$/, replacement: packed }]),
    worker: host.worker,
    publicDir: false,
    build: {
      outDir: out,
      emptyOutDir: true,
      target: "es2022",
      minify: false,
      rollupOptions: {
        input: EMBED_PAGE,
        external: [packed],
        // Flat, because the page's only script import is `./graph-studio.js`: a chunk under
        // `assets/` would resolve that to `assets/graph-studio.js`, which no host serves.
        output: { format: "es", entryFileNames: "[name].js", chunkFileNames: "[name].js",
                  paths: (id: string) => (id === packed ? `./${ENTRY}` : id) },
      },
    },
  };
}

function needed(name: string): string {
  const value = process.env[name] ?? "";
  if (value === "") throw new Error(`vite.pack.config.ts: ${name} is the directory this build writes to`);
  return value;
}

export default defineConfig(() =>
  process.env.PACK_MODE === "host" ? hostBuild(needed("PACK_OUT_DIR"), needed("PACK_DIR"))
                                   : packBuild(needed("PACK_OUT_DIR")));