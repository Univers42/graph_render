# syntax=docker/dockerfile:1
#
# Linters for the scripts under deploy/ and scripts/: shellcheck, shfmt, pyflakes.
# Debian packages only (docs/decisions/images-from-debian.md). Never shipped.
#
#   docker build -f deploy/lint.Dockerfile -t gm-lint deploy
#   docker run --rm -v "$PWD:/w" -w /w gm-lint shellcheck scripts/studio-perf.sh

FROM debian:trixie-slim

RUN apt-get update \
 && apt-get install -y --no-install-recommends shellcheck shfmt pyflakes3 python3 \
 && rm -rf /var/lib/apt/lists/*
