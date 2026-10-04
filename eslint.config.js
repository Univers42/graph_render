// Standalone flat config for graph-engine.
//
// Ported from osionos's root eslint.config.js. Two things are ported:
//
//   1. The rule DEFINITIONS the copied source depends on. Without these the port
//      reports failures that are artifacts of an incomplete port rather than real
//      defects: `src/core/render/nodeGlassDark.ts` has an unused `_style` arg that
//      the host allows via the "^_" argsIgnorePattern convention, and
//      `src/react/useGraphEngine.ts` carries a `react-hooks/exhaustive-deps`
//      disable comment for a plugin that must be registered or the rule name is
//      unknown and the comment itself errors.
//
//   2. The core/react import firewall — the one boundary that makes this package
//      what it claims to be.
//
// Deliberately NOT ported: the host's `@/*` ban ("must not import host-app code").
// There is no host app in this repo, so that half of the rule is meaningless here;
// keeping only the React ban is the correct minimal port, not a corner cut.
// Also not ported: the four other packages' firewalls (draw-engine, osionos-ui,
// http-gate, markdown-engine) — they do not exist here. Nor the host's
// `ignores` for vendored trees, for the same reason.
import js from "@eslint/js";
import ts from "typescript-eslint";
import react from "eslint-plugin-react";
import reactHooks from "eslint-plugin-react-hooks";

export default [
  js.configs.recommended,
  ...ts.configs.recommended,
  {
    files: ["**/*.{ts,tsx}"],
    plugins: {
      react,
      "react-hooks": reactHooks,
    },
    languageOptions: {
      parserOptions: {
        ecmaFeatures: { jsx: true },
      },
    },
    rules: {
      ...react.configs.recommended.rules,
      ...reactHooks.configs.recommended.rules,
      "react/react-in-jsx-scope": "off", // No necesario en React 17+
      // TypeScript already validates component props at compile time, so the
      // runtime-era prop-types rule is redundant noise (and can't read
      // forwardRef<T, Props> generics, producing false positives).
      "react/prop-types": "off",
      // Ported from the host (eslint.config.js:52). The host sets "warn" and
      // escalates it via --max-warnings=0; keeping the same severity keeps the same
      // effective bar. Zero occurrences in src/ today, so this costs nothing.
      "@typescript-eslint/no-explicit-any": "warn",
      "no-unused-vars": "off",
      // Regla de prefijo '_'
      "@typescript-eslint/no-unused-vars": ["error", { argsIgnorePattern: "^_" }],
    },
    settings: {
      react: { version: "detect" },
    },
  },
  // The SDK's `.mjs` files run under Node and name a handful of Node/web globals, none of
  // which a bare `eslint <dir>` config knows about. The list is inline rather than the
  // `globals` package because `globals` is not a direct dependency and eight names do not
  // justify one; the ninth global a `.mjs` file needs is added to this list.
  {
    files: ["crates/graph-sdk-js/**/*.mjs"],
    languageOptions: {
      globals: {
        Buffer: "readonly",
        Response: "readonly",
        TextEncoder: "readonly",
        URL: "readonly",
        WebAssembly: "readonly",
        console: "readonly",
        fetch: "readonly",
        process: "readonly",
      },
    },
  },
  {
    files: ["src/core/**/*.{ts,tsx}"],
    rules: {
      "no-restricted-imports": [
        "error",
        {
          patterns: [
            {
              group: ["react", "react-dom", "react/*", "react-dom/*"],
              message:
                "graph-engine core must be framework-agnostic. React lives in src/react/.",
            },
          ],
        },
      ],
    },
  },
];
