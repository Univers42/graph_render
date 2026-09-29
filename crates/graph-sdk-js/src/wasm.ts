// The wasm loader (`docs/cheatsheet/wasm_native/wasm-bridge.md`'s pattern, adapted: the
// `osionos/.../bridge.ts:63-86` this phase was originally pointed at no longer exists
// upstream — `docs/contract/wasm-abi.md` "Deviations" records it). A lazy singleton:
// concurrent callers before the first load settles share one in-flight promise (never two
// concurrent compiles), a failed load latches (`initFailed`) so a broken module is never
// silently retried forever, and `__GM_DISABLE_WASM__` is a kill switch checked before any
// `WebAssembly.*` call — set it and this module never touches wasm at all this session.

import { WasmUnavailableError } from "./errors.ts";

/** The raw ABI (`docs/contract/wasm-abi.md`). Every export takes and returns plain `u32`s
 * (D6) — `index.ts` is the only place that turns them into the SDK's typed surface. */
export interface RawExports {
  readonly memory: WebAssembly.Memory;
  gm_alloc(len: number): number;
  gm_free(ptr: number, len: number): void;
  gm_layout_count(): number;
  gm_layout_id(i: number): number;
  gm_build(ingestPtr: number, ingestLen: number): number;
  gm_run(handle: number, layoutId: number, paramsPtr: number, paramsLen: number): number;
  gm_node_count(handle: number): number;
  gm_geometry_kind(handle: number): number;
  gm_edge_geometry_kind(handle: number): number;
  gm_column_ptr(handle: number, columnId: number): number;
  gm_column_len(handle: number, columnId: number): number;
  gm_snapshot_json(handle: number): number;
  gm_snapshot_bytes(handle: number): number;
  gm_release(handle: number): void;
  gm_last_error(): number;
}

/** Bytes, or a URL/`Response` `fetch` can resolve (browser only — Node callers always
 * pass bytes, e.g. `readFile`'d themselves; this module never assumes a filesystem). */
export type WasmSource = BufferSource | string | URL;

let singleton: Promise<RawExports> | null = null;
let initFailed: unknown = null;

/** `>>> 0` (C9): every `number` this SDK sends across the ABI as a `u32` argument goes
 * through this first, so a negative number or a non-integer coerces exactly the way the
 * wire's own `u32` arithmetic would, rather than however `WebAssembly`'s own implicit
 * ToInt32-ish coercion happens to round it. */
export function toU32(value: number): number {
  return value >>> 0;
}

function killSwitchIsOn(): boolean {
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
    if (typeof WebAssembly.instantiateStreaming === "function" && typeof fetch === "function") {
      try {
        const streamed = await WebAssembly.instantiateStreaming(fetch(source), {});
        refuseImports(streamed.module);
        return streamed.instance;
      } catch {
        // Some static hosts serve .wasm as the wrong MIME type, which
        // instantiateStreaming refuses outright; retried below via a plain fetch.
      }
    }
    const response = await fetch(source);
    const bytes = await response.arrayBuffer();
    const { instance, module } = await WebAssembly.instantiate(bytes, {});
    refuseImports(module);
    return instance;
  }
  const { instance, module } = await WebAssembly.instantiate(source, {});
  refuseImports(module);
  return instance;
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
  if (singleton !== null) return singleton;
  singleton = compile(source)
    .then((instance) => instance.exports as unknown as RawExports)
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

/** Clears the singleton and the `initFailed` latch. Test-only: production code has no
 * legitimate reason to load a second, different module into the same session. */
export function resetForTests(): void {
  singleton = null;
  initFailed = null;
}
