/** Shared by the tests: ingest records from a few members, and the real wasm when it is built. */
import { readFile } from "node:fs/promises";

import type { IngestEdge, IngestNode } from "../src/source/ingest.ts";

export function node(id: string, patch: Partial<IngestNode> = {}): IngestNode {
  return {
    id, kind: "record", database_id: null, source: "test", label: id.toUpperCase(), group: null,
    weight: 0.5, version: 0, has_note: false, icon: null, ...patch,
  };
}

export function edge(id: string, source: string, target: string): IngestEdge {
  return {
    id, source, target, kind: "relation", label: "relation", strength: 0.5,
    directed: false, record_id: null, child_first: false,
  };
}

export const WASM_PATH = new URL("../../../target/wasm32-unknown-unknown/release/graph_wasm.wasm", import.meta.url);

/** `null` when the module is not built: the caller skips, and a skip is not a pass. */
export async function wasmBytes(): Promise<Uint8Array<ArrayBuffer> | null> {
  try {
    const file = await readFile(WASM_PATH);
    const bytes = new Uint8Array(new ArrayBuffer(file.byteLength));
    bytes.set(file);
    return bytes;
  } catch {
    return null;
  }
}
