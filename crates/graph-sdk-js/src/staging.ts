// Stages a document through `gm_alloc` and frees it again (C7: the staging buffer is the
// SDK's job, not the caller's, since the caller never sees the pointer). Shared by
// `Motor#build` and `Motor#buildContract`, which differ only in the export they call and
// the error class a refusal takes. ASCII text is encoded straight into the staging buffer
// (`encodeInto`), so a document crosses into linear memory with one copy, not three.

import { type RawExports, toU32 } from "./wasm.ts";
import { BuildRefusedError, ContractRefusedError } from "./errors.ts";
import { encoder, invoke, lastError, type Loaded } from "./calls.ts";
import type { ColumnViews } from "./views.ts";
import type { Handle } from "./types.ts";

export interface StagedBuild {
  /** Names the buffer in the `gm_alloc` refusal: "ingest" or "contract". */
  buffer: string;
  call: "gm_build" | "gm_build_contract";
  /** The message and error class of the build export's own refusal. */
  refusal: string;
  refuse: (message: string, code: number) => Error;
}

/** The provisional-ingest build: `gm_build` over the document the host studio and the
 *  hash gate already speak — node/edge JSON, not a contract.
 *
 *  `Motor#build` is the two build paths' first half (`docs/contract/wasm-abi.md` "Two
 *  build paths"); what it accepts and what it refuses is here rather than on the method,
 *  because the buffer is what carries a document into linear memory and the buffer is
 *  this module's whole subject. */
export const INGEST_BUILD: StagedBuild = {
  buffer: "ingest",
  call: "gm_build",
  refusal: "gm_build refused the ingest buffer",
  refuse: (message, code) => new BuildRefusedError(message, code),
};

/** The contract build: `gm_build_contract` over an **ingest contract** document
 *  (`docs/contract/ingest-schema.json`, written by this package's own
 *  `rowsToIngest`/`notionToIngest` adapters) — the one shape every source maps to.
 *
 *  The other way in from {@link INGEST_BUILD}, and a separate export that stays separate:
 *  `gm_build` is what the host studio and the hash gate already speak, and the derivation
 *  from a contract document — roles to nodes, tags to hubs, hierarchy to edges — is
 *  `graph_core::ingest`'s one derivation, which this package cannot do in JS without
 *  becoming a second copy of it. So a caller maps its source into a contract document (one
 *  of the adapters, or its own) and hands it over, and the motor does the rest.
 *
 *  A document that is not a valid contract is refused with a `ContractRefusedError` — an
 *  unknown member, a role outside the eight, a dangling collection, a `:` in a coordinate
 *  that cannot round-trip — never half-read. A provisional node/edge document is *not* one
 *  of these refusals in spirit: it is simply not a contract, and it is refused as one. */
export const CONTRACT_BUILD: StagedBuild = {
  buffer: "contract",
  call: "gm_build_contract",
  refusal: "gm_build_contract refused the contract document",
  refuse: (message, code) => new ContractRefusedError(message, code),
};


/** One reserved buffer and the length the build export must be handed. `free` gives the
 *  buffer back with the length it was reserved under, which is not `len` on the fallback. */
interface Staged {
  readonly ptr: number;
  readonly len: number;
  free(): void;
}

function reserve(exports: RawExports, spec: StagedBuild, chars: number): number {
  const ptr = invoke("gm_alloc", () => exports.gm_alloc(toU32(chars)));
  if (ptr === 0) throw new BuildRefusedError(`gm_alloc could not reserve the ${spec.buffer} buffer`, lastError(exports));
  return ptr;
}

/** Stages `text` in linear memory, one copy where the text allows it.
 *
 *  A character outside ASCII is more than one byte, so the encoded bytes are longer than the
 *  character count: those bytes do not fit the buffer reserved for the characters, and that
 *  buffer is given back before the encoded array is staged the long way. ASCII — every
 *  document the studio generates, and most a user drops in — fills the reservation in place,
 *  so no second byte array is ever built.
 *
 *  The branch is on `read`, the characters `encodeInto` consumed, and never on `written`, the
 *  bytes it wrote: a truncated encoding still fills the buffer to the brim, so a document
 *  cut at its character count would be staged whole and the motor would read a truncated
 *  document (`packages/graph-studio/tests/staging.motor.test.ts`). */
function stage(exports: RawExports, views: ColumnViews, spec: StagedBuild, text: string): Staged {
  const chars = toU32(text.length);
  let ptr = reserve(exports, spec, chars);
  let len = chars;
  // `exports.memory.buffer` is read per call: `gm_alloc` may have grown the memory.
  const read = encoder.encodeInto(text, new Uint8Array(exports.memory.buffer, ptr, chars)).read;
  if (read !== chars) {
    // No `bump()` for this free: the buffer never held a view, and `free()` below bumps for
    // the reservation that did, so the fallback epoch sequence is the same one as before.
    invoke("gm_free", () => exports.gm_free(ptr, chars));
    const bytes = encoder.encode(text);
    len = toU32(bytes.length);
    ptr = reserve(exports, spec, len);
    new Uint8Array(exports.memory.buffer, ptr, len).set(bytes);
  }
  return {
    ptr,
    len,
    free: () => {
      invoke("gm_free", () => exports.gm_free(ptr, len));
      views.bump();
    },
  };
}

export function buildStaged({ exports, views }: Loaded, text: string, spec: StagedBuild): Handle {
  const staged = stage(exports, views, spec, text);
  try {
    const handle = invoke(spec.call, () => exports[spec.call](staged.ptr, staged.len));
    views.bump();
    if (handle === 0) throw spec.refuse(spec.refusal, lastError(exports));
    return handle as Handle;
  } finally {
    staged.free();
  }
}
