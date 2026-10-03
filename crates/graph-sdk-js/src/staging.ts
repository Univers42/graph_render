// Stages a document through `gm_alloc` and frees it again (C7: the staging buffer is the
// SDK's job, not the caller's, since the caller never sees the pointer). Shared by the three
// build paths, which differ only in the export they call, the payload they carry and the
// error class a refusal takes.

import { toU32 } from "./wasm.ts";
import { BuildRefusedError } from "./errors.ts";
import { encoder, invoke, lastError, type Loaded } from "./calls.ts";
import type { Handle } from "./types.ts";

export interface StagedBuild {
  /** Names the buffer in the `gm_alloc` refusal: "ingest", "contract" or "columns". */
  buffer: string;
  call: "gm_build" | "gm_build_contract" | "gm_build_columns";
  /** The message and error class of the build export's own refusal. */
  refusal: string;
  refuse: (message: string, code: number) => Error;
}

export function buildStaged(
  { exports, views }: Loaded,
  payload: string | Uint8Array,
  spec: StagedBuild,
): Handle {
  // A columnar document arrives already encoded (`encodeColumns`), so it is copied through
  // unchanged; the two text paths go through `TextEncoder` first.
  const bytes = typeof payload === "string" ? encoder.encode(payload) : payload;
  const len = toU32(bytes.length);
  const ptr = invoke("gm_alloc", () => exports.gm_alloc(len));
  if (ptr === 0) throw new BuildRefusedError(`gm_alloc could not reserve the ${spec.buffer} buffer`, lastError(exports));
  try {
    new Uint8Array(exports.memory.buffer, ptr, len).set(bytes);
    const handle = invoke(spec.call, () => exports[spec.call](ptr, len));
    views.bump();
    if (handle === 0) throw spec.refuse(spec.refusal, lastError(exports));
    return handle as Handle;
  } finally {
    invoke("gm_free", () => exports.gm_free(ptr, len));
    views.bump();
  }
}
