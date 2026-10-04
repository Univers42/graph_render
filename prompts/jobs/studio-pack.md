# Job studio-pack: the studio as a versioned, self-contained pack (service DoD step 4)

Worktree `~/goinfre/wt/studio-pack`, branch `studio-pack`, from develop 802f0f30. The DoD
(`~/goinfre/orch/handoff/service-dod.md` step 4) asks for an ESM bundle plus both wasm artifacts,
versioned, with the COOP/COEP requirement and the serial fallback's number documented. Today the
studio ships source only (`packages/graph-studio/package.json` "Ships no build: a host bundles
src/element.ts"), so a host needs this repo's toolchain to use it. Dispatch at most 2 subagents.

## Facts (checked 2026-10-04 on 802f0f30)

- `scripts/studio.sh` `stage_assets` (`:68-86`) builds `graph_wasm.wasm` (serial) and
  `graph_wasm_threads.wasm` (`scripts/orch/wasm-threads.sh`) and copies both into `app/public`.
- The worker finds the threads module as a sibling of the `wasm` attribute:
  `packages/graph-studio/src/motor/worker.ts:56-63` (`new URL("graph_wasm_threads.wasm", wasmUrl)`),
  and falls back to the serial module when the page is not cross-origin isolated.
- `<graph-studio wasm=... fixtures=...>` is the element (`packages/graph-studio/src/element.ts:6`);
  `defineGraphStudio` and `HOST_API` are its exports (`app/src/embed.ts:15`).
- The embed gate: `scripts/studio-embed.sh` serves `app/dist` and runs `deploy/nav/embed.py
  --dist ... --out ...` in three runs, `plain`, `isolated` and `csp`. The CSP a host must allow is
  `HOST_CSP` (`deploy/nav/embed.py:42`): `worker-src 'self'` refuses a `blob:` or `data:` worker.
- The serial fallback's numbers: `docs/measurements/perf-p3-browser.md:44-47`. At 1M nodes the tick
  is 657–758 ms on 1 thread and 158–170 ms on 8; at 400k it is 238–252 ms and 69–76 ms.
- The ABI revision is `ABI_VERSION` (`crates/graph-wasm/src/lib.rs:139`, now 2), and the pack
  version is `packages/graph-studio/package.json` `version` (0.1.0).
- The browser gates need `scripts/studio.sh build` first and never take the host gate lock.

## Tasks, in order

1. **Bundle config.** `app/vite.pack.config.ts`: entry `packages/graph-studio/src/element.ts`,
   ES output, `graph-studio.js` as the entry file name, React bundled in (the embed host has no
   React of its own). The motor worker must be its own `.js` file in the pack, never inlined,
   because of `worker-src 'self'`. Rollup `input` with `preserveEntrySignatures: "exports-only"`
   does that. `build.lib` may inline workers; use it only if you show that it emits the worker as
   a file. Reuse `app/vite.config.ts`'s `resolve` block: import it, do not copy it.
2. **`scripts/studio-pack.sh [outdir]`.** Its header is its manual, like the other `studio-*.sh`.
   Run node through the same Docker path `scripts/studio.sh` uses (reuse its functions or call it).
   The default outdir is `target/pack`. Steps:
   - Run `stage_assets` (both wasm), then the vite pack build, into
     `<outdir>/graph-studio-<version>/`.
   - Copy in both wasm files, then write `pack.json`:
     `{name, version, abi_version, source_rev, files: {<name>: {bytes, sha256}}}`, with keys sorted.
     `source_rev` is `git rev-parse HEAD`, plus `-dirty` on an unclean tree.
   - Write a deterministic `graph-studio-<version>.tgz`: `tar --sort=name --mtime=@0 --owner=0
     --group=0 --numeric-owner`, piped through `gzip -n`.
   - Then verify the pack, with exit 0 = good, 1 = bad pack, 2 = could not build:
     - every file in `pack.json` exists with its sha256;
     - both wasm files are present;
     - the threads module is referenced by name from a pack `.js` file;
     - no pack file references `/src/`, `node_modules` or `../`;
     - no `blob:` or `data:` worker is constructed.
   - Print one line per file with its size, then the tarball sha256.
   - `STUDIO_PACK_BREAK=1` builds, deletes `graph_wasm_threads.wasm` from the pack and then
     verifies: it must exit 1 and name the missing file.
3. **The pack under the embed gate.** Add `STUDIO_EMBED_PACK=<pack dir>` to `scripts/studio-embed.sh`.
   - It assembles a host directory: the pack's files, the fixtures, and an embed page whose only
     script import is `./graph-studio.js` from the pack.
   - Prefer building `app/embed.html` once more with an alias that maps the element import to the
     pack. A small new host page is fine if that is simpler.
   - Then it runs the same three `embed.py` runs over that directory, same rows.
   - In addition, record which wasm the worker fetched, from the CDP network log. `plain` must
     fetch `graph_wasm.wasm` only; `isolated` must fetch `graph_wasm_threads.wasm`. Each is a row
     in `report.json`.
   - Keep `deploy/nav/embed.py` under 300 lines; put new code in a new `deploy/nav/embedpack.py`.
4. **Docs.**
   - Create `docs/contract/packaging.md`. It covers:
     - what the pack contains, and the `pack.json` fields;
     - how a host serves it: `application/wasm`, both wasm files beside each other, the CSP of
       verdict 13, and COOP `same-origin` + COEP `require-corp` for threads;
     - the serial fallback with both numbers and the citation above;
     - versioning: the pack version, `abi_version`, and when each is bumped;
     - "What it does not do": no npm publish, no CDN; a host with its own React still gets the
       pack's copy inside the shadow root; no SSR.
   - Add one link line to it in `docs/contract/host-api.md`.
   - Create `docs/measurements/studio-pack.md`: the size of each file, the build time, and the
     tarball sha256 of two builds of the same tree (must be equal), with the commands.
5. **Rows.** Copy the rows of `target/wf/studio-pack.rows` from `studio-pack` on (same names,
   same outdirs: the break builds into its own outdir so `studio-pack-embed` keeps a whole pack)
   into `scripts/orch/rows/host-api.rows`, additively:
   - `studio-pack` and `negctl-studio-pack` (`STUDIO_PACK_BREAK=1 ...; test $? -eq 1`);
   - `studio-pack-repro`: two builds into two outdirs, equal tarball sha256;
   - `studio-pack-embed` and `negctl-studio-pack-embed`, under `STUDIO_EMBED_BREAK=1`, which must
     exit 1.
6. Run each new row by hand once, and paste each exit code and its last 3 lines.

## Allowed paths

- New: `app/vite.pack.config.ts`, `scripts/studio-pack.sh`, `deploy/nav/embedpack.py`,
  `docs/contract/packaging.md`, `docs/measurements/studio-pack.md`. One new host page under `app/`
  if task 3 needs it.
- Edits:
  - `scripts/studio-embed.sh`;
  - `deploy/nav/embed.py`, only to call `embedpack`;
  - `docs/contract/host-api.md`, one line;
  - `scripts/orch/rows/host-api.rows`, additive;
  - `target/**`.
- Nothing under `packages/`, `crates/`, `src/` or the root `package.json`/lockfile. If the bundle
  needs a source change in `packages/graph-studio`, STOP and report it under "decisions needed",
  with the exact change.

## Return block

- the pack listing with sizes and the tarball sha256 of both builds;
- the `report.json` verdict counts of `studio-pack-embed`, plain/isolated/csp, and the wasm each
  run fetched;
- each new row's exit code;
- under "decisions taken" / "decisions needed": the lib-vs-input choice and anything that stopped
  you.

## Done when

Every row in `target/wf/studio-pack.rows` is green, and both negative controls are red for their own
reason.
