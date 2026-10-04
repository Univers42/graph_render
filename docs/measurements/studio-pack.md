# studio-pack: what the pack weighs, and that two builds agree

Measured 2026-10-04 on `studio-pack` at `7094fcab` (the tree was dirty: the pack build, the host
page build and the two docs are the work of this branch), 20 cores, the same host as
`perf-p3-browser.md`.

## Commands

```sh
scripts/studio.sh build                       # once: app/node_modules and both wasm artifacts
scripts/studio-pack.sh target/pack            # one build, into target/pack/
scripts/studio-pack.sh target/pack-a          # a second build of the same tree, another outdir
cmp <(sha256sum < target/pack-a/graph-studio-0.1.0.tgz) \
    <(sha256sum < target/pack-b/graph-studio-0.1.0.tgz)
```

Row `studio-pack-repro` runs the last two builds and that `cmp`. Row `studio-pack` is the first
build alone; `negctl-studio-pack` is the same with the threads artifact deleted after the build.

## The pack, file by file

`scripts/studio-pack.sh target/pack-a`, printed by the script itself, in byte order:

```
   1047298  graph-studio.js
   1687577  graph_wasm.wasm
   1804986  graph_wasm_threads.wasm
      2441  helper.js
       735  pack.json
    184722  worker.js
```

| File | Bytes | gzip |
|---|---:|---:|
| `graph-studio.js` | 1 047 298 | 257.1 kB (reported by vite) |
| `worker.js` | 184 722 | — |
| `helper.js` | 2 441 | — |
| `graph_wasm.wasm` | 1 687 577 | — |
| `graph_wasm_threads.wasm` | 1 804 986 | — |
| `pack.json` | 735 | — |
| **`graph-studio-0.1.0.tgz`** | **1 392 577** | — |

The two wasm artifacts are 3 492 563 bytes of the 4 735 759 the pack weighs; the three JavaScript
files are 1 234 461, React and its scheduler among them. The pack is not minified, so these are the
sizes a host reads off a directory listing, not the sizes of a gzipped transfer.

## Build time

| Step | Wall clock |
|---|---:|
| `scripts/studio-pack.sh target/pack-a` | 2.1 s |
| `scripts/studio-pack.sh target/pack-b` | 2.2 s |

Both are warm: the cargo release build of `graph-wasm` (and of the threads variant through
`scripts/orch/wasm-threads.sh`) is a no-op cargo run when `target/` already holds it, and vite builds
in about 130 ms. A cold tree adds the two cargo builds — the threads one rebuilds the standard
library with atomics and is the expensive half. No number for a cold tree was measured here.

## Two builds of one tree

```
6e674a0b6780fb70f9cbc6534e2c822c97cf509de7ed5f743b095904f9fa96b3  target/pack-a/graph-studio-0.1.0.tgz
6e674a0b6780fb70f9cbc6534e2c822c97cf509de7ed5f743b095904f9fa96b3  target/pack-b/graph-studio-0.1.0.tgz
```

Equal. What makes them equal is in `scripts/studio-pack.sh`: `tar --sort=name` for the order,
`--mtime=@0 --owner=0 --group=0 --numeric-owner` for the headers, `--mode=u+rw,go=rX,go-w` for what
the file system hands over (without it a pack directory that already existed with a group-writable
mode tarred differently from a fresh one — measured, the same tree gave two different tarball
digests until the mode was normalised), and `gzip -n` for the gzip header.

Caveat: the pack carries `pack.json`'s `source_rev`, so two builds of *different* trees differ, and
two builds that straddle a commit differ too. The claim is one tree, one digest — which is what the
row asserts, both builds inside one command.

## What the pack costs to serve

The `studio-pack-embed` run is the same page over the pack as `studio-embed` over `app/dist`, in
the same three runs (`plain`, `isolated`, `csp`), and it drew the fixture in the same load: the pack
adds nothing on the page's path but the entry's own bytes, since the wasm is compiled in the worker
either way. What the pack changes is what a host serves — a fixed 4 735 759-byte directory it can
copy and nothing to build. The numbers this document does not have: a CDN's cache hit ratio, and a
first-visit parse on a slow link. Nothing here claims them.