// The embed bundle: `<graph-studio>` and the SDK as ES modules the service serves under
// `/embed/<version>/`, and a host page imports through its own origin, because the motor's
// Worker must be same-origin (`docs/deploy/service.md`, host-api.md condition 13). `scripts/studio.sh
// embed DIR` passes `--outDir` and adds the two wasm builds; scripts/service.sh then names the
// directory by its content hash, the `<version>`. What differs from vite.config.ts, and why:
//   - `build.lib`: no page; the entries are modules with stable names a script tag can name.
//   - `define`: lib mode leaves `process.env.NODE_ENV` alone, and React reads it at import.
//   - `publicDir: false`: app/public holds the studio page's wasm and fixtures; the service
//     stages the two wasm builds itself and serves no fixtures.
import react from "@vitejs/plugin-react";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";

const modules = fileURLToPath(new URL("./node_modules/", import.meta.url));
const source = (name: string): string => fileURLToPath(new URL(name, import.meta.url));

export default defineConfig({
  base: "./",
  plugins: [react()],
  define: { "process.env.NODE_ENV": JSON.stringify("production") },
  resolve: {
    alias: { react: `${modules}react`, "react-dom": `${modules}react-dom` },
    dedupe: ["react", "react-dom"],
  },
  worker: { format: "es" },
  publicDir: false,
  build: {
    outDir: "dist-embed", emptyOutDir: true, target: "es2022",
    lib: {
      entry: { "graph-studio": source("src/embed.ts"), "graph-sdk": source("../crates/graph-sdk-js/src/index.ts") },
      formats: ["es"],
      fileName: (_format, name) => `${name}.js`,
    },
  },
});
