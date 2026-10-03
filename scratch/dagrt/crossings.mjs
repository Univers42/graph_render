// Our own crossing counts, read against the numbers docs/measurements/phase05-crossings.md
// froze before the fix: the 7 fixtures and the 230-seed sweep total (sumOurs 5242).
import { readFileSync } from "node:fs";

const graphs = JSON.parse(readFileSync("target/dag-crossings.json", "utf8"));
const frozen = {
  chain: 0, diamond: 0, cyclic: 0, "multi-span": 0,
  "wide-layer": 36, disconnected: 0,
};
let moved = 0;
for (const [name, want] of Object.entries(frozen)) {
  const got = graphs.find((g) => g.name === name)?.our_crossings;
  const same = got === want;
  if (!same) moved += 1;
  console.log(`${same ? "ok  " : "MOVED"} ${name}: ours ${got}, doc ${want}`);
}
const sweep = graphs.filter((g) => g.name.startsWith("synthetic-"));
const sum = sweep.reduce((a, g) => a + g.our_crossings, 0);
console.log(`${sum === 5242 ? "ok  " : "MOVED"} sweep: ${sweep.length} graphs, sumOurs ${sum}, doc 5242`);
const worse = sweep.filter((g) => g.our_crossings > 0).length;
console.log(`sweep graphs with crossings: ${worse}`);
process.exit(moved === 0 && sum === 5242 ? 0 : 1);