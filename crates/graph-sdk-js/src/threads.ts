// The threads artifact (`scripts/orch/wasm-threads.sh`) as a motor: one shared
// WebAssembly.Memory, this thread as the pool's coordinator, and `helpers` more instances of the
// same module, one per Worker the caller spawns, parked in `gm_thread_serve`. Only the live tick
// is threaded (`gm_force_session_tick_threaded`); every other call is the serial export, and the
// tick's bytes equal the serial tick's at any worker count (`docs/measurements/perf-p3-session-threads.md`).
//
// The coordinator blocks in `memory.atomic.wait` while its helpers run, which a page's main
// thread may not do: load this only inside a Worker. The caller owns the Workers because only
// it knows how its bundler names a worker script; `serveHelper` is the whole helper body.

import { killSwitchIsOn, requireExports, toU32, type RawExports, type WasmSource } from "./wasm.ts";
import { WasmUnavailableError } from "./errors.ts";

/** What one helper needs, all structured-cloneable: post it to the helper's Worker. */
export interface HelperStart {
  readonly module: WebAssembly.Module;
  readonly memory: WebAssembly.Memory;
  /** The top of its 16-aligned stack: wasm stacks grow down. */
  readonly stackTop: number;
  readonly tls: number;
}

export interface MotorThreads {
  /** Helper threads beside the caller's own; 0 runs the threads artifact serially. */
  readonly helpers: number;
  /** Starts one Worker that calls {@link serveHelper} with `start`. */
  readonly spawn: (start: HelperStart) => void;
  /** The pool's negative control, for tests only: bit 0 makes the last part write nothing. */
  readonly flags?: number;
}

interface ThreadExports {
  gm_thread_block(len: number): number;
  gm_thread_count(): number;
  gm_force_session_tick_threaded(session: number, ticks: number, workers: number, flags: number): number;
  readonly __tls_size: WebAssembly.Global;
  readonly __tls_align: WebAssembly.Global;
}

const STACK = 4 << 20;
const INITIAL_PAGES = 1024; // --initial-memory=67108864 in scripts/orch/wasm-threads.sh
const MAX_PAGES = 65536;
const JOIN_POLL_MS = 2;
/**
 * Ponytail: helpers that have not joined after this long are left to join later. The pool
 * clamps each pass to the helpers that have joined, so the bytes never depend on it. Failing
 * input: a Worker that never starts (a blocked script URL) costs this wait once per load, and
 * its share of every tick runs on the coordinator. Direction: slower, never wrong.
 */
const JOIN_WAIT_MS = 5000;

const THREAD_NAMES = ["gm_thread_block", "gm_thread_count", "gm_force_session_tick_threaded", "__tls_size", "__tls_align"];

function threadExports(exports: WebAssembly.Exports): ThreadExports {
  const missing = THREAD_NAMES.filter((name) => !(name in exports));
  if (missing.length > 0) throw new Error(`not the threads artifact: it lacks ${missing.join(", ")}`);
  return exports as unknown as ThreadExports;
}

/** The threads artifact imports exactly its shared memory, and nothing else. */
function requireMemoryImport(module: WebAssembly.Module): void {
  const imports = WebAssembly.Module.imports(module).map((i) => `${i.module}.${i.name}:${i.kind}`);
  if (imports.length !== 1 || imports[0] !== "env.memory:memory") {
    throw new Error(`the threads artifact must import only env.memory, it imports ${imports.join(", ") || "nothing"}`);
  }
}

async function compileFrom(source: WasmSource): Promise<WebAssembly.Module> {
  if (typeof source === "string" || source instanceof URL) {
    const response = await fetch(source);
    if (!response.ok) throw new Error(`${String(source)}: HTTP ${response.status}`);
    return WebAssembly.compile(await response.arrayBuffer());
  }
  return WebAssembly.compile(source);
}

/** Every block is taken before any helper starts: a refusal halfway must not strand a helper parked on a dropped memory. */
function startHelpers(raw: ThreadExports, start: Omit<HelperStart, "stackTop" | "tls">, threads: MotorThreads): void {
  if (Number(raw.__tls_align.value) > 16) throw new Error(`TLS alignment ${String(raw.__tls_align.value)} exceeds gm_thread_block's 16`);
  const tlsSize = Math.max(Number(raw.__tls_size.value), 16);
  const starts: HelperStart[] = [];
  for (let i = 0; i < threads.helpers; i++) {
    const stack = raw.gm_thread_block(STACK) >>> 0;
    const tls = raw.gm_thread_block(tlsSize) >>> 0;
    if (stack === 0 || tls === 0) throw new Error("gm_thread_block refused a helper's stack or TLS");
    starts.push({ ...start, stackTop: (stack + STACK) & ~15, tls });
  }
  for (const helper of starts) threads.spawn(helper);
}

/** Yields to the event loop while it waits: a Worker spawned here starts only after that. */
async function awaitJoins(raw: ThreadExports, helpers: number): Promise<void> {
  for (let waited = 0; raw.gm_thread_count() < helpers && waited < JOIN_WAIT_MS; waited += JOIN_POLL_MS) {
    await new Promise((resolve) => setTimeout(resolve, JOIN_POLL_MS));
  }
}

/**
 * Loads the threads artifact, starts its helpers and returns the serial ABI with the live tick
 * swapped for the threaded one. Same status word, so `ForceSession` cannot tell them apart.
 * Rejects with {@link WasmUnavailableError}; the caller falls back to {@link loadMotor}.
 */
export async function loadThreaded(source: WasmSource, threads: MotorThreads): Promise<RawExports> {
  if (killSwitchIsOn()) throw new WasmUnavailableError("__GM_DISABLE_WASM__ is set: the motor will not load this session");
  try {
    const module = await compileFrom(source);
    requireMemoryImport(module);
    const memory = new WebAssembly.Memory({ initial: INITIAL_PAGES, maximum: MAX_PAGES, shared: true });
    const instance = await WebAssembly.instantiate(module, { env: { memory } });
    const exports = requireExports(instance, memory);
    const raw = threadExports(instance.exports);
    startHelpers(raw, { module, memory }, threads);
    await awaitJoins(raw, threads.helpers);
    const workers = toU32(threads.helpers + 1);
    const flags = toU32(threads.flags ?? 0);
    const tick = (session: number, ticks: number): number => raw.gm_force_session_tick_threaded(session, ticks, workers, flags) >>> 0;
    return { ...exports, gm_force_session_tick: tick };
  } catch (error) {
    throw new WasmUnavailableError("the threads artifact failed to load", error);
  }
}

/** A helper Worker's whole body: returns only when the coordinator closes the pool. */
export function serveHelper(start: HelperStart): void {
  const { exports } = new WebAssembly.Instance(start.module, { env: { memory: start.memory } });
  const stackPointer = exports.__stack_pointer;
  const initTls = exports.__wasm_init_tls;
  const serve = exports.gm_thread_serve;
  if (!(stackPointer instanceof WebAssembly.Global) || typeof initTls !== "function" || typeof serve !== "function") {
    throw new Error("not the threads artifact: it lacks __stack_pointer, __wasm_init_tls or gm_thread_serve");
  }
  stackPointer.value = start.stackTop;
  initTls(start.tls);
  serve();
}
