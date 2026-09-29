# syntax=docker/dockerfile:1
#
# Headless Chromium for the studio perf gate ONLY (scripts/studio-perf.sh). Never shipped.
# Debian packages, no vendor browser image (docs/decisions/images-from-debian.md).
#
# Ponytail: no GPU in the container, so every frame is software-rastered. The numbers are
# comparable run to run on one host, and are NOT the user's browser's — those are read off
# the studio HUD and reported as a manual row.
#
#   docker build -f deploy/chromium.Dockerfile -t gm-chromium deploy
#   docker run --rm gm-chromium chromium --version

FROM debian:trixie-slim

ENV PYTHONDONTWRITEBYTECODE=1 PYTHONUNBUFFERED=1

RUN apt-get update \
 && apt-get install -y --no-install-recommends chromium python3 ca-certificates fonts-dejavu-core \
 && rm -rf /var/lib/apt/lists/*

RUN chromium --version && python3 --version
