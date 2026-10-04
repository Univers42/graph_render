// The studio's lint: the house limits as rules, and the layering of the plan as import
// bans. Run from the repository root (scripts/studio.sh lint): a flat config cannot see
// files above the directory it is run from, and packages/ is a sibling of app/.
import jsxA11y from "eslint-plugin-jsx-a11y";
import reactHooks from "eslint-plugin-react-hooks";
import tseslint from "typescript-eslint";

const ORACLE = {
  regex: "^(\\.\\./)+src/(core|react|index)",
  message: "The oracle engine is never shipped: port what is needed (docs/decisions/render-ports-not-imports.md).",
};
const SDK = {
  regex: "crates/graph-sdk-js",
  message: "Only the motor's own files speak to the SDK: src/motor/worker.ts, src/motor/local.ts and src/motor/helper.ts.",
};
const REACT = { regex: "^react(-dom)?(/|$)", message: "The renderer draws on the canvas it is given and knows no UI library." };
const STUDIO = { regex: "graph-studio/", message: "The renderer does not know the studio." };
const INSIDE = {
  regex: "packages/(graph-render/|graph-studio/src/(?!element\\.ts$|motor/local\\.ts$))",
  message: "A host takes the element and nothing behind it.",
};

// Every DOM or React sink that parses a string as markup.
const SINKS = ["dangerouslySetInnerHTML", "innerHTML", "outerHTML", "insertAdjacentHTML"];

const banned = (...patterns) => ({ "no-restricted-imports": ["error", { patterns }] });

const HOUSE = {
  "max-lines": ["error", { max: 300 }],
  "max-lines-per-function": ["error", { max: 40, skipBlankLines: true, skipComments: true }],
  "max-params": ["error", 4],
  "max-depth": ["error", 3],
  "@typescript-eslint/consistent-type-assertions": ["error", { assertionStyle: "never" }],
  "@typescript-eslint/consistent-type-imports": ["error", { fixStyle: "inline-type-imports" }],
  "@typescript-eslint/restrict-template-expressions": ["error", { allowNumber: true }],
  "@typescript-eslint/no-confusing-void-expression": ["error", { ignoreArrowShorthand: true }],
  // node:test returns a promise for the runner, which awaits it; a test file never does.
  "@typescript-eslint/no-floating-promises": ["error", {
    allowForKnownSafeCalls: [{ from: "package", package: "node:test", name: ["test", "describe", "it"] }],
  }],
  "no-restricted-syntax": [
    "error",
    { selector: "ExportDefaultDeclaration", message: "Named exports only." },
    { selector: "TSEnumDeclaration", message: "No enums: a union of literals." },
    // Verdict 5 (docs/contract/host-api.md): a host's preview strings reach the page as text
    // only, so no sink that parses a string as HTML exists anywhere for one to reach.
    ...SINKS.map((name) => ({ selector: `JSXAttribute[name.name='${name}']`, message: `No ${name}: render text (host-api verdict 5).` })),
    ...SINKS.map((name) => ({ selector: `Property[key.name='${name}']`, message: `No ${name}: render text (host-api verdict 5).` })),
    ...SINKS.map((name) => ({ selector: `MemberExpression[property.name='${name}']`, message: `No ${name}: set textContent (host-api verdict 5).` })),
  ],
};

export default tseslint.config(
  // Two breaks, each checked on its own by scripts/studio.sh: tests/breaks must fail tsc with
  // TS2430, and tests/ui/raw-html.tsx must draw the markup-sink bans.
  {
    ignores: [
      "app/dist/**", "app/public/**", "**/node_modules/**",
      "packages/graph-studio/tests/breaks/**", "packages/graph-studio/tests/ui/raw-html.tsx",
    ],
  },
  ...tseslint.configs.strictTypeChecked,
  {
    languageOptions: {
      parserOptions: {
        project: [
          "app/tsconfig.json", "packages/graph-render/tsconfig.json",
          "packages/graph-studio/tsconfig.json", "packages/graph-studio/tsconfig.motor.json",
        ],
        tsconfigRootDir: new URL("..", import.meta.url).pathname,
      },
    },
    rules: HOUSE,
  },
  { files: ["**/*.tsx"], ...jsxA11y.flatConfigs.strict },
  { files: ["**/*.{ts,tsx}"], ...reactHooks.configs.flat["recommended-latest"] },
  { files: ["packages/graph-render/**"], rules: banned(ORACLE, SDK, REACT, STUDIO) },
  { files: ["packages/graph-studio/**"], rules: banned(ORACLE, SDK) },
  {
    files: ["packages/graph-studio/src/motor/{worker,local,helper}.ts", "packages/graph-studio/tests/*.motor.test.ts", "packages/graph-studio/tests/motor.ts"],
    rules: banned(ORACLE),
  },
  { files: ["app/src/**"], rules: banned(ORACLE, SDK, INSIDE) },
  // The parity page is the studio's own second page: it mounts the parity module directly,
  // with no motor behind it (packages/graph-studio/src/parity/page.ts).
  { files: ["app/src/parity.ts"], rules: banned(ORACLE, SDK) },
  // The three bundler configs, by exact path: no tsconfig holds them, and a bundler's config is its
  // default export, which `HOUSE` bans. Their exemption is exactly these three files — the markup-sink
  // ban included, so `packages/` has no JavaScript blind spot. `tests/ui/raw-html.tsx` is
  // deliberately NOT here: it is the negative control that has to draw all five bans
  // (scripts/studio.sh lint asserts exactly five errors on it, all `no-restricted-syntax`).
  {
    files: ["app/vite.config.ts", "app/vite.embed.config.ts", "packages/graph-studio/ui-tests.config.mjs"],
    ...tseslint.configs.disableTypeChecked,
    rules: { ...tseslint.configs.disableTypeChecked.rules, "no-restricted-syntax": "off" },
  },
);
