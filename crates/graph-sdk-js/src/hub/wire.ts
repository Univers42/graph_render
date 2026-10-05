// The hub wire, as graph-contract's codegen writes it. Named exactly as
// `crates/graph-contract/generated/hub.d.ts` names it, so a change to the Rust schema
// surfaces here as a type error instead of as a wrong payload. `import type` only
// (C18), the same rule `src/types.ts:1-10` states for `snapshot-header.d.ts`.
export type {
  AnswerWire, BatchWire, Cardinality, ChangeKind, ChangeWire, DeleteWire, ErrorWire,
  Field, JsonValue, Link, ManifestWire, NoticeWire, Role, UpsertWire,
} from "../../../graph-contract/generated/hub.d.ts";
export type { Collection, StoredDelete, StoredRecord } from "../../../graph-contract/generated/hub.d.ts";

// One knob for the four `negctl-hub-sdk-*` rows (`scripts/orch/rows/hub-sdk.rows`). A row sets
// it the only way `scripts/orch/node-slim.sh` takes env, `env GM_HUB_SDK_BREAK=<knob> node …`,
// which lands in `process.env`; `globalThis` is read first so a caller that has no `process` at
// all — a browser, §3 — can still spell the knob. Both are `undefined` when nobody set it.
interface Knobs {
  readonly GM_HUB_SDK_BREAK?: string;
  readonly process?: { readonly env?: { readonly GM_HUB_SDK_BREAK?: string } };
}

/** True when `GM_HUB_SDK_BREAK` names `knob`: the four negative controls' single switch.
 *
 * A break knob is a test seam, not a setting. It lives here so each row turns exactly one
 * shipped behaviour off, and it is read through one helper so the four source-side breaks stay
 * four lines. Nothing but the four `negctl-hub-sdk-*` rows ever sets it.
 */
export function breaking(knob: string): boolean {
  const given = globalThis as unknown as Knobs;
  const name = given.GM_HUB_SDK_BREAK ?? given.process?.env?.GM_HUB_SDK_BREAK;
  return name === knob;
}
