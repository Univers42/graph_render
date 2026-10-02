#!/usr/bin/env bash
# wasm-threads.sh — build browser threads model (a): graph-wasm with `--features threads` over one
# shared, imported memory, into target/wasm-threads/wasm32-unknown-unknown/release/graph_wasm.wasm.
#
# The shipped std for wasm32-unknown-unknown has no atomics, so std is rebuilt with them
# (`-Z build-std`, rust-src in the ge-wasm-threads image: docker/wasm-threads.Dockerfile).
# RUSTC_BOOTSTRAP=1 lets the pinned stable toolchain take `-Z`; no nightly is pulled.
# The linker exports the TLS and stack globals each helper instance sets before it serves
# (harness/wasm-threads.mjs). The initial memory is fixed at 64 MiB so the host creates the
# shared Memory with that size before instantiating; the maximum is the 4 GiB wasm32 limit.
#
# Its own target dir: the default artifact (import-free, `refuseImports`) is never touched.
# Exit: 0 built and checked, 1 the artifact lacks a required import or export, 2 could not build.
set -uo pipefail
here=$(dirname "$(readlink -f "$0")")
flags=(
  -C target-feature=+atomics,+bulk-memory,+mutable-globals
  -C link-arg=--shared-memory -C link-arg=--import-memory
  -C link-arg=--initial-memory=67108864 -C link-arg=--max-memory=4294967296
)
for g in __wasm_init_tls __tls_size __tls_align __tls_base __stack_pointer; do
  flags+=(-C "link-arg=--export=$g")
done
GR_IMAGE=${GR_IMAGE:-ge-wasm-threads} "$here/gr" -e RUSTC_BOOTSTRAP=1 -e "RUSTFLAGS=${flags[*]}" \
  cargo build --release -p graph-wasm --target wasm32-unknown-unknown \
  -Z build-std=std,panic_abort --features threads --target-dir target/wasm-threads || exit 2
"$here/gr" node -e '
const m = new WebAssembly.Module(require("fs").readFileSync(process.argv[1]));
const im = WebAssembly.Module.imports(m).map((i) => `${i.module}.${i.name}:${i.kind}`);
const ex = new Set(WebAssembly.Module.exports(m).map((e) => e.name));
const need = ["__wasm_init_tls", "__tls_size", "__stack_pointer", "gm_thread_serve", "gm_run_threaded"];
const missing = need.filter((n) => !ex.has(n));
const ok = im.length === 1 && im[0] === "env.memory:memory" && missing.length === 0;
console.log(`imports ${im.join(",")} · missing exports ${missing.join(",") || "none"}`);
process.exit(ok ? 0 : 1);
' target/wasm-threads/wasm32-unknown-unknown/release/graph_wasm.wasm
