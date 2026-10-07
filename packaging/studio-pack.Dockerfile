# The studio pack (docs/contract/packaging.md) as an OCI image: FROM scratch, the six pack files
# under /pack/ and nothing else. Built by .github/workflows/studio-pack.yml, whose context is
# `git archive <commit>` — so it can pack a commit older than this file.
#
# It mirrors scripts/studio-pack.sh step for step instead of running it: that script drives docker
# itself and builds ge-rust FROM an unpinned debian:trixie-slim, and a published image is built
# from bases pinned by digest. The cost is drift: a change to studio-pack.sh, studio.sh's wasm
# build, scripts/orch/wasm-threads.sh or the toolchain pins must be repeated here. The workflow
# refuses a commit whose copies of those files differ from the ones this mirrors (77b68d9b), and
# publishes nothing unless this pack is byte-identical to the commit's own studio-pack.sh output.
#
# Ponytail: apt packages are not pinned (whatever trixie ships on the build day). gcc only links
# host build scripts and the wasm links with rust-lld, so the pack is expected not to move with
# them — expected, not proven; two builds on one day cannot see it. The mirror is of
# studio-pack.sh at 77b68d9b: when the workflow's drift guard fails, update this file first.

ARG DEBIAN_IMAGE=debian:trixie-slim@sha256:a99cfc517144bc59b1978475ec53b46ecabec7e43635402ee5b77cc54cd1b20a
ARG NODE_IMAGE=node:22.23.3-slim@sha256:43ac6c60b8f89723f746e8a92ce91abd5017e627ce1ddfe4238355d3a30b772c

# hadolint ignore=DL3006
FROM ${DEBIAN_IMAGE} AS toolchain
ARG RUST_VERSION=1.98.1
ARG RUSTUP_VERSION=1.29.1
ARG RUSTUP_SHA256=dda7234360b7f578ca8b0ddcb80145646fa61a67c1720a5abc7051b35c9fcb71
# Same homes as docker/rust.Dockerfile: the paths rustc embeds in the wasm stay those of a local build.
ENV CARGO_HOME=/opt/cargo RUSTUP_HOME=/opt/rustup PATH=/opt/cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
SHELL ["/bin/bash", "-o", "pipefail", "-c"]
# hadolint ignore=DL3008
RUN set -eu; \
    [ "$(dpkg --print-architecture)" = amd64 ] || { echo "studio-pack: amd64 only (RUSTUP_SHA256 is the amd64 one)" >&2; exit 1; }; \
    apt-get update; \
    apt-get install -y --no-install-recommends ca-certificates curl gcc libc6-dev; \
    rm -rf /var/lib/apt/lists/*; \
    curl -fsSLo /tmp/rustup-init "https://static.rust-lang.org/rustup/archive/${RUSTUP_VERSION}/x86_64-unknown-linux-gnu/rustup-init"; \
    echo "${RUSTUP_SHA256}  /tmp/rustup-init" | sha256sum -c -; \
    chmod +x /tmp/rustup-init; \
    /tmp/rustup-init -y --no-modify-path --profile minimal --default-toolchain "${RUST_VERSION}" \
      --target wasm32-unknown-unknown; \
    rm /tmp/rustup-init

# Two stages, as upstream has two images (ge-rust, ge-wasm-threads): with rust-src installed,
# rustc embeds std's real source paths in panic locations instead of /rustc/<hash>/, and the
# serial wasm came out 377 bytes off upstream's (measured at 77b68d9b). So rust-src exists only
# for the threads build. Each runs `cargo fetch --locked` first; the builds are the exact
# commands of scripts/studio.sh build_wasm and scripts/orch/wasm-threads.sh, without --locked.
FROM toolchain AS wasm-serial
WORKDIR /w
COPY . .
RUN cargo fetch --locked && cargo build -p graph-wasm --target wasm32-unknown-unknown --release

FROM toolchain AS wasm-threads
RUN rustup component add rust-src
WORKDIR /w
COPY . .
RUN cargo fetch --locked && \
    RUSTC_BOOTSTRAP=1 \
    RUSTFLAGS="-C target-feature=+atomics,+bulk-memory,+mutable-globals -C link-arg=--shared-memory -C link-arg=--import-memory -C link-arg=--initial-memory=67108864 -C link-arg=--max-memory=4294967296 -C link-arg=--export=__wasm_init_tls -C link-arg=--export=__tls_size -C link-arg=--export=__tls_align -C link-arg=--export=__tls_base -C link-arg=--export=__stack_pointer" \
    cargo build --release -p graph-wasm --target wasm32-unknown-unknown \
      -Z build-std=std,panic_abort --features threads --target-dir target/wasm-threads

# hadolint ignore=DL3006
FROM ${NODE_IMAGE} AS pack
ARG GRAPH_RENDER_SHA
WORKDIR /w/app
COPY app/package.json app/package-lock.json ./
RUN NPM_CONFIG_UPDATE_NOTIFIER=false npm ci --ignore-scripts --no-audit --no-fund
WORKDIR /w
COPY . .
WORKDIR /w/app
RUN PACK_MODE=pack PACK_OUT_DIR=/out node_modules/.bin/vite build --config vite.pack.config.ts
COPY --from=wasm-serial /w/target/wasm32-unknown-unknown/release/graph_wasm.wasm /out/graph_wasm.wasm
COPY --from=wasm-threads /w/target/wasm-threads/wasm32-unknown-unknown/release/graph_wasm.wasm /out/graph_wasm_threads.wasm
# scripts/orch/wasm-threads.sh's import/export check, verbatim (single quotes: it is JavaScript)
# hadolint ignore=SC2016
RUN node -e ' \
const m = new WebAssembly.Module(require("fs").readFileSync(process.argv[1])); \
const im = WebAssembly.Module.imports(m).map((i) => `${i.module}.${i.name}:${i.kind}`); \
const ex = new Set(WebAssembly.Module.exports(m).map((e) => e.name)); \
const need = ["__wasm_init_tls", "__tls_size", "__stack_pointer", "gm_thread_serve", "gm_run_threaded", "gm_force_session_tick_threaded"]; \
const missing = need.filter((n) => !ex.has(n)); \
const ok = im.length === 1 && im[0] === "env.memory:memory" && missing.length === 0; \
console.log(`imports ${im.join(",")} · missing exports ${missing.join(",") || "none"}`); \
process.exit(ok ? 0 : 1); \
' /out/graph_wasm_threads.wasm
# ctl is a named build context (--build-context ctl=packaging), not a stage.
# hadolint ignore=DL3022
COPY --from=ctl studio-pack-manifest.sh /usr/local/bin/
RUN studio-pack-manifest.sh /out "${GRAPH_RENDER_SHA}" /w

# The bare pack, for `--output type=local` (the workflow's two-build comparison).
FROM scratch AS pack-files
COPY --from=pack /out/ /

FROM scratch AS image
ARG GRAPH_RENDER_SHA
ARG SOURCE_URL
ARG CREATED
ARG RECIPE_REV
# revision is the packed commit; recipe_rev the commit this Dockerfile and its workflow came from.
LABEL org.opencontainers.image.revision="${GRAPH_RENDER_SHA}" \
      org.univers42.graph-studio-pack.recipe-rev="${RECIPE_REV}" \
      org.opencontainers.image.source="${SOURCE_URL}" \
      org.opencontainers.image.created="${CREATED}"
COPY --from=pack-files / /pack/
