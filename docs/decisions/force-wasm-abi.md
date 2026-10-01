# The force session's wasm ABI (`gm_force_session_*`)

Status: decided (M2). `crates/graph-wasm/src/exports/session.rs` implements it,
`crates/graph-wasm/src/session.rs` holds everything behind it, and
`docs/decisions/live-force-session.md` is the physics contract this adds a wire to.

`sim` (M1) landed `ForceSession`: one tick, pins, a reheat, `xs`/`ys`, `StepReport`. What was
missing was a way to *call* it from a worker — `studio-force`'s panel is visible and disabled,
and says why in the source: `"live forces need the motor session (force-wasm)"`. This is that
ABI: a table of `extern "C"` exports, no wasm-bindgen, the same `u32` handle discipline as the
rest of `docs/contract/wasm-abi.md`.

## Where the shape came from

Two inputs, and neither alone would have been right.

**What `studio-force` calls.** Read off that branch (`git show origin/studio-force:...`), its
`motor/live.ts` declares the port the studio drives, and it is already committed to a shape:

| Studio port (`LiveForce`) | This ABI |
|---|---|
| `step(ticks): number` — the alpha after them | `gm_force_session_tick` (a status) + `gm_force_session_alpha` (`f64`) |
| `pin(id: string, x, y)`, `unpin(id)` | `gm_force_session_pin(session, row, x, y)`, `gm_force_session_unpin` |
| `setParams(knobs)` | `gm_force_session_set_params` |
| `reheat(alpha)` | `gm_force_session_reheat` |
| `positions(): { xs: Float64Array, ys: Float64Array }` | `gm_force_session_column_ptr`/`_len` on `f64` columns |
| — (no teardown in the port) | `gm_force_session_release` |
| — | `gm_force_session_create`, `gm_force_session_params`, `gm_force_session_unpin_all` |
| `ForcePort.seat()` — after each layout | `gm_force_session_seat(session, graph)` |
| `ForcePort.restart()` — "Animate" | `gm_force_session_restart(session)` |

Three things that port decided and this table keeps:

- **`Float64Array`, not `Float32Array`.** Every existing ABI column is `f32`, because every
  existing column is a *snapshot* column and snapshots are hashed. A live session's columns
  are not: they are the simulation state itself, and `liveLoop.ts` already `.slice()`s them
  into transferable buffers every frame. Narrowing them to `f32` here would cost precision the
  next tick reads back — so these two columns are `f64`, and they are the only `f64` columns
  in the module.
- **`pin(id: string, …)`, not a dense index.** The studio holds node ids in dense order
  (`liveDrag.ts` maps a render index through `ids()`), so the id is a studio-side lookup and
  the wire's argument is the row it resolves to. `row` is the node's row in the two position
  columns — the same dense order `gm_column_ptr`'s `NodeX` addresses — which is also exactly
  what the studio's drag already has. No node-id string crosses this ABI, so
  `live-force-session.md`'s "the dense index never leaves the motor" is not weakened: it *is*
  a row in a column the motor owns, and the studio is already holding that column.
- **The port has no teardown**, so a session would leak per graph. `gm_force_session_release`
  exists anyway, and returns `1`/`0` rather than nothing (unlike `gm_release`) precisely
  because a host that ignored it could not tell.

**What `ForceSession` can actually do.** `session.rs`'s own boundary, kept: no
`from_positions` on the wire (a warm start is a *caller* keeping a picture, and the studio's
is the drawing it already rendered), and **no `set_alpha_target` yet**. `settled` is
`alpha < alpha_min && alpha_target == 0`, so with the target pinned at the frozen stage's `0`
the status word is meaningful today and M3's cooling policy is where the setter belongs.
`from_frozen` is likewise not exported: a host with no parameter buffer gets the same thing by
passing `0` (`LiveParams::default()` *is* the frozen set — `docs/decisions/live-force-session.md`).

## The exports

Every wire integer is `u32` (D6): no `usize`, no pointer-width anything. `f64` appears as an
argument (the pin coordinates) and as one return (`alpha`) — fixed-width IEEE-754, identical
on both targets, and no narrower than what the caller's own view already holds.

| Export | Params | Returns | Refuses with |
|---|---|---|---|
| `gm_force_session_create` | `graph: u32, params_ptr: u32, params_len: u32` | session id `>= 1`, or `0` | `InvalidHandle`, `SessionParamsInvalid`, `SessionRefused`, `HandlesExhausted` |
| `gm_force_session_set_params` | `session: u32, params_ptr: u32, params_len: u32` | `1`, or `0` | `InvalidSession`, `SessionParamsInvalid`, `SessionRefused` |
| `gm_force_session_params` | `session: u32` | framed 104-byte `f64` buffer, or `0` | `InvalidSession` |
| `gm_force_session_tick` | `session: u32, ticks: u32` | `0` refused, `1` ran and cooling, `2` ran and settled | `InvalidSession` |
| `gm_force_session_alpha` | `session: u32` | `f64`, or `0.0` | `InvalidSession` |
| `gm_force_session_reheat` | `session: u32, alpha: f64` | `1`, or `0` | `InvalidSession`, `SessionRefused` |
| `gm_force_session_pin` | `session: u32, row: u32, x: f64, y: f64` | `1`, or `0` | `InvalidSession`, `SessionRefused` |
| `gm_force_session_unpin` | `session: u32, row: u32` | `1`, or `0` | `InvalidSession`, `SessionRefused` |
| `gm_force_session_unpin_all` | `session: u32` | `1`, or `0` | `InvalidSession` |
| `gm_force_session_seat` | `session: u32, graph: u32` | `1`, or `0` | `InvalidSession`, `InvalidHandle`, `NoGeometryYet`, `SessionRefused` |
| `gm_force_session_restart` | `session: u32` | `1`, or `0` | `InvalidSession` |
| `gm_force_session_column_ptr` | `session: u32, axis: u32` (`0` = x, `1` = y) | address, or `0` | `InvalidSession`, `IndexOutOfRange` |
| `gm_force_session_column_len` | `session: u32, axis: u32` | element count (one per node), or `0` | `InvalidSession`, `IndexOutOfRange` |
| `gm_force_session_release` | `session: u32` | `1`, or `0` | `InvalidSession` |

Additive: nothing above renumbers, redefines or shadows an existing export, and no existing
error code moves. The three new codes append (15, 16, 17), same as every other code in
`crates/graph-wasm/src/errors.rs`.

### Three decisions in that table, and why

**A status word, not a bool, from `tick`.** `0` is this ABI's refusal sentinel everywhere, so a
`1`-means-ran encoding cannot also say "settled" without stealing the refusal value. `1`
running / `2` settled is what the studio's loop branches on, and it is the only place in the
ABI where a successful call returns something other than `1`, `0` or a pointer.

**Parameters are a 104-byte buffer, read bytewise.** Thirteen `f64` fields is thirteen
arguments — past the house's four-parameter cap, and past what a caller can hold on a stack.
The buffer is `params_len == 0` (the compiled-in defaults) or exactly 104; **any other length
is `SessionParamsInvalid`, never read as the defaults**, because a host that got its length
wrong must be told rather than handed a plausible-looking session at the wrong parameters.

*Bytewise* because `gm_alloc`'s buffers are aligned to 4 (`alloc::ALIGN`, the wire's own word
alignment), and an `f64` load from a 4-aligned address traps on wasm32. So the thirteen fields
are `as_chunks::<8>` + `f64::from_le_bytes`, in `LiveParams`' declaration order — which is also
the byte order every other face of this wire uses (`h1-byte-order.md`).

`gm_force_session_params` exists so a host does not have to *know* the defaults: it reads the
session's own set back as the same 104 bytes. The alternative is thirteen defaults written out
in TypeScript, which is a copy that goes stale silently the day `LiveParams::default` changes.

**The position columns are the session's own `Vec<f64>`.** `column_ptr` is that vector's
address, so reading positions is zero-copy — and the studio's frame loop copies them with
`.slice()` before transferring, exactly as it does for a column view. The one thing the wire
must not do is hand out an address that later changes underneath the reader: it cannot, because
`Sim`'s columns are never resized (the only writer that could, `set_positions`, is reachable
solely from `from_positions`, which this ABI does not export) and each session lives behind a
`Box` in the handle table, so no later insert moves it. An address that does not fit a `u32`
is reported as `0`, never truncated — a truncated heap address is a wild pointer in JS.

## Who owns the memory

| What | Owner | Freed by | Valid until |
|---|---|---|---|
| A parameter buffer (`gm_alloc`'d, 104 bytes) | the caller | the caller, via `gm_free` — the export copies out of it and never frees it, on a refusal too | the call returns (C7) |
| The `gm_force_session_params` framed buffer | the motor's shared out-buffer | nothing; overwritten by the next motor call | the next call on **any** handle (C7) |
| A session | the session table (`handle::Table`, its own id space) | `gm_force_session_release` | — the id is never reissued (C6) |
| A position column's `(ptr, len)` | the session's own `Vec<f64>` | the session's own storage, dropped by `gm_force_session_release` | the session's life; **read it before the next call anyway**, because a wasm memory growth detaches the JS `ArrayBuffer` |

The session table is **not** the handle table. `graph` is a graph handle (`gm_build`'s id);
everything else takes a session id, and a graph id passed as a session id reads
`InvalidSession` (15) rather than `InvalidHandle` (1) — a dead session must never read as a
dead graph, or a host debugging one would go looking for the wrong thing.

A session holds no borrow of its graph: `create` copies what it needs out of the handle's
`Topology`, so the host may `gm_release` the graph the moment `create` returns.

## What is *not* here, and where it goes

- **`from_positions`** — a warm start is a host keeping a picture; the studio's is the drawing
  it already rendered. When a host needs it, it is a fourth wire change, not an oversight.
- **`set_alpha_target`** — M3's cooling policy. With the target at the frozen `0`, `settled`
  means what it says; the setter only earns its place when something can hold the layout hot.
- **A node-id → row lookup** — the studio maps ids in JS today (`liveDrag.ts`); a string-keyed
  export would put a second id grammar on the wire for one caller.
- **`from_frozen`** — `params_len == 0` *is* the frozen parameter set, by construction.

## What proves it

- `crates/graph-wasm/src/session/tests/bits.rs`, natively (C21): N ticks through these functions
  are the same **bits** as `ForceSession::step(N)`, read through the columns the wire returns;
  pins, reheat and parameters likewise. `refusals.rs` holds the id table (issued from 1, never
  reissued), a refused verb leaving the session untouched, and an address the wire cannot carry
  being refused rather than truncated.
- `harness/sdk-smoke.mjs` §force: the SDK's own path — create, tick, drag one node, watch a
  neighbour move — plus the refusals and a released session.
- `graph-cli force-gate` — native ×2 against wasm32 ×2, per seed, over the positions after 50
  ticks, four arms compared line for line by `hashgate`'s own comparator. Its negative control
  is `GM_MUTATE_FORCE_SESSION_GRAVITY`, which perturbs the native arm's live `gravity` and the
  wasm arm cannot see it, so a wired mutation shows up as exactly the cross-target divergence.
  A control that cannot reach a live session is **refused** (exit 2), never run vacuously.

The two halves are deliberately different in kind: the native one says the layer adds no
arithmetic, and the cross-target one says wasm32 agrees with native. Either alone would leave a
real gap — the first cannot see a target-dependent `libm` call, the second cannot tell a
deliberate conversion from a lost bit.