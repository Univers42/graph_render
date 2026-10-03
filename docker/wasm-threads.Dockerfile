# syntax=docker/dockerfile:1
#
# The graph-motor toolchain (docker/rust.Dockerfile) plus the standard library's source, for the
# shared-memory wasm build of browser threads model (a) (`scripts/orch/wasm-threads.sh`). The
# shipped std for wasm32-unknown-unknown is built without atomics, so that build recompiles std
# with `-Z build-std`, which needs `rust-src`. Our own image, not a vendor one: build ge-rust
# first, then
#
#   docker build -f docker/wasm-threads.Dockerfile -t ge-wasm-threads .
#
# Kept out of ge-rust so the gate image stays the toolchain and nothing else.
#
# Behind a TLS-intercepting proxy, pass its CA as the same optional secret ge-rust takes.

ARG BASE=ge-rust
FROM ${BASE}

RUN --mount=type=secret,id=extra_ca,required=false,mode=0444 \
    set -eu; \
    if [ -s /run/secrets/extra_ca ]; then export RUSTUP_CA_FILE=/run/secrets/extra_ca; fi; \
    rustup component add rust-src
