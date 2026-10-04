verdict: PROCEED-WITH-CONDITIONS

# Review — one bundle instead of two (`/embed/<version>/` vs the pack)

Judge: independent, 2026-10-04. Branch `review-bundle-unify` at `db1f56cd`, base
`origin/develop`. I built both bundles in this task and read the emitted bytes; I did **not** run
`gate.sh`, `service-image.sh` or `studio-embed.sh`, so every claim about a *gate row's* outcome is
UNKNOWN here and is marked as such. No file under version control changed but this one.

---

## 0. The finding that reorders the question

**The stated reason the pack avoids `build.lib` is false on this tree.** `app/vite.pack.config.ts:13-17`
says a library build "is free to fold that worker into the entry as a blob URL, and `worker-src 'self'`
… refuses a `blob:` worker". Measured, the service bundle's worker is a real file on the host's own
origin. The premise of the unification is not there.

The second, larger finding is methodological, and it is why this is not simply PROCEED:

> **The measurement command this job prescribed is a false negative.** `grep -o 'new Worker([^)]*)'`
> returns **nothing** on the service bundle — not because there is no worker, but because `[^)]*`
> cannot cross the newline that minification leaves between `new Worker(new URL(` and its argument.
> An engineer who ran only the prescribed greps would report "no `new Worker` at all" and conclude the
> studio has no motor worker. It has one.

## 1. The service bundle's worker, measured

`scripts/studio.sh wasm` → exit 0 (`target/review-wasm.log`), then `scripts/studio.sh embed
target/embed-probe` → exit 0 (`target/review-embed.log`), on vite 8.3.1, 244 modules transformed.

```
$ ls -la target/embed-probe
total 4052
drwxr-xr-x 2 dlesieur dlesieur    4096 Oct  4 10:50 .
drwxr-xr-x 7 dlesieur dlesieur    4096 Oct  4 10:50 ..
drwxr-xr-x 2 dlesieur dlesieur    4096 Oct  4 10:50 assets
-rw-r--r-- 1 dlesieur dlesieur   46115 Oct  4 10:50 graph-sdk.js
-rw-r--r-- 1 dlesieur dlesieur  586540 Oct  4 10:50 graph-studio.js
-rwxr-xr-x 1 dlesieur dlesieur 1687577 Oct  4 10:50 graph_wasm.wasm
-rwxr-xr-x 1 dlesieur dlesieur 1804986 Oct  4 10:50 graph_wasm_threads.wasm

$ ls -la target/embed-probe/assets
total 72
drwxr-xr-x 2 dlesieur dlesieur 4096 Oct  4 10:50 .
drwxr-xr-x 7 dlesieur dlesieur 4096 Oct  4 10:50 ..
-rw-r--r-- 1 dlesieur dlesieur  1053 Oct  4 10:50 helper-DVGXk0FG.js
-rw-r--r-- 1 dlesieur dlesieur 60518 Oct  4 10:50 worker-D2ArSvee.js

$ grep -o 'new Worker([^)]*)' -r target/embed-probe | head
                       <-- EMPTY. Not "no worker": see section 0.

$ grep -c 'blob:\|data:application/javascript\|data:text/javascript' -r target/embed-probe
target/embed-probe/graph_wasm_threads.wasm:0
target/embed-probe/graph-sdk.js:0
target/embed-probe/assets/worker-D2ArSvee.js:0
target/embed-probe/assets/helper-DVGXk0FG.js:0
target/embed-probe/graph_wasm.wasm:0
target/embed-probe/graph-studio.js:0
                                                 (grep exit 1: no file matched)
```

The construction the prescribed grep could not see, at byte offset 580857 of `graph-studio.js`:

```js
function qC() {
	return rv(new Worker(new URL(
		/* @vite-ignore */
		new URL("assets/worker-D2ArSvee.js", import.meta.url).href,
		"" + import.meta.url
	), { type: "module" }));
}
```

and the threads helper's own spawn, at offset 59650 of `assets/worker-D2ArSvee.js`:

```js
function Mi(e){new Worker(new URL(
/* @vite-ignore */
new URL(`helper-DVGXk0FG.js`,import.meta.url).href,``+import.meta.url),{type:`module`}).postMessage(e)}
```

Both are `new URL(<relative file>, import.meta.url)` — a same-origin path resolved against the module's
own URL, which is what `host-api.md:162-164` and `packaging.md:59-60` require. The `createObjectURL`
hits in the bundle (`graph-studio.js:16858,16870,17508,20239`) are the element's `save()` download path,
not workers.

### CSP conclusion, from those lines only

**A host page under `worker-src 'self'` can start that worker.** The URL the Worker is given is
`/embed/<version>/assets/worker-<hash>.js`, same-origin with the page, and `worker-src 'self'` admits
same-origin. Nothing about the service bundle needs `blob:` or `data:`. **Task 5 does not fire: there is
no `blob:`/`data:` worker in the service bundle.**

For completeness, the pack builds the identical form, so the two agree here and only here differ in the
filename's location (`scripts/studio-pack.sh` → exit 0, `target/review-pack.log`):

```
$ ls -la target/pack/graph-studio-0.1.0/     # 1066142 graph-studio.js, 185976 worker.js, 2441 helper.js
new Worker(new URL(
		/* @vite-ignore */
		new URL("worker.js", import.meta.url).href,
```

### The row that corroborates it — cited, NOT run

`scripts/orch/rows/service-image.rows:14` (`svc-image` → `timeout 2400 scripts/service-image.sh`) drives
`deploy/nav/service.py:84-85`, which browses `/isolated/` and `/plain/`. Those pages are built by
`deploy/nav/serviceproxy.py:52-54` and carry exactly the CSP of `service.md:106-107` —
`CSP = "script-src 'self' 'wasm-unsafe-eval'; worker-src 'self'"` at `serviceproxy.py:23`. On those
pages `servicerows.py:95` asserts `row_drew_nodes`, and `servicerows.py:104-107` requires
`bool(workers)` — at least one attached motor worker, discovered by `page.watch_workers()`
(`service.py:60`). `direct-refused` (`servicerows.py:141-148`) proves the same worker is
same-origin-bound.

**Did it run the threads worker too? Partly asserted, partly inferred.**
- *Asserted:* `isolated-wasm-build` (`servicerows.py:111-121`) requires that the worker fetched
  `graph_wasm_threads.wasm` and no other `.wasm`, read from the host proxy's own request log. So the
  **threads code path** ran on the isolated page.
- *Inferred, not asserted:* the threads helper pool. `worker.ts:48` (`spawnHelper`) is only reached when
  `threadsFor(...) > 1` (`worker.ts:57`), so `assets/helper-DVGXk0FG.js` was fetched — but **no row reads
  a `.js` fetch**, so nothing asserts the helper worker started. Treat that half as UNKNOWN.

I did not run this row. Its pass/fail state is the orchestrator's, not mine.

## 2. Every consumer of each surface, with `file:line`

### `/embed/<version>/` — the service surface, the one a host imports

| Thing | Where it is fixed | Who reads it |
|---|---|---|
| host import path `…/graph-studio.js` + `wasm=` attribute | `docs/deploy/service.md:98-99` | the gate's own host page, `serviceproxy.py:34-35` |
| same-origin requirement | `docs/deploy/service.md:81-86`, `host-api.md:162-164` (condition 13) | `direct-refused`, `servicerows.py:141` |
| `embed/VERSION` = 16 hex + `\n` | written `scripts/service.sh:85`; read `scripts/service.sh:56`, `:95` | `server/graph-server/src/embed.rs:53`, format at `embed.rs:100`; `svc-embed-unversioned` (`servicerows.py:75-80`) |
| content-hash `<version>` | `scripts/service.sh:44-46` (`content_hash`, first 16 hex over `sha256sum` of every file, path-sorted); `<tag>` = same over the stage, `service.sh:92` | `service.md:36-40`; the `immutable` cache is what makes this safe |
| file name + content type | `docs/deploy/service.md:26` | `servicerows.py:19` (`graph-studio.js` → `text/javascript` + COEP/CORP/nosniff/immutable), `servicerows.py:18` (both wasm → `application/wasm`) |
| chunks under `assets/` are served | `server/graph-server/src/embed.rs:86` (table lookup on the `/`-joined path), proven by `server/graph-server/tests/embed.rs:55` | `assets/worker-*.js`, `assets/helper-*.js` |
| all of it | `docs/deploy/service.md:26-28` (the staged list) | `service-image.rows:14` |

### The pack surface

| Thing | Where | Who reads it |
|---|---|---|
| six flat files | `docs/contract/packaging.md:16-23` | `scripts/studio-pack.sh:144-150` (report) |
| `pack.json` `{name,version,abi_version,source_rev,files}` | written `studio-pack.sh:80-94`, shape at `:18-21`, documented `packaging.md:30-44` | verified `studio-pack.sh:107-121`; a host is told to check its download against it (`packaging.md:42-44`). **No in-tree consumer beyond the verify.** |
| rows | `host-api.rows:9,12,15,16,19` | `studio-pack`, `negctl-studio-pack`, `studio-pack-repro`, `studio-pack-embed`, `negctl-studio-pack-embed` |
| the pack does **not** auto-define | `app/src/embed.ts:97` calls `defineGraphStudio()` | `packaging.md:26` "the entry keeps its exports for the host" |

### `graph-sdk.js`: built, staged, served — and consumed by nobody

- Built: `app/vite.embed.config.ts:30`. Staged by `scripts/service.sh:78-86`. Served by graph-server like
  any other file under the version directory.
- `grep -rn "graph-sdk\.js" scripts server deploy app` → **no matches.**
- The only mention in the whole tree is `docs/deploy/service.md:26`, which lists it.
- No row fetches it. No host document tells a host to import it.
- It is not the SDK the studio uses: `grep -c gm_run` gives **0** in `graph-studio.js` and **1** in
  `assets/worker-D2ArSvee.js`, with `forceSession` 0 and 1 respectively. The studio's SDK copy is in the
  worker chunk; `graph-sdk.js` is a second, unreferenced 46,115-byte copy of the same source.

**Task 2 decides this one: `graph-sdk.js` is dead weight on a public surface.** Dropping it changes the
file set, therefore `<version>` (`service.sh:44-46`), therefore every host's tag — the release rule
`service.md:129-130` already states.

### The one gate hole this review found

`grep -rn "new Worker" scripts/ deploy/ --include=*.sh --include=*.rows --include=*.py` outside
`studio-pack.sh` → **no matches.** The worker-form check exists for the pack (`studio-pack.sh:138`) and
for **nothing else**. `target/service/bundle` is named only in `scripts/service.sh:79,81`. So the property
`vite.pack.config.ts:13-17` was invented to protect is asserted over one of the two artifacts and
silently unasserted over the other — the one a host actually imports today.

## 3. The auto-define difference, and who depends on which behaviour

Measured on the two built entries:

- service: `//#region src/embed-bundle.ts` → `sw();` (auto-define on import), and
  `export { w as HOST_API, ue as OPEN_VIAS, O as backendOf, sw as defineGraphStudio };`
- pack: `export { HOST_API, OPEN_VIAS, backendOf, defineGraphStudio };` and **no call** —
  `grep -c 'defineGraphStudio('` = 1 in each file, and in the pack that one hit is the definition.

| Documented host snippet | Relies on |
|---|---|
| `docs/deploy/service.md:98-99` — bare `<script type="module" src="/embed/<v>/graph-studio.js">` | **auto-define**. No import, no call. |
| `deploy/nav/serviceproxy.py:34-35` — the gate's own page, identical shape | **auto-define**. Every `svc-image` browser row depends on it. |
| `app/embed.html:18` → `app/src/embed.ts:97` | **exports**: an explicit `defineGraphStudio()` after setting `resolve` (`embed.ts:73`) and the attributes (`:68-69`). |
| `docs/contract/packaging.md:26` | **exports**: "The entry keeps its exports for the host". |

**These are not the same requirement, and auto-define is not a safe superset.**
`packages/graph-studio/src/element.ts:146-147` is `if (customElements.get(tag) !== undefined) return;` —
**first definition wins.** So if the pack's entry auto-defined, a pack host that imports it and then calls
`defineGraphStudio({...myOptions})` would get the auto-registered default-options element and have its
options **silently discarded**, with no error. `host-api.md:158-159` documents the same first-wins rule
from the other side. That is the sharp edge in option (a) and it is invisible to every row in the tree:
`app/src/embed.ts:97` passes no options, so `studio-pack-embed` stays green either way.

## 4. The options, with the diff

Sizes are from the two builds measured above: service 4,186,789 B over 5 files + `assets/`;
pack 4,747,851 B over 6 files. The pack is **+561,062 B (+13.4%)** because it is not minified
(`packaging.md:25-27`, `vite.pack.config.ts:59`).

### (a) The service stages the pack; `vite.embed.config.ts` + `embed-bundle.ts` deleted

- Delete `app/vite.embed.config.ts` (35 lines) and `app/src/embed-bundle.ts` (11).
- `scripts/service.sh:78-86` `stage_embed`: call `scripts/studio-pack.sh`, copy the six files into
  `embed/<v>/` instead of `scripts/studio.sh embed` (~10 lines changed).
- Delete `scripts/studio.sh:174-184` `embed()` and its `case` arm at `:212-218` (~15 lines).
- `docs/deploy/service.md:26-27` rewritten.
- **≈70 lines deleted, ≈15 changed.**
- **Breaks, in order of severity:**
  1. **Every existing host page stops mounting the studio, silently.** `service.md:98-99` and
     `serviceproxy.py:34-35` both depend on auto-define, which the pack does not do. The element is
     simply never defined; no exception, no `graph-error` (`host-api.md:158` — `whenDefined` never
     resolves). Restoring it means re-adding auto-define to the pack, which brings back the
     options-discarded hazard of section 3. **This is the break that decides the option.**
  2. `<version>` changes → every host retags in the same release (`service.md:129-130`). Already the
     documented rule; still a coordinated cut.
  3. `/embed/` grows 13.4% on the wire, and `graph-studio.js` stops being minified.
  4. The pack's `worker.js`/`helper.js` are flat siblings of the entry, which satisfies
     `packaging.md:59-60`; the version directory can hold them flat too. **No break.**
  5. `graph-sdk.js`: drop it (recommended — zero consumers, section 2), or add it as a second entry to
     `vite.pack.config.ts` (+~6 lines), which adds a seventh file to `pack.json` and to every host's
     directory listing for no reader.
- **Auto-define:** kept only by making the **pack** auto-define — which is the hazard in section 3. There
  is no version of (a) that keeps both documented host behaviours without either changing pack behaviour
  or keeping `embed-bundle.ts`.

### (b) `vite.embed.config.ts` imports the pack's build function and only adds the SDK entry

- Export `packBuild` from `app/vite.pack.config.ts`; `vite.embed.config.ts` composes it with a second
  entry. **≈15 lines changed, nothing deleted.**
- **Breaks: nothing on the host side.** `/embed/<version>/` keeps its name, its auto-define, its
  minification and its content hash shape. It buys exactly one thing: the motor worker stops being a
  lib-mode worker under `assets/` and becomes the pack's flat, unhashed `worker.js`.
- It does **not** unify: two vite invocations, two entry graphs, and `<version>` still covers a
  different file set than `pack.json`. It retires the one risk that section 0 shows was never real.

### (c) Keep two bundles; add a row that fails when they disagree on the worker form — **RECOMMENDED**

- One new row, in `scripts/orch/rows/host-api.rows` (or `service-image.rows`), that greps the built
  service bundle the way `studio-pack.sh:138` greps the pack. **≈3 lines of rows file.**
- **Breaks: nothing.** No public surface moves, no host retags, no version changes.
- It closes the real hole in section 2 ("asserted over one of the two artifacts, unasserted over the
  one a host imports") and it is the only option that does.

### (d) Make the configs agree by construction

`vite.pack.config.ts:28` already imports `./vite.config.ts` and takes its `resolve` and `worker` blocks,
so the `resolve` disagreement the two configs appear to have is already resolved. What is left is
`input`/`preserveEntrySignatures` and `minify` — which is (b) restated. No third option.

## 5. Verdict

**PROCEED-WITH-CONDITIONS.** Recommended option: **(c)**, plus the two cheap corrections below. Option
(a) is **BLOCK** until condition 1 is answered, and (b) is only worth its 15 lines if a real blob worker
appears.

### Axis scores (5 = worst), scored on option (a), the unification on the table

| Axis | Score | Why |
|---|---|---|
| Blast radius | 4 | `/embed/<version>/` is the public surface a host imports (`service.md:98-99`), served by graph-server and consumed by every `svc-image` browser row. (a) rewrites its file set under every host at once. |
| Reversibility | 4 | a code revert restores the two configs, but `<version>` (`service.sh:44-46`) is a content hash over the file set: hosts that already retagged to the new URL must retag **again** on revert. Two coordinated cuts, not one. |
| **Cost on failure** | **5 — worst** | auto-define loss (`element.ts:146-147`, section 3) means the element is never defined. No exception, no `graph-error`, no banner — `row_no_exception` and friends stay green and the studio simply never appears. A silent, total, public-surface outage, discovered by a user, not by a row. |
| Confidence | 2 | the load-bearing fact is **measured, not assumed**: I built both bundles and read the emitted worker URL, so the `blob:` premise is dead. Confidence is 2 not 1 only because the pack's first-wins options behaviour (section 3) is read from source and not executed, and no gate row was run here. |

### Conditions

Each is checkable by a row, and each names the break that turns it red.

1. **Answer the auto-define question in writing before any of (a) lands.** Either (i) the pack's entry
   keeps *not* auto-defining and `docs/deploy/service.md:98-99` plus `deploy/nav/serviceproxy.py:34-35`
   are changed to import-and-call, which is a breaking change to every host's HTML and must be released
   as one; or (ii) the pack's entry auto-defines, and then `docs/contract/host-api.md:158` must gain the
   warning that a host's own `defineGraphStudio(options)` is then **discarded**, and
   `packages/graph-studio/tests/` must gain a case that fails when a second definition with options is
   ignored. *No row today can tell (i) from (ii)* — `app/src/embed.ts:97` passes no options.
   *Break that turns it red:* under (ii), edit `app/src/embed.ts:97` to
   `defineGraphStudio({ threads: { helpers: 0 } })` and assert the element honours it; the new case fails
   today, because `element.ts:147` returns first.
2. **Add the worker-form row over the service bundle** — condition for (b) and (c) both, and the row that
   makes option (c) worth anything. Files: `scripts/orch/rows/host-api.rows` (new row), reusing the
   predicate at `scripts/studio-pack.sh:138`. It must run against `target/service/bundle` after
   `scripts/studio.sh embed`, and it must use a pattern that survives minification —
   `grep -nF 'new Worker' … | grep -e blob: -e data: -e createObjectURL`, **not**
   `grep -o 'new Worker([^)]*)'` (section 0: that pattern is a false negative on this output).
   *Break:* a row whose own fixture is a one-line `new Worker(new URL(\n"x.js",import.meta.url))` and which
   must PASS; then the same fixture with `blob:` must turn it red. If the row cannot be made to fail on
   the `blob:` fixture, it is not a gate.
3. **Fix the false rationale in the source before it is cited again.**
   `app/vite.pack.config.ts:13-17` claims a `build.lib` worker "is free to fold that worker into the entry
   as a blob URL". On vite 8.3.1 with `worker: { format: "es" }` (`vite.embed.config.ts:25`) it does not,
   and `assets/worker-<hash>.js` is a real file. Rewrite the comment to the measured reason the pack
   differs — flat, unhashed, auditable names (`packaging.md:19-20,59-60`) — and keep the blob claim as
   the thing condition 2 now *watches* rather than the thing it *assumes*.
   *Break:* none needed (a comment); but condition 2's row is what makes the rewrite safe to make.
4. **Decide `graph-sdk.js` and say so in `docs/deploy/service.md:26`.** Task 2's finding stands: zero
   consumers outside that one line. Recommended: drop it from the `build.lib` entries. Either way the
   document must stop listing a file nobody imports, and `<version>` changes either way
   (`service.sh:44-46`), so it belongs in a host release.
   *Break:* a row asserting the staged file list under `target/service/stage/embed/<v>/` equals the list
   `docs/deploy/service.md:26-27` prints. It turns red the moment the two drift — which is the state
   today for `graph-sdk.js` only if you count the doc as authoritative for *use*; as shipped they agree
   on *presence*, so this row's first job is to make "listed ⇒ consumed" checkable at all.
5. **Run the gate before the verdict is spent.** `svc-image` (`service-image.rows:14`) and
   `studio-pack-embed` (`host-api.rows:16`) are **NOT RUN** in this review; both are image/chromium gates
   the orchestrator owns. Until `svc-image` passes, the corroboration in section 1 is a citation, not a
   result, and the auto-define half of condition 1 stays UNKNOWN by the `UNKNOWN = FAIL` rule.

## 6. What I do not know

- **No gate row was run here.** Every statement about a row's outcome is UNKNOWN, including
  `svc-image`, `studio-pack`, `studio-pack-embed`.
- **Whether the threads *helper* worker actually starts under the service bundle** is inferred from
  `worker.ts:48,57`, not measured: no row reads a `.js` fetch (`servicerows.py:118` filters `.wasm`).
- **The pack's first-wins options behaviour** (section 3) is read from `element.ts:146-147`, not executed.
- `docs/contract/packaging.md:18` records `graph-studio.js` at 1,047,298 B; the build I measured is
  1,066,142 B (`target/review-pack.log`). The table is stale by 18,844 B on this tree — cosmetic, but it
  means the doc's sizes are not a measurement anyone can reproduce today.

## 7. Record

My own rule (`risk.md:33-38`: BLOCK or PROCEED-WITH-CONDITIONS on a public-surface change ⇒ write an
ADR) fires here, and `templates/adr.md` does not exist in this repository and `docs/adr/` does not
exist either. This job's allowed paths are `docs/reviews/review-bundle-unify.md`, its prompt and
`target/**`, so I wrote no ADR. **Recommendation: the builder records this as
`docs/adr/0001-two-bundles-not-one.md`, with the line that decides it — "the service bundle's worker is
a real same-origin file, measured at `assets/worker-<hash>.js` with zero `blob:`/`data:` hits, so the
reason the pack avoids `build.lib` does not exist on vite 8.3.1" — because that is the fact that would
have to change for option (a) to become the answer.** If a future vite folds lib-mode workers into blobs,
that line is false and the decision reopens; nothing else here would.
