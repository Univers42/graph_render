# syntax=docker/dockerfile:1
#
# cargo-mutants on top of the graph-motor toolchain image (docker/rust.Dockerfile), for
# the per-phase mutation run (ONBOARDING §6.2). Our own image, not a vendor one: build
# ge-rust first, then
#
#   docker build -f docker/mutants.Dockerfile -t ge-mutants .
#   docker run --rm -v "$PWD:/w" ge-mutants cargo mutants --in-diff phase.diff
#
# Kept out of ge-rust so the gate image stays the toolchain and nothing else. The
# mutation settings (excluded arid nodes, each with its reason) are in .cargo/mutants.toml.
#
# Behind a TLS-intercepting proxy, pass its CA as the same optional secret ge-rust takes.

ARG BASE=ge-rust
FROM ${BASE}

ARG CARGO_MUTANTS_VERSION=27.1.0

RUN --mount=type=secret,id=extra_ca,required=false,mode=0444 \
    set -eu; \
    if [ -s /run/secrets/extra_ca ]; then export CARGO_HTTP_CAINFO=/run/secrets/extra_ca; fi; \
    cargo install --locked cargo-mutants --version "${CARGO_MUTANTS_VERSION}"; \
    rm -rf "${CARGO_HOME}/registry" "${CARGO_HOME}/git"
