// The d3-force version this repo's d3 arms are attributed to.
//
// `package-lock.json` pins `d3-force@3.0.0` and `crates/graph-cli/src/bench.rs:229` and
// `harness/stress-d3.mjs` say so in prose; none of that is a check. Three arms resolve
// d3 themselves — `harness/stress-d3.mjs`, `harness/oracle-tick-bench.mjs` — and each
// compares positions or wall time against the other, so a silently-resolved different d3
// would be measured as a layout or performance difference. The version is therefore read
// from the package the resolver actually picked, and a mismatch is a refusal (exit 2)
// rather than a number nobody can attribute.
//
// `resolve("d3-force/package.json")` is not usable: `d3-force`'s `exports` map does not
// list `./package.json`, so that subpath is `ERR_PACKAGE_PATH_NOT_EXPORTED`. The entry
// point is resolved instead and the nearest `package.json` above it is read.

import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";

/** The one version this repo's recorded d3 correlations and timings were taken against. */
export const PINNED_D3_VERSION = "3.0.0";

/** How far above the resolved entry point to look for the owning `package.json`. */
const MAX_MANIFEST_DEPTH = 8;

/**
 * The `version` of the package `resolve("d3-force")` landed in, or `null` when the path
 * names no d3-force package. `resolve` is passed in rather than created here, so this
 * reads the tree the CommonJS resolution path (`createRequire`) actually picked — the
 * same one both arms fall back to when ESM resolution fails.
 */
export function resolvedD3Version(resolve) {
  let entry;
  try {
    entry = resolve("d3-force");
  } catch {
    return null;
  }
  for (let dir = dirname(entry), depth = 0; depth < MAX_MANIFEST_DEPTH; dir = dirname(dir), depth += 1) {
    let parsed;
    try {
      parsed = JSON.parse(readFileSync(join(dir, "package.json"), "utf8"));
    } catch {
      continue;
    }
    if (parsed.name === "d3-force") return parsed.version ?? null;
  }
  return null;
}

/**
 * Why the resolved d3-force is not the pinned one, or `null` when it is. There is no
 * shape of "different" that a recorded correlation or timing is still comparable under.
 */
export function versionRefusal(found) {
  if (found === PINNED_D3_VERSION) return null;
  return found === null
    ? `the resolved d3-force carries no readable version (pinned ${PINNED_D3_VERSION})`
    : `resolved d3-force is ${found}, not the pinned ${PINNED_D3_VERSION}`;
}
