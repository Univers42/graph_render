# syntax=docker/dockerfile:1
#
# The graph-motor toolchain (docker/rust.Dockerfile) plus the standard library's source, for the
# shared-memory wasm build of browser threads model (a) (`scripts/orch/wasm-threads.sh`). The
# shipped std for wasm32-unknown-unknown is built without atomics, so that build recompiles std
# with `-Z build-std`, which needs `rust-src`. Our own image, not a vendor one: scripts/orch/gr
# builds it on first use with GR_IMAGE=ge-wasm-threads (scripts/orch/image.sh).
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
