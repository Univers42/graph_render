// Options-independent build paths: provisional `build` and the ingest contract path.

import {
  BuildRefusedError,
  ContractRefusedError,
} from "../../crates/graph-sdk-js/src/index.ts";
import { check, refusedWith } from "./lib.mjs";

export async function runBuildSection(ctx) {
  const { motor } = ctx;
const node = (id, kind) => ({
  id,
  kind,
  database_id: null,
  source: "s",
  label: id.toUpperCase(),
  group: null,
  weight: 0.5,
  version: 0,
  has_note: false,
  icon: null,
});
const ingest = JSON.stringify({
  version: 1,
  nodes: [node("a", "record"), node("b", "note")],
  edges: [{ id: "e", source: "a", target: "b", kind: "relation", label: "", strength: 0.5, directed: false, record_id: null }],
});

const handle = motor.build(ingest);
check("build reports the right node count", motor.nodeCount(handle) === 2);

// --- the ingest contract path (`gm_build_contract`) ------------------------------------
//
// `build` above takes the *provisional* node/edge JSON; this takes the phase-10 contract
// document — the one shape every source maps to — and the motor derives the graph. Both
// must keep working, and neither may accept the other's document: a caller that handed a
// contract document to `build`, or a node/edge document to `buildContract`, has made a
// mistake that a plausible-looking graph would hide.
const contractSource = {
  version: 1,
  source: "s",
  collections: [
    {
      id: "task",
      name: "Tasks",
      titleField: "name",
      fields: [
        { id: "name", name: "Name", role: "title", link: null },
        { id: "labels", name: "Labels", role: "tags", link: null },
      ],
    },
  ],
  records: [
    { id: "r1", collection: "task", deleted: false, updatedAt: 7, values: { name: "Write", labels: ["docs"] } },
    { id: "r2", collection: "task", deleted: false, updatedAt: 8, values: { name: "Ship", labels: ["docs", "graph"] } },
  ],
};
const contractJson = JSON.stringify(contractSource);
const contractHandle = motor.buildContract(contractJson);
check(
  "buildContract derives one node per live record plus one per tag value",
  motor.nodeCount(contractHandle) === 4,
);
check(
  "buildContract's node ids are the contract's, in derivation order",
  JSON.parse(motor.toJSON(motor.layout(contractHandle, "layout.grid").handle)).nodes.id.join(",") ===
    ["s:task:r1", "s:task:r2", "tag:docs", "tag:graph"].join(","),
);
check(
  "the two build paths are not interchangeable: a node/edge document is not a contract",
  await refusedWith(ContractRefusedError, () => motor.buildContract(ingest)),
);
check(
  "…and a contract document is not the provisional node/edge JSON",
  await refusedWith(BuildRefusedError, () => motor.build(contractJson)),
);
check(
  "a contract document with an unknown member is refused, not half-read",
  await refusedWith(ContractRefusedError, () =>
    motor.buildContract(contractJson.replace('"source":"s"', '"source":"s","extra":1')),
  ),
);
check(
  "the refusal names ContractInvalid, not the provisional IngestInvalid",
  (() => {
    try {
      motor.buildContract(contractJson.replace('"version":1', '"version":2'));
      return false;
    } catch (error) {
      return error instanceof ContractRefusedError && error.codeName === "ContractInvalid";
    }
  })(),
);
motor.release(contractHandle);
  ctx.node = node;
  ctx.ingest = ingest;
  ctx.handle = handle;
}
