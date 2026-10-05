import { matchesQuery, rowOf } from "../src/console/queryMatch.ts";
import { UNGROUPED, type GraphMeta } from "../src/source/meta.ts";

const META: GraphMeta = {
  nodeCount: 6,
  ids: ["a", "b", "c", "d", "e", "f"],
  labels: ["Alpha hub", "Beta note", "Gamma tag", "Delta db", "Epsilon rec", "Zeta note"],
  kinds: ["record", "note", "tag", "database", "record", "note"],
  groups: ["Core", "Tags", UNGROUPED],
  group: Uint16Array.of(0, 0, 1, 0, 2, 0),
  weight: Float32Array.of(1, 0.5, 0.25, 0, 0.5, 0.25),
  degree: Uint32Array.of(3, 1, 2, 0, 0, 0),
  maxDegree: 3,
  versions: Float64Array.of(1700000000, 1600000000, 1700000000, 0, 1500000000, 1800000000),
  tags: [["one", "two"], ["two"], [], ["three"], [], ["ONE"]],
  dbs: ["db-1", "db-1", "db-2", "", "db-3", "db-1"],
  paths: ["src/a.md", "src/b.md", "", "src/d.md", "", "src/f.md"],
};

console.log(JSON.stringify(rowOf(META, 0)));
console.log(Array.from({ length: 6 }, (_, i) => matchesQuery(
  { kind: "field", field: "group", op: "", value: "Core" }, rowOf(META, i),
)));