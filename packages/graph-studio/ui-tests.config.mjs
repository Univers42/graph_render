// Bundles tests/ui/*.test.tsx for node, which runs no JSX. React comes from app/: this
// package has no node_modules of its own (package.json, "//toolchain").
import { readdirSync } from "node:fs";
import { join } from "node:path";

const here = import.meta.dirname;
const modules = join(here, "../../app/node_modules");
const tests = join(here, "tests/ui");

export default {
  input: readdirSync(tests).filter((name) => name.endsWith(".test.tsx")).map((name) => join(tests, name)),
  platform: "node",
  external: [/^node:/],
  resolve: { alias: { react: join(modules, "react"), "react-dom": join(modules, "react-dom") } },
  output: { dir: join(here, "../../target/ui-tests"), format: "esm" },
};
