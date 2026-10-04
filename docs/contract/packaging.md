# Packaging: shipping `<graph-studio>` to a host

Status: **as built, 2026-10-04**. One pack, built by `scripts/studio-pack.sh`, verified by its own
rows and served by the embed gate (`scripts/studio-embed.sh` with `STUDIO_EMBED_PACK`). The API a
host calls is `host-api.md`; this is how the files that carry it get there.

Today `packages/graph-studio` ships source only ("a host bundles src/element.ts"), so a host needs
this repository's toolchain to use the studio. The pack removes that requirement: the studio leaves
as one ESM file, its worker chunks, and both wasm artifacts.

## What the pack is

`<outdir>/graph-studio-<version>/`, and `<outdir>/graph-studio-<version>.tgz` beside it. Sizes as
built (`docs/measurements/studio-pack.md`):

| File | Bytes | What it is |
|---|---:|---|
| `graph-studio.js` | 1 047 298 | the entry: `defineGraphStudio`, `HOST_API`, the custom element, React bundled in |
| `worker.js` | 184 722 | the motor worker, spawned by `new URL("worker.js", import.meta.url)` |
| `helper.js` | 2 441 | the threads helper pool's worker, spawned the same way from `worker.js` |
| `graph_wasm.wasm` | 1 687 577 | the serial motor |
| `graph_wasm_threads.wasm` | 1 804 986 | the threads motor (model (a), `docs/decisions/browser-threads.md`) |
| `pack.json` | 735 | the manifest below |

Six files, flat, no directory. It is not minified: a host audits the bytes it ships, and every
check in `scripts/studio-pack.sh` is a line of the bundle. The entry keeps its exports for the host
(`preserveEntrySignatures: "exports-only"`), and the motor worker stays a real file rather than a
`blob:` URL, which the CSP below refuses.

### pack.json

```json
{ "name": "graph-studio", "version": "0.1.0", "abi_version": 2, "source_rev": "7094fcab…-dirty",
  "files": { "graph-studio.js": {"bytes": 1047298, "sha256": "b657a44a…"}, … } }
```

- `name`, `version` — `packages/graph-studio/package.json`.
- `abi_version` — `ABI_VERSION` (`crates/graph-wasm/src/lib.rs:139`), the revision of the wasm ABI
  the SDK speaks.
- `source_rev` — `git rev-parse HEAD`, with `-dirty` appended when the tree was unclean. A pack is a
  claim about a tree; this is the tree.
- `files` — every file of the pack but `pack.json` itself, keys in byte order, each with its size
  and sha256. A host that serves the pack over HTTP can check its download with this and nothing
  else; `scripts/studio-pack.sh` checks the pack against it before it calls the pack shippable.

The tarball is deterministic: `tar --sort=name --mtime=@0 --owner=0 --group=0 --numeric-owner
--format=gnu --mode=u+rw,go=rX,go-w` piped through `gzip -n`, so two builds of one tree have the
same sha256 (row `studio-pack-repro`).

## Serving it

**Same origin, flat.** Serve all six files from one directory on the host's own origin. Two of the
lookups are relative and both are load-bearing:

- The threads artifact is a **sibling of the serial one**: the motor worker builds it as
  `new URL("graph_wasm_threads.wasm", wasmUrl)` from the URL of the `wasm` attribute
  (`packages/graph-studio/src/motor/worker.ts:59`). Put the two `.wasm` files in the same directory
  or the threads path 404s and the worker falls back to the serial module without a word.
- The two workers are siblings of `graph-studio.js` itself (`new URL("worker.js", import.meta.url)`),
  so `worker.js` and `helper.js` go in the same directory as the entry.

**`application/wasm`.** Both artifacts must be served with that content type, or the browser refuses
the module as a MIME mismatch (this is what `deploy/serve.py` maps `.wasm` to, and what the `Log`
channel of the smoke gate reads). Serve the pack's files over `Content-Encoding: identity` if you
compress them.

**CSP.** Verdict 13 of `host-api.md` asks for exactly this, and the `csp` run of the embed gate
serves it over the pack:

```
default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; worker-src 'self'; img-src 'self' data:
```

`script-src 'wasm-unsafe-eval'` is what lets the module compile; `worker-src 'self'` is why the
motor worker is a file of the pack and never a `blob:` or `data:` URL. `img-src 'self' data:` is the
studio's own icons and nothing a host has to think about.

**COOP/COEP, for threads.** `Cross-Origin-Opener-Policy: same-origin` and
`Cross-Origin-Embedder-Policy: require-corp` on the page **and** on every file it loads make the
page cross-origin isolated, which is the only way a browser hands out a `SharedArrayBuffer` the
threads motor needs (`docs/decisions/browser-threads.md`). Without them the studio still runs, on one
thread: `threadsFor` returns 1 for a page that is not isolated
(`packages/graph-studio/src/motor/threads.ts`) and the worker loads `graph_wasm.wasm`.

That fallback is not a failure mode with no cost, at the sizes a host cares about. From
`docs/measurements/perf-p3-browser.md:44-47`, the live settle's tick:

| nodes | 1 thread | 8 threads |
|---|---:|---:|
| 400 000 | 238–252 ms | 69–76 ms |
| 1 000 000 | 657–758 ms | 158–170 ms |

So a host that ships the headers buys 3.4× at 400k and 4.3× at 1M; a host that does not pays the
left column and still draws the same graph.

## Versioning

Two numbers, bumped for different reasons.

- **`version`** (`packages/graph-studio/package.json`) is the pack. Bump it for anything a host can
  observe: the element's API (`host-api.md`), what the element draws, which attributes it reads. It
  names the pack directory and the tarball, so a host pins it.
- **`abi_version`** (`ABI_VERSION`, `crates/graph-wasm/src/lib.rs`) is the wire contract between the
  SDK in the bundle and the wasm beside it. Bump it when the exported functions, the buffer layout
  or the snapshot format change — that is, when an old bundle must not be paired with a new
  artifact. `pack.json` carries both, and `scripts/studio-pack.sh` reads them from the tree rather
  than taking them as arguments, so they cannot drift from what was built.

A host that pins `version` and checks `pack.json` against its download needs neither of this
repository's toolchains, its sources or its node_modules.

## What it does not do

- **No npm publish, no CDN.** The pack is a tarball or a directory a host copies. There is no
  registry entry and no upload anywhere in this repository.
- **No shared React.** React is bundled into `graph-studio.js` on purpose: an embed host has no React
  of its own, and a host that *does* still gets the pack's copy inside the element's shadow root.
  Two Reacts on one page never meet, because the pack's is not on the page's side of the shadow
  boundary — at the price of the bytes.
- **No SSR.** The element is a custom element with a shadow root, a canvas and a Worker; it needs a
  DOM. A host renders it client-side, after its own script runs.
- **No fixtures.** The pack ships no fixture directory. The `fixtures` attribute is the host's, and
  so is the graph it serves.
- **No minification, no source maps.** The bundle is what a host can read; there is no `.map` to
  publish and no original path to leak.

## The rows that hold it

| Row | Command | What it proves |
|---|---|---|
| `studio-pack` | `scripts/studio-pack.sh target/pack` | the pack builds and passes its own verify |
| `negctl-studio-pack` | `STUDIO_PACK_BREAK=1 …` | deleting the threads artifact turns the verify red, naming the file |
| `studio-pack-repro` | two builds, two outdirs | the same tree gives byte-identical tarballs |
| `studio-pack-embed` | `STUDIO_EMBED_PACK=… scripts/studio-embed.sh` | every embed row holds over the pack, and each run fetched the wasm its isolation allows |
| `negctl-studio-pack-embed` | `STUDIO_EMBED_PACK=… STUDIO_EMBED_BREAK=1 …` | the same rows can go red over the pack, one fault each |