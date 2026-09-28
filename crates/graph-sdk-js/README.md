# @graph-motor/sdk-js

Thin JS/TS wrapper over `crates/graph-wasm`'s raw `extern "C"` ABI
(`docs/contract/wasm-abi.md` is the authoritative contract; this README is a guide to the
wrapper, not a second copy of it). No wasm-bindgen, no generated glue: every export takes
and returns plain `u32`s, and this package's only job is to make that pleasant to call from
JS without ever letting a raw pointer leak past its own methods.

Ships no build, same convention as the repo root: import `src/index.ts` directly (bundle it
yourself, or run it on Node ≥22.6 with `--experimental-strip-types`).

## Provisional ingest

`Motor#build` takes the **provisional** ingest JSON `crates/graph-wasm/src/ingest.rs`
documents — a versioned array of node/edge records in `graph_core::records`' own shape.
**Phase 10 owns the real ingest contract.** Every field is named and required; a `null`
where a field may be absent, never an omitted key; an unknown member (a stray camelCase
`hasNote`, say) refuses the whole document rather than being silently dropped.

## Ownership

| What | Who owns it | Valid until |
|---|---|---|
| The ingest buffer (`gm_alloc`'d) | The caller | `Motor#build` always frees it itself, even on refusal — nothing above this package ever sees the pointer |
| A framed return buffer (JSON/bytes/layout id) | The motor | The next motor call, on *any* handle — copied out (`.slice()`) before this package's methods return, so a caller never touches wasm memory directly for these |
| A column view (`Motor#column`) | The motor | The next motor call, on *any* handle (C7) — this package re-derives it lazily via an epoch counter (`views.ts`), but does not stop a caller from reading a JS reference to an old typed array after that; don't hold one past the next call |
| A handle | The motor's handle table | `Motor#release`; the id is never reissued (C6) |

## Errors

Every refusal is a `GraphMotorError` subclass with a stable `code`/`codeName` mirroring
`crates/graph-wasm/src/errors.rs::Code` — never a bare string, never a raw
`WebAssembly.RuntimeError`. See `src/errors.ts`.

## Zero-copy columns and the epoch

`Motor#column` returns a typed array aliasing the motor's own `WebAssembly.Memory`
directly — writing through it writes through the motor's own buffer. Reading it back after
a tamper is not refused by `column` itself (nothing on that path re-validates); it is
refused the next time you ask the motor to encode a face (`toJSON`/`toBytes`), which
re-checks every value is finite before writing it out (D9) and throws
`TamperedGeometryError` if it is not.

Every mutating call bumps `Motor#epoch`. A column view fetched before that point is never
silently reused after it, even if the pointer, length, and backing buffer all still happen
to match — and if the module's memory grew in between, the backing `ArrayBuffer` itself
was replaced, which alone forces a fresh view regardless of the epoch.

## Kill switch and load failure

Set `globalThis.__GM_DISABLE_WASM__ = true` before the first `createMotor` call to refuse
loading the module at all this session. `createMotor` itself never throws for this, or for
any other load failure (a bad `.wasm`, a network error): it resolves to a *degraded*
`Motor` whose `available` getter reads `false`, and whose `build`/`layout`/`column`/
`toJSON`/`toBytes`/`release` throw `WasmUnavailableError` predictably the first time one
is actually called — never a silent no-op, never fabricated data, and never at load time
itself (a motor that throws on load takes the host page down with it). A load that fails
on its own latches the same way for the rest of the session — `resetForTests()`
(test-only) clears it.

## `options`

This phase, `createMotor(source, options)` accepts only `{}` or `{ exec: "auto" }`
(`docs/decisions/compute-tiers.md` reserves `exec` for Phase 11's compute tiers). Any other
key, or any other `exec` value, is refused before the module is even asked to load —
`options` is never silently ignored.
