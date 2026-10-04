// The standalone host of <graph-studio>. The studio and the renderer are source packages
// above this directory with no node_modules of their own, which is what every entry here
// is for:
//   - `resolve.alias` + `dedupe`: their `react` imports resolve to this package's copy, and
//     to one copy. Two Reacts on a page fail at the first hook.
//   - `server.fs.allow`: the dev server refuses files outside its root otherwise.
//   - `worker.format`: the motor's worker is a module (it imports the SDK's sources).
//   - `server.headers` + `preview.headers`: plan P3's threads build (model (a) of
//     docs/decisions/browser-threads.md) needs a SharedArrayBuffer, which a browser
//     only exposes to a cross-origin isolated page; both headers are what the deploy
//     gates serve, so dev and preview behave like production does.
import react from "@vitejs/plugin-react";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";

const modules = fileURLToPath(new URL("./node_modules/", import.meta.url));
const repoRoot = fileURLToPath(new URL("../", import.meta.url));
// The studio, the parity page and the embed example beside it: the gates load /parity.html and
// /embed.html out of the same dist, so all three are built by one command (scripts/studio.sh build).
const page = (name: string): string => fileURLToPath(new URL(name, import.meta.url));

export default defineConfig({
  base: "./",
  plugins: [react()],
  resolve: {
    alias: { react: `${modules}react`, "react-dom": `${modules}react-dom` },
    dedupe: ["react", "react-dom"],
  },
  worker: { format: "es" },
  // 0.0.0.0 is the container's own interface; scripts/studio.sh publishes it on the
  // host's loopback only.
  server: {
    host: "0.0.0.0", port: 5174, strictPort: true, fs: { allow: [repoRoot] },
    headers: { "Cross-Origin-Opener-Policy": "same-origin", "Cross-Origin-Embedder-Policy": "require-corp" },
  },
  preview: {
    headers: { "Cross-Origin-Opener-Policy": "same-origin", "Cross-Origin-Embedder-Policy": "require-corp" },
  },
  build: {
    outDir: "dist", emptyOutDir: true, target: "es2022",
    rollupOptions: { input: { studio: page("index.html"), parity: page("parity.html"), embed: page("embed.html") } },
  },
});
