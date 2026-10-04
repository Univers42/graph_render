# syntax=docker/dockerfile:1
#
# cargo-deny on top of the graph-motor toolchain image (docker/rust.Dockerfile), for the
# supply-chain gate (scripts/orch/rows/audit.rows). Our own image, not a vendor one: build
# ge-rust first, then
#
#   docker build -f docker/audit.Dockerfile -t ge-audit .
#   docker run --rm -v "$PWD:/w" ge-audit cargo deny --manifest-path Cargo.toml check advisories
#
# Kept out of ge-rust so the gate image stays the toolchain and nothing else, the same reason
# docker/mutants.Dockerfile is its own. The policy is deny.toml at the repo top; --config for the
# server workspace, whose lockfile is server/Cargo.lock.
#
# CARGO_DENY_VERSION=0.20.2: chosen from the crates.io version list, the newest release with
# rust_version = 1.88.0 (published 2026-07-09, not yanked). 1.88.0 <= the image's 1.98.1
# (docker/rust.Dockerfile:23), so it builds on this toolchain; every 0.20.x release carries the
# same MSRV. Bumping it means bumping the pinned rustc too if the MSRV ever moves past 1.98.
#
# Behind a TLS-intercepting proxy, pass its CA as the same optional secret ge-rust takes.

ARG BASE=ge-rust
FROM ${BASE}

ARG CARGO_DENY_VERSION=0.20.2

# git is the one package this image adds on top of the toolchain: cargo-deny clones the RustSec
# advisory-db with it, and without it `check advisories` fails on the fetch, not on a finding.
RUN --mount=type=secret,id=extra_ca,required=false,mode=0444 \
    set -eu; \
    src=/etc/apt/sources.list.d/debian.sources; \
    if [ -s /run/secrets/extra_ca ]; then \
      sed -i 's#http://#https://#' "$src"; \
      echo 'Acquire::https::CAInfo "/run/secrets/extra_ca";' > /etc/apt/apt.conf.d/99extra-ca; \
    fi; \
    apt-get update; \
    apt-get install -y --no-install-recommends git; \
    sed -i 's#https://#http://#' "$src"; \
    rm -rf /var/lib/apt/lists/* /etc/apt/apt.conf.d/99extra-ca

RUN --mount=type=secret,id=extra_ca,required=false,mode=0444 \
    set -eu; \
    if [ -s /run/secrets/extra_ca ]; then export CARGO_HTTP_CAINFO=/run/secrets/extra_ca; fi; \
    cargo install --locked cargo-deny --version "${CARGO_DENY_VERSION}"; \
    rm -rf "${CARGO_HOME}/registry" "${CARGO_HOME}/git"