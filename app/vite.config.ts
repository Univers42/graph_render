// Vite config for graph-motor studio.
//
// Two things here are load-bearing and not defaults:
//   1. `server.fs.allow` — the studio IMPORTS the engine, it does not copy it: the wasm
//      SDK comes from `crates/graph-sdk-js/src/index.ts` and the aurora render layer from
//      `src/core/render`. Those live ABOVE this package's root, and Vite's dev server
//      refuses to serve anything outside it, so the repo root is allow-listed explicitly.
//   2. `optimizeDeps.exclude` — the SDK is a `.ts` source dependency resolved from outside
//      `node_modules`; pre-bundling it would hand Vite a second copy of the module with
//      its own wasm-loader singleton, which is exactly the kind of "it works in dev and
//      not in the build" split this file exists to prevent.

import react from "@vitejs/plugin-react";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";

const here = fileURLToPath(new URL(".", import.meta.url));
const repoRoot = fileURLToPath(new URL("../", import.meta.url));

export default defineConfig({
  plugins: [react()],
  server: { host: "0.0.0.0", port: 5173, fs: { allow: [here, repoRoot] } },
  preview: { host: "0.0.0.0", port: 5173 },
  optimizeDeps: { exclude: ["graph-sdk-js"] },
  build: { outDir: "dist", emptyOutDir: true, target: "es2022" },
});
