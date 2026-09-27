/**
 * Vite config for the visual-parity harness (verification rig — not shipped).
 *
 * The harness mounts TWO independent copies of the graph engine in ONE document:
 *
 *   @ge-host        osionos' in-tree `packages/graph-engine`  — the reference,
 *                   i.e. byte-for-byte what the app renders today
 *   @ge-standalone  this repo's `src`                           — the extraction
 *
 * Same document on purpose: the aurora background is driven by its own rAF loop,
 * so two separately-loaded copies would sit at different phases and any pixel
 * comparison between them would be meaningless. One document and one frame clock
 * means the only possible source of a difference is the engine source itself.
 *
 * Two deliberate choices about dependencies:
 *
 * 1. This file imports NOTHING. It exports a plain object rather than using
 *    `defineConfig`, and it configures esbuild's JSX transform inline instead of
 *    using @vitejs/plugin-react. The rig runs in a container where vite is
 *    installed in the rig's own node_modules while this file is bind-mounted in
 *    from the repo — so a bare `import "vite"` here would resolve against the
 *    repo's node_modules, which deliberately has no bundler, and fail. Keeping
 *    the config import-free makes it location-independent.
 *
 * 2. The bare deps (d3-force, react, react-dom) are aliased to THIS repo's
 *    node_modules, because osionos has no host node_modules at all — its
 *    packages are only ever built inside its own Docker container. That also
 *    guarantees both copies run against identical resolved dependency versions,
 *    which is a precondition for the diff to mean anything.
 *
 * Paths come from env so the same file works on the VM and inside the container.
 */

const SELF = process.env.SELF ?? "/work";
const HOST_APP = process.env.HOST_APP ?? "/home/dlesieur/Documents/osionos";
const NM = `${SELF}/node_modules`;
/**
 * Dev-server port. 4322 by default rather than something arbitrary: it is the
 * port this VM's firewall is configured to allow for the rig, and it was
 * previously hardcoded as 5555 in three separate places (this file, the rig CMD,
 * and run-parity.sh), which is exactly how a port drifts out of sync with the
 * firewall. Override with PARITY_PORT.
 */
const PORT = Number(process.env.PARITY_PORT ?? 4322);

export default {
  root: `${SELF}/verify/parity`,
  server: {
    port: PORT,
    strictPort: true,
    host: "0.0.0.0",
    // The rig reads the osionos checkout, which lives outside the served root.
    fs: { allow: [SELF, HOST_APP] },
  },
  // esbuild's own JSX transform, so no @vitejs/plugin-react dependency is needed.
  esbuild: { jsx: "automatic", jsxImportSource: "react" },
  resolve: {
    alias: [
      { find: /^@ge-host$/, replacement: `${HOST_APP}/packages/graph-engine/src/index.ts` },
      { find: /^@ge-standalone$/, replacement: `${SELF}/src/index.ts` },
      { find: /^d3-force$/, replacement: `${NM}/d3-force/src/index.js` },
      { find: /^react$/, replacement: `${NM}/react/index.js` },
      { find: /^react-dom$/, replacement: `${NM}/react-dom/index.js` },
      { find: /^react-dom\/client$/, replacement: `${NM}/react-dom/client.js` },
      { find: /^react\/jsx-runtime$/, replacement: `${NM}/react/jsx-runtime.js` },
      // Vite's dev server asks for the *dev* JSX runtime and its build asks for
      // the production one. Both are aliased so the same config works either
      // way, which is what lets the rig serve live in dev and pre-bundle for
      // the browser check without editing anything.
      { find: /^react\/jsx-dev-runtime$/, replacement: `${NM}/react/jsx-dev-runtime.js` },
    ],
  },
};
