// The wasm loader (`docs/cheatsheet/wasm_native/wasm-bridge.md`'s pattern, adapted: the
// `osionos/.../bridge.ts:63-86` this phase was originally pointed at no longer exists
// upstream — `docs/contract/wasm-abi.md` "Deviations" records it). A lazy singleton:
// concurrent callers before the first load settles share one in-flight promise (never two
// concurrent compiles), a failed load latches (`initFailed`) so a broken module is never
// silently retried forever, and `__GM_DISABLE_WASM__` is a kill switch checked before any
// `WebAssembly.*` call — set it and this module never touches wasm at all this session.

import { WasmUnavailableError } from "./errors.ts";

/** The raw ABI (`docs/contract/wasm-abi.md`, and `docs/decisions/force-wasm-abi.md` for the
 *  `gm_force_session_*` family). Every export takes and returns plain `u32`s
 * (D6) — `index.ts` is the only
 *  place that turns them into the SDK's typed surface; `force.ts` does the same for the force
 *  session, whose `f64` values are fixed-width IEEE-754 rather than pointer-width. */
export interface RawExports {
  readonly memory: WebAssembly.Memory;
  gm_abi_version(): number;
  gm_alloc(len: number): number;
  gm_free(ptr: number, len: number): void;
  gm_layout_count(): number;
  gm_layout_id(i: number): number;
  gm_layout_params(i: number): number;
  gm_build(ingestPtr: number, ingestLen: number): number;
  gm_build_contract(contractPtr: number, contractLen: number): number;
  gm_build_columns(columnsPtr: number, columnsLen: number): number;
  gm_run(handle: number, layoutId: number, paramsPtr: number, paramsLen: number): number;
  gm_node_count(handle: number): number;
  gm_geometry_kind(handle: number): number;
  gm_edge_geometry_kind(handle: number): number;
  gm_dim(handle: number): number;
  gm_column_ptr(handle: number, columnId: number): number;
  gm_column_len(handle: number, columnId: number): number;
  gm_snapshot_json(handle: number): number;
  gm_snapshot_bytes(handle: number): number;
  gm_post_count(): number;
  gm_post_id(i: number): number;
  gm_post_run(handle: number, postIndex: number): number;
  gm_analysis_count(): number;
  gm_analysis_id(i: number): number;
  gm_analysis_run(handle: number, index: number): number;
  gm_release(handle: number): void;
  gm_last_error(): number;
  gm_force_session_create(graph: number, paramsPtr: number, paramsLen: number): number;
  gm_force_session_create_mesh(graph: number, paramsPtr: number, paramsLen: number): number;
  gm_force_session_set_params(session: number, paramsPtr: number, paramsLen: number): number;
  gm_force_session_params(session: number): number;
  gm_force_session_tick(session: number, ticks: number): number;
  gm_force_session_alpha(session: number): number;
  gm_force_session_reheat(session: number, alpha: number): number;
  gm_force_session_pin(session: number, row: number, x: number, y: number): number;
  gm_force_session_unpin(session: number, row: number): number;
  gm_force_session_unpin_all(session: number): number;
  gm_force_session_column_ptr(session: number, axis: number): number;
  gm_force_session_column_len(session: number, axis: number): number;
  gm_force_session_release(session: number): number;
}

/** Every name in [`RawExports`], checked at load: the compiler keeps this object's keys equal to
 * the interface's, so a module older than this SDK is refused by name when it loads instead of
 * failing later as `exports.gm_dim is not a function` on the first call that needs it. */
const EXPORT_NAMES: { readonly [K in keyof RawExports]: true } = {
  memory: true, gm_abi_version: true, gm_alloc: true, gm_free: true, gm_layout_count: true,
  gm_layout_id: true, gm_layout_params: true,
  gm_build: true, gm_build_contract: true, gm_build_columns: true, gm_run: true, gm_node_count: true,
  gm_geometry_kind: true, gm_edge_geometry_kind: true, gm_dim: true, gm_column_ptr: true,
  gm_column_len: true, gm_snapshot_json: true, gm_snapshot_bytes: true, gm_post_count: true,
  gm_post_id: true, gm_post_run: true, gm_analysis_count: true, gm_analysis_id: true,
  gm_analysis_run: true, gm_release: true, gm_last_error: true,
  gm_force_session_create: true, gm_force_session_create_mesh: true, gm_force_session_set_params: true,
  gm_force_session_params: true, gm_force_session_tick: true, gm_force_session_alpha: true,
  gm_force_session_reheat: true, gm_force_session_pin: true, gm_force_session_unpin: true,
  gm_force_session_unpin_all: true, gm_force_session_column_ptr: true,
  gm_force_session_column_len: true, gm_force_session_release: true,
};

/** The ABI revision this SDK speaks: `gm_abi_version()` must return exactly this
 * (`docs/contract/wasm-abi.md` "Exports").
 *
 *  `2` since `gm_run`'s `params_ptr`/`params_len` stopped being refused and started
 *  carrying a layout's published parameters, and `Code::ParamsMustBeEmpty` stopped being
 *  produced. A module from before it is refused by name at load (`gm_layout_params` is in
 *  `EXPORT_NAMES`), which degrades the whole motor rather than only its parameters — the
 *  price of an SDK that reads one module's schemas instead of guessing at them. */
export const ABI_VERSION = 2;

/** `instance`'s exports checked against this SDK. `memory` is given when the module imports
 * its memory instead of exporting it (the threads artifact, `threads.ts`). */
export function requireExports(instance: WebAssembly.Instance, memory?: WebAssembly.Memory): RawExports {
  const raw = memory === undefined ? instance.exports : { ...instance.exports, memory };
  const missing = Object.keys(EXPORT_NAMES).filter((name) => !(name in raw));
  if (missing.length > 0) {
    throw new Error(`module lacks ${missing.join(", ")}: it is older than this SDK; rebuild it`);
  }
  const exports = unsignedResults(raw);
  const reported = exports.gm_abi_version();
  if (reported !== ABI_VERSION) {
    throw new Error(`module speaks ABI version ${reported}, this SDK speaks ${ABI_VERSION}: build both from one tree`);
  }
  return exports;
}

/** The exports whose result is not a `u32`: the memory, and the force session's `f64` alpha. */
const NOT_U32: ReadonlySet<string> = new Set(["memory", "gm_force_session_alpha"]);

/** Every `u32` result read back through `>>> 0`, once, here. A wasm `i32` result reaches JS
 * signed, so an address at or past 2 GiB arrived negative: a 1M-node studio load failed with
 * "Offset is outside the bounds of the DataView" in `frame` (2026-10-01). */
function unsignedResults(exports: WebAssembly.Exports): RawExports {
  const out: Record<string, unknown> = {};
  for (const [name, value] of Object.entries(exports)) {
    const wrap = typeof value === "function" && !NOT_U32.has(name);
    out[name] = wrap ? (...args: number[]): unknown => {
      const result: unknown = value(...args);
      return typeof result === "number" ? result >>> 0 : result;
    } : value;
  }
  return out as unknown as RawExports;
}

/** Bytes, or a URL/`Response` `fetch` can resolve (browser only — Node callers always
 * pass bytes, e.g. `readFile`'d themselves; this module never assumes a filesystem). */
export type WasmSource = BufferSource | string | URL;

let singleton: Promise<RawExports> | null = null;
/** The {@link sourceKey} of the source `singleton` was built from, so a later caller asking for
 *  a different module is told rather than silently handed this one. */
let singletonKey: string | BufferSource | null = null;
let initFailed: unknown = null;

/** `>>> 0` (C9): every `number` this SDK sends across the ABI as a `u32` argument goes
 * through this first, so a negative number or a non-integer coerces exactly the way the
 * wire's own `u32` arithmetic would, rather than however `WebAssembly`'s own implicit
 * ToInt32-ish coercion happens to round it. */
export function toU32(value: number): number {
  return value >>> 0;
}

export function killSwitchIsOn(): boolean {
  return (globalThis as { __GM_DISABLE_WASM__?: boolean }).__GM_DISABLE_WASM__ === true;
}

/** No import object: the shipped module imports nothing (`prompt.md` §3.2), so a module
 * that does import something is refused here — the same check `harness/wasm-run.mjs`
 * makes for the gate — rather than left for `WebAssembly.instantiate` to reject less
 * legibly, or worse, resolved against an empty import object and traps on first call. */
function refuseImports(module: WebAssembly.Module): void {
  const imports = WebAssembly.Module.imports(module);
  if (imports.length > 0) {
    throw new Error(`module imports ${imports.map((i) => `${i.module}.${i.name}`).join(", ")}`);
  }
}

async function compile(source: WasmSource): Promise<WebAssembly.Instance> {
  if (typeof source === "string" || source instanceof URL) {
    const streamed = await streamedInstance(source);
    if (streamed !== null) {
      // Outside `streamedInstance`'s `catch`: a refusal thrown here must not be retried into
      // the plain fetch below, where the same module fails to link against the empty import
      // object and the report names only its module, never the field.
      refuseImports(streamed.module);
      return streamed.instance;
    }
    const response = await fetch(source);
    return instantiateBytes(await response.arrayBuffer());
  }
  return instantiateBytes(source);
}

/** The streaming path, or `null` when it cannot be used: some static hosts serve `.wasm` as
 *  the wrong MIME type, which `instantiateStreaming` refuses outright, and the caller then
 *  retries through a plain fetch. Its `catch` returns `null` rather than rethrowing, so it
 *  carries transport failures only — nothing the loader itself refuses is ever swallowed. */
async function streamedInstance(
  source: string | URL,
): Promise<{ instance: WebAssembly.Instance; module: WebAssembly.Module } | null> {
  if (typeof WebAssembly.instantiateStreaming !== "function" || typeof fetch !== "function") {
    return null;
  }
  try {
    const streamed = await WebAssembly.instantiateStreaming(fetch(source), {});
    return { instance: streamed.instance, module: streamed.module };
  } catch {
    return null;
  }
}

/** Bytes to an instance, with {@link refuseImports} between compiling and linking: the two
 *  are separate steps, so a module carrying an import is refused by name here instead of
 *  failing to link and reporting only `Import #0 module="m"` — the field, the part a caller
 *  can act on, is lost. For a module that imports nothing the two are the same work. */
async function instantiateBytes(bytes: BufferSource): Promise<WebAssembly.Instance> {
  const module = await WebAssembly.compile(bytes);
  refuseImports(module);
  return await WebAssembly.instantiate(module, {});
}

/** How the singleton decides that two calls asked for the same module: a `string` is itself,
 *  a `URL` is its `href` — the two spellings of one location agree — and a `BufferSource` is
 *  compared by object identity. Two calls holding equal bytes are therefore two calls: proving
 *  they are equal would mean hashing every load, a cost no caller asked for and no contract
 *  promises. The cost of that choice is a caller that re-reads the file per call is refused on
 *  the second one and must hold the bytes; `resetForTests()` is the documented way out. */
function sourceKey(source: WasmSource): string | BufferSource {
  if (typeof source === "string") return source;
  if (source instanceof URL) return source.href;
  return source;
}

/** A key as it appears in a refusal: the string keys print as themselves, a `BufferSource`
 *  as its length, because a caller holding two distinct copies of one module would otherwise
 *  be told only that two opaque objects disagree. */
function describeKey(key: string | BufferSource): string {
  return typeof key === "string" ? `"${key}"` : `<${key.byteLength} bytes>`;
}

/** Loads the motor once per session and returns its raw exports, sharing one promise
 * across every concurrent caller. Rejects with [`WasmUnavailableError`] — never a bare
 * compile error, never a silent `undefined` — if the kill switch is set, a previous load
 * already failed, the module imports anything, or compiling/instantiating itself throws. */
export async function loadMotor(source: WasmSource): Promise<RawExports> {
  if (killSwitchIsOn()) {
    throw new WasmUnavailableError("__GM_DISABLE_WASM__ is set: the motor will not load this session");
  }
  // Ponytail: the latch below never retries for the process lifetime. Failing input: a
  // transient failure on the *first* load (a flaky network, a momentarily-offline CDN) —
  // a network that recovers a second later is never exploited automatically. Direction:
  // deliberately the strict one (never-retry, not retry-forever): a caller sees a stable
  // "unavailable" rather than an unbounded pile of retries against a host that is down.
  // Escape hatch: reload the page, or the test-only resetForTests() below.
  if (initFailed !== null) {
    throw new WasmUnavailableError("a previous load already failed this session; call resetForTests() to retry", initFailed);
  }
  const key = sourceKey(source);
  if (singleton !== null) {
    // One module per session. Handing a second source's caller the first source's exports
    // would bind it to a module it never asked for, with nothing said; naming both is the
    // only way a caller can tell which of its two modules is live.
    if (singletonKey === null || Object.is(singletonKey, key)) return singleton;
    throw new WasmUnavailableError(
      `a different wasm module is already loaded this session: already loaded ${describeKey(singletonKey)}, ` +
        `asked for ${describeKey(key)}; call resetForTests() to load the new one`,
    );
  }
  singletonKey = key;
  singleton = compile(source)
    .then((instance) => requireExports(instance))
    .catch((error: unknown) => {
      initFailed = error;
      singleton = null;
      // Warn, don't swallow: a caller polling `catch` still sees the rejection, but a
      // caller that only checks `layout` support after a fire-and-forget `createMotor`
      // still learns the module degraded, not just silently got no layouts ever again.
      console.error("graph-motor: wasm module failed to load; layout support is unavailable this session", error);
      throw new WasmUnavailableError("wasm module failed to load", error);
    });
  return singleton;
}

/** Clears the singleton, the key it was built from, and the `initFailed` latch. Test-only:
 * production code has no legitimate reason to load a second, different module into the same
 * session. */
export function resetForTests(): void {
  singleton = null;
  singletonKey = null;
  initFailed = null;
}
