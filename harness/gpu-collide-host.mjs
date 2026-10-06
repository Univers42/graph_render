// The collide pass's host check over the emitted fixtures: the CPU's `resolve` in f64 against
// the fixture's own `delta_collide`, and the f32-narrowing floor (Amendment 3). Host-only, no
// GPU. Run over the 1M fixtures too, with at least 12 GB free.
//
// Run: node --experimental-strip-types harness/gpu-collide-host.mjs <dir> [--only 1k,10k,50k]
//
// For every `.gmfx` in <dir> it prints one line:
//   PASS|FAIL <name> reproduce=<maxAbs> floorRms=<…> k_c=<…> rmsRef=<…>
// It FAILs when reproduce > 1e-12 · max(1, max|reference|). Exit 0 all pass, 1 any fail,
// 2 could not run.
//
// Caveat: the pair set is the grid's candidate set, not the CPU's exact pair set — a pair whose
// f32 distance rounds across the diameter is visited by one arm and not the other, and its push
// is near zero, so `reproduce` reads slightly high on a fixture with such a pair. The committed
// fixtures have none, and the numpy analysis (`harness/gpu-collide-floor.py`) reads the same.

import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

import { collideFloor, hostCollide, maxContacts } from "../crates/graph-sdk-js/src/gpu/collide-host.ts";
import { gridFor } from "../crates/graph-sdk-js/src/gpu/collide.ts";
import { loadFixture } from "../crates/graph-sdk-js/src/gpu/fixture.ts";

/** The reproduction tolerance: `1e-12 · max(1, max|reference|)`. */
const REPRODUCE_TOLERANCE = 1e-12;

/** One fixture's host numbers. */
function measure(bytes) {
  const f = loadFixture(bytes);
  const grid = gridFor(f.posX, f.posY);
  const host = hostCollide(f.posX, f.posY, grid);
  let reproduce = 0;
  let maxRef = 0;
  let sumRef = 0;
  for (let i = 0; i < f.n; i += 1) {
    const rx = f.delta.collide.x[i] ?? 0;
    const ry = f.delta.collide.y[i] ?? 0;
    reproduce = Math.max(reproduce, Math.abs((host.x[i] ?? 0) - rx), Math.abs((host.y[i] ?? 0) - ry));
    maxRef = Math.max(maxRef, Math.abs(rx), Math.abs(ry));
    sumRef += rx * rx + ry * ry;
  }
  return {
    reproduce,
    floorRms: collideFloor(f.posX, f.posY, grid),
    k_c: maxContacts(f.posX, f.posY, grid),
    rmsRef: Math.sqrt(sumRef / (2 * f.n)),
    tolerance: REPRODUCE_TOLERANCE * Math.max(1, maxRef),
  };
}

function main() {
  const [dir, onlyArg] = process.argv.slice(2);
  if (dir === undefined) {
    console.error("usage: gpu-collide-host.mjs <dir> [--only 1k,10k,50k]");
    return 2;
  }
  const only = onlyArg?.startsWith("--only") ? process.argv[process.argv.indexOf(onlyArg) + 1]?.split(",") : undefined;
  let files;
  try {
    files = readdirSync(dir).filter((name) => name.endsWith(".gmfx")).sort();
  } catch (error) {
    console.error(`gpu-collide-host: cannot read ${dir}: ${error.message}`);
    return 2;
  }
  const kept = only ? files.filter((name) => only.some((size) => name.includes(`-${size}-`))) : files;
  if (kept.length === 0) {
    console.error(`gpu-collide-host: no .gmfx fixtures in ${dir}${only ? ` matching ${only.join(",")}` : ""}`);
    return 2;
  }
  let failed = 0;
  for (const name of kept) {
    const bytes = readFileSync(join(dir, name));
    const buffer = bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength);
    const m = measure(buffer);
    const pass = m.reproduce <= m.tolerance;
    if (!pass) failed += 1;
    console.log(
      `${pass ? "PASS" : "FAIL"} ${name} reproduce=${m.reproduce.toPrecision(6)} ` +
      `floorRms=${m.floorRms.toPrecision(6)} k_c=${m.k_c} rmsRef=${m.rmsRef.toPrecision(6)}`,
    );
  }
  return failed === 0 ? 0 : 1;
}

process.exit(main());
