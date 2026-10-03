# syntax=docker/dockerfile:1
#
# Headless Chromium for the studio perf gate ONLY (scripts/studio-perf.sh). Never shipped.
# Debian packages, no vendor browser image (docs/decisions/images-from-debian.md).
#
# Ponytail: by default no GPU device is passed, so every frame is software-rastered. The numbers
# are comparable run to run on one host, and are NOT the user's browser's — those are read off
# the studio HUD and reported as a manual row. The Mesa userspace below is additive: it lets the
# perf probes opt into the host's GPU with GM_GPU=1 (scripts/orch/gpu.sh), which is the only way
# a WebGL2 number here is measured where a user runs it. With no device passed, nothing picks it
# up and the software arm is byte for byte the arm measured before.
#
#   docker build -f deploy/chromium.Dockerfile -t gm-chromium deploy
#   docker run --rm gm-chromium chromium --version
#   GM_GPU=1 scripts/studio-probe.sh settle 200000 webgl2     (needs /dev/dri on the host)

FROM debian:trixie-slim

ENV PYTHONDONTWRITEBYTECODE=1 PYTHONUNBUFFERED=1

# libgl1-mesa-dri: the GL and GLES gallium drivers, for --use-angle=gl-egl.
# libegl1: the EGL loader ANGLE's GL backend dlopens.
# mesa-vulkan-drivers + libvulkan1: the Vulkan ICD and loader, for --use-angle=vulkan.
RUN apt-get update \
 && apt-get install -y --no-install-recommends chromium python3 ca-certificates fonts-dejavu-core \
      libgl1-mesa-dri libegl1 mesa-vulkan-drivers libvulkan1 libdrm2 \
 && rm -rf /var/lib/apt/lists/*

RUN chromium --version && python3 --version
