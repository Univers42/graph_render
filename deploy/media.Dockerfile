# syntax=docker/dockerfile:1
#
# ffmpeg for the showcase video (scripts/showcase.sh). Never shipped, never a gate.
# Debian packages, no vendor image (docs/decisions/images-from-debian.md).
#
#   docker build -f deploy/media.Dockerfile -t gm-media deploy
#   scripts/orch/drun --rm gm-media ffmpeg -version

FROM debian:trixie-slim

ENV PYTHONDONTWRITEBYTECODE=1 PYTHONUNBUFFERED=1

# fonts-inter: the caption face; fontconfig: fc-match finds its file.
RUN apt-get update \
 && apt-get install -y --no-install-recommends ffmpeg python3 fonts-inter fontconfig \
 && rm -rf /var/lib/apt/lists/*

RUN ffmpeg -hide_banner -version | head -n 1 && fc-match -f '%{file}\n' 'Inter:semibold'
