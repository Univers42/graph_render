# @graph-motor/sdk-js

Thin JS/TS wrapper over `crates/graph-wasm`'s raw `extern "C"` ABI
(`docs/contract/wasm-abi.md` is the authoritative contract, and
`docs/decisions/force-wasm-abi.md` for the `gm_force_session_*` family; this README is a guide
to the wrapper, not a second copy of either). No wasm-bindgen, no generated glue: every export
takes and returns plain `u32`s — the force session's coordinates and `alpha` excepted, which
are fixed-width `f64`s — and this package's only job is to make that pleasant to call from
JS without ever letting a raw pointer leak past its own methods.

Ships no build, same convention as the repo root: import `src/index.ts` directly (bundle it
yourself, or run it on Node ≥22.6 with `--experimental-strip-types`).

## What this package does not do

A page with only a happy path is marketing, so here is the other half.

**Not rendered.** There is no canvas, no SVG, no WebGL, no hit-testing, no camera, no
animation loop. This package hands you typed arrays over the motor's own memory and gets
out of the way. Choosing a renderer is the consumer's, and it should be: the transport is
zero-copy precisely so it can feed any of them.

**Not a data source.** Nothing here connects to a database, issues a query, listens for
changes, or writes anything back. There is no driver and no ORM, and no `fetch` outside
`./remote` (below), which calls a graph-motor service and nothing else. The adapters
below map a structure **you already have** in memory; getting it is your problem, and
should be, because the shape you can get differs per source and the motor does not care.

**Not a renderer, and not the thing that decides 3D is drawable.** The transport **carries**
3D rather than refusing it: a snapshot's header says `dim` (`0` 2D, `1` 3D), `Motor.layout`
and `Motor.post` report it as `RunResult.dim`/`PostResult.dim`, and a 3D run's depths are
`motor.column(handle, ColumnId.NodeZ)` — `f32`, like `x` and `y`, and `null` for a 2D run
(absent, not a zero-length array). Whether a 3D run can be *drawn* is the consumer's call,
and a 2D-only consumer should decline it explicitly rather than project z away; the
renderer and studio in this repository do, refusing by the name `dimension-3d`. No 3D
**layout** exists yet, so every run this package can produce today is 2D
(`docs/decisions/contract-3d.md` §3).

**Not a mutation or write path.** The motor is pure: topology in, geometry out, no
network, no clock, no randomness beyond seeded generators, and no way to write back to a
source. `docs/decisions/compute-tiers.md` reserves the write path as a separate decision
with consequences outside this repository.

**Not a graph-theory reference.** The ledger (`graph-cli capabilities --json`) is the
list of what is actually implemented, and every row there carries its oracle, its measured
ceiling, what happens past it, and its known approximation. The not-ported list, and why
each is not ported, is in `prompt.md` §10: the Graphviz engines, igraph's
DrL/LGL/Graphopt/Davidson-Harel, SBEB bundling, 3D layouts, `graph-server`, a declarative
mapping DSL, and `SharedArrayBuffer`. Read the ledger rather than this paragraph; the
ledger cannot go stale, and a paragraph can.

**Not a validator.** The ingest contract's reader is in Rust
(`graph_contract::ingest::read`) and its JSON Schema is committed at
`docs/contract/ingest-schema.json`. Use whatever validator you already have if you want
one in JavaScript; shipping a dependency this package does not otherwise need would be
worse than the three lines you would have written.

## Adapters — source shape to the ingest contract

Two adapters ship, deliberately unlike each other:

| Import | Source shape | For |
|---|---|---|
| `@graph-motor/sdk-js/adapters/rows` | named columns, rows of values | the general case |
| `@graph-motor/sdk-js/adapters/notion` | property types, type-tagged values | the vendor-shaped case |

Both are **pure mappings**. They decide what a source's fields *mean* — by reading the
roles a schema declares, or by mapping a vendor's property types onto the eight declared
roles — and write them down. They build no node ids, derive no edges, choose no
strengths and synthesise no tag hubs: that is `graph_core::ingest::build`, once, for
every source. An adapter containing graph logic is the abstraction leaking, and the
convergence fixture below is what would stop.

### The eight roles

`title`, `label`, `group`, `tags`, `link`, `scalar`, `weight`, `parent`. They replace a
twenty-member vendor type enum, and a **declared** role is not a heuristic: the code this
replaces inferred roles from property type strings ("the first `multi_select`, or a field
named `/^tags?$/i/`"), and a wrong inference *silently changes the graph* — no error, no
warning, nothing in the output to detect it. Here the declaration is the thing being read,
so it cannot be wrong in that way.

`typeToRole` in `notion.ts` is the one place a value is chosen rather than declared: a
property type the table does not name becomes `scalar`, which means *declared and read
by nobody*. That is the safe direction — a wrong structural role would add or remove
edges — and the escape hatch is the caller's `roles` override, which is a declaration
and outranks the table.

### The convergence proof

`fixtures/ingest/{rows.json,notion.json}` are the **same logical dataset in two source
shapes**. Both adapters map them to one contract document, byte for byte, and that
document derives one graph, byte for byte. It is a gate row, not a claim:

```sh
node harness/sdk-smoke.mjs --adapter-convergence     # the adapters agree
cargo test -p graph-core ingest                      # the derivation is what is pinned
```

`fixtures/ingest/expected-graph.json` holds both halves — the contract document and the
graph derived from it — so the two runtimes are pinned to one artifact rather than to two
that can drift.

### A refusal, not a guess

Both adapters refuse with a dotted path rather than guessing: a cell naming an
undeclared column, a `link` column with no target, a property type with no role, a
timestamp that is not RFC 3339. Each would otherwise become a well-formed graph with
nothing in it to show the mistake. `RowsAdapterError` carries `path` and `what`; the
message is `${path}: ${what}`.

### The id grammar (H5)

A derived node id is `source:collection:record`, and the grammar **cannot represent `:`**
inside `source` or a collection id: the parse comes back *shifted and wrong*, not
`null`. The contract's reader therefore refuses such a coordinate by name, and the
derivation refuses a `:` in a tag value for the same reason. A **record id** may contain
`:` freely — it is the last segment, and a test pins the round trip. Broadening the
grammar would move every existing node id, and a node id that moves is a layout that
moves, so that is a decision with consequences outside this repository and is not taken
here.

## Provisional node/edge ingest

`Motor#build` takes the **provisional** node/edge JSON `crates/graph-wasm/src/ingest.rs`
documents — a versioned array of records in `graph_core::records`' own shape. Every field
is named and required; a `null` where a field may be absent, never an omitted key; an
unknown member (a stray camelCase `hasNote`, say) refuses the whole document rather than
being silently dropped.

The role-based contract above is the front of the pipeline; this provisional shape is
what the wasm ABI still takes. `graph-cli ingest` runs the derivation from one to the
other, which is how you can see the boundary without writing Rust.

## Reading the output with no SDK at all

The JSON face is a contract, not a private format, and that is the promise worth keeping.
`harness/read-snapshot-raw.mjs` reads a real snapshot with `JSON.parse` and nothing else —
no import from this package, no wasm — and checks the committed schema against it. It is
also a gate row. `EXAMPLES.md` §4 has the reader written out.


## Layouts

`Motor#layouts()` returns every layout id the loaded module registered, in registry order,
read from `gm_layout_count`/`gm_layout_id` (C1) and cached for the motor's lifetime.
`Motor#layout(id, …)` resolves the id you hand it through that same map, so the id is never
a hard-coded index and a layout registered after this package was written is reachable and
discoverable with no change here. This is the surface a consumer should enumerate rather
than a list of names copied out of a release note.

```js
for (const id of motor.layouts()) {
  const run = motor.layout(handle, id);
  console.log(id, run.nodeCount, run.nodeKind, run.edgeKind);
}
```

A degraded motor (see below) refuses `layouts()` the way it refuses every other method that
needs the module. It never answers `[]`: "this module has no layouts" and "this module never
loaded" are different facts, and only one of them is true.

## Live force sessions

`Motor#layout` runs a layout to a finished picture. `Motor#forceSession` starts the other
thing the motor owns: **a live force simulation the caller drives**, tick by tick
(`docs/decisions/force-wasm-abi.md`).

```js
const session = motor.forceSession(handle, { gravity: 0.2 });
session.tick(4);                          // { status: "running", alpha, ticksRun }
session.drag(0, 500, -500);               // pin node row 0 there, from the next tick on
session.reheat(1.0);
session.tick(30);
const { xs, ys } = session.positions();   // zero-copy Float64Arrays, one entry per node
session.unpinAll();
session.release();
```

Four things worth knowing before you build on it:

- **`positions()` is `Float64Array`, not `Float32Array`.** Every other column here is `f32`
  because every other column is a *snapshot* column. These two are the simulation's own
  state, which the next tick reads back. Copy them (`.slice()`) before transferring them to a
  worker, exactly as with a column view.
- **A verb moves nothing on its own.** `pin`/`drag`/`reheat` arm the next tick; read the
  effect after `tick()`, not before.
- **Parameters are partial and never clamped.** `setParams({ theta })` keeps the motor's own
  value for the other twelve (read back through `params()`), and a value outside its range is
  a `ForceSessionRefusedError` with the session untouched.
- **A session outlives its graph handle**, and has its own id space: `motor.release(handle)`
  leaves the session running, and `session.release()` is a separate call. A released session
  says `released` and refuses every method rather than answering emptily.

## Ownership

| What | Who owns it | Valid until |
|---|---|---|
| The ingest buffer (`gm_alloc`'d) | The caller | `Motor#build` always frees it itself, even on refusal — nothing above this package ever sees the pointer |
| A framed return buffer (JSON/bytes/layout id) | The motor | The next motor call, on *any* handle — copied out (`.slice()`) before this package's methods return, so a caller never touches wasm memory directly for these |
| A column view (`Motor#column`) | The motor | The next motor call, on *any* handle (C7) — this package re-derives it lazily via an epoch counter (`views.ts`), but does not stop a caller from reading a JS reference to an old typed array after that; don't hold one past the next call |
| A handle | The motor's handle table | `Motor#release`; the id is never reissued (C6) |
| A force session | the motor's session table | `ForceSession#release`; its own id space, never reissued |
| A force session's position view | the session's own columns | the session's life — the address does not move, but copy before transferring, as above |
| A staged parameter buffer | this package | `setParams` frees it itself, on a refusal too; the caller never sees the pointer |

Column views are typed-array aliases over the module's own memory, valid until the next
motor call on any handle. Read `x`/`y` (and `r` for Circle nodes, `w`/`h` for Box nodes,
`offsets`/`pts` for Polyline and Curve edges, `degree` for Curve) with
`Motor#column(handle, ColumnId.NodeR)` and friends: which column ids exist for a given run
is decided by that run's node/edge geometry kind, and an id that does not apply reads `null`
rather than an empty array.

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
`toJSON`/`toBytes`/`release`/`forceSession` throw `WasmUnavailableError` predictably the
first time one is actually called — never a silent no-op, never fabricated data, and never at
load time itself (a motor that throws on load takes the host page down with it). A load that
fails on its own latches the same way for the rest of the session — `resetForTests()`
(test-only) clears it.

## `options`

This phase, `createMotor(source, options)` accepts only `{}` or `{ exec: "auto" }`
(`docs/decisions/compute-tiers.md` reserves `exec` for Phase 11's compute tiers). Any other
key, or any other `exec` value, is refused before the module is even asked to load —
`options` is never silently ignored.

## Remote

`@graph-motor/sdk-js/remote` (`src/remote.ts`) calls the graph-motor HTTP service
(`docs/contract/service-api.md`) instead of a local `Motor`. The service sends the binary face
by default. The SDK's own snapshot reader decodes it, so `column` answers with the same
`ColumnId`s and typed arrays that `Motor#column` does for the same run.

This example comes from the passing test `README: a remote layout reads like a local one`
(`test/remote.test.mjs`). In that test, `service` is the fake service, `key` is a key generated per run,
and `document` is a fixture's ingest document:

```js
const remote = createRemote({ baseUrl: "http://graph.test", apiKey: key, fetch: service.fetch });
const meta = await remote.meta();
const snapshot = await remote.layout(document, { layout: "layout.tree.tidy" });
const x = snapshot.column(ColumnId.NodeX); // Float32Array, one entry per node, in nodeIds order
```

Against a real service, omit `fetch` and the platform's `globalThis.fetch` is used.

- **`createRemote({ baseUrl, apiKey?, fetch?, dangerouslyAllowBrowser? })`.**
  - `baseUrl` is `http:` or `https:`. It may carry a path prefix, and a trailing slash is
    ignored. It is refused if it carries credentials, a query or a fragment.
  - `apiKey` is sent as `Authorization: Bearer <key>`, and never in a URL.
  - An unknown option, or a key that is not an RFC 6750 bearer token, throws
    `InvalidOptionsError` before any request is made.
- **`meta()`.** Returns `{ api, abi, version, layouts, posts }`. Fields a newer service adds are
  dropped.
- **`layout(doc, { layout, post?, source?, format? })`.**
  - `doc` is a JSON string or an object.
  - `post` takes one id: the service runs at most one post. `"a,b"`, an array, or `""` is
    refused before any request. To chain posts, use a local `Motor`.
  - `source: "contract"` sends the ingest contract document instead of the provisional
    ingest.
  - `format: "json"` returns the canonical JSON face, parsed, instead of a decoded snapshot.

### Errors

Every failure is a `RemoteError`, which is a `GraphMotorError`.

- `status` is the HTTP status. It is `0` when no response arrived, and `200` when the response
  was in the wrong face or could not be read.
- `codeName` is the body's `error` name, typed: either the motor's `Code` names
  (`CODE_NAMES`, for example `UnknownLayoutId`, `IngestInvalid` or `IngestTooLarge`), or
  the service's own names (`SERVICE_ERROR_NAMES`: `BadRequest`, `Unauthorized`, `NotFound`,
  `NotAcceptable`, `Busy`, `Internal`, `Timeout`).
- `code` is the wire value for a motor name, and `undefined` for a service name.
- A status with no error body, such as a proxy's HTML page, or a name the SDK does not type,
  gives `codeName` `undefined`. The message still quotes an unknown name. It never quotes
  the body.
- `retryAfter` is the whole seconds of `Retry-After` on any refusal that sends one (a 429
  does). It is `undefined` otherwise, including for the HTTP-date form.
- No message carries the URL, a header, or the key. If the server's own message contains
  the key, the key is cut out.

### What it does not do

- **No retries and no backoff.** `retryAfter` is the caller's input.
- **No key in a browser.** With an `apiKey`, `createRemote` throws when it sees `window`,
  `document` or `WorkerGlobalScope`, because the key would ship to every visitor. Call the service
  from a server, or pass `dangerouslyAllowBrowser: true` if you accept that.
- **No redirects.** A redirect is a failure (`redirect: "error"`), so the key never follows
  one to another host.
- **No `fetch` anywhere else in the SDK.** Only `./remote` touches the network.

### Live check

`scripts/live-check.mjs` runs four checks against a running service. Build the release wasm first:
`scripts/orch/gr cargo build -p graph-wasm --release --target wasm32-unknown-unknown`.

```sh
GRAPH_API_KEY=<key> node --experimental-strip-types crates/graph-sdk-js/scripts/live-check.mjs <baseUrl>
```

The checks:

- `meta()` lists the layouts and posts that the parity cases use.
- Each parity case matches the local wasm `Motor`, column for column.
- A wrong key gets the typed 401 `Unauthorized`.
- `post=a,b` gets a typed 400.

It prints one `PASS`/`FAIL` line per check and never prints the key. Exit codes:

- 0: every check passed.
- 1: a check failed.
- 2: it could not run (no `baseUrl`, no key, no wasm build, or no response).
