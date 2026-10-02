"""GM_GPU: the one place the studio's browser probes pick a GL backend, and read back its name.

Two environment knobs, read here and by the wrapper that builds the probe's container
(scripts/orch/gpu.sh, the only place the device is passed):

  GM_GPU=1        opt in to the host's GPU. The container is given /dev/dri, and Chromium drops
                  `--enable-unsafe-swiftshader` for the hardware ANGLE backend, so a WebGL2 number
                  is measured where a user runs it instead of on a CPU rasteriser. Host-local and
                  off by default: there is no GPU device in CI, and every probe runs unchanged
                  without it.
  GM_GPU_BREAK=1  the negative control. The check stays on while the flags fall back to software,
                  so a container that was never given the device lands on a software rasteriser and
                  the probe exits 2 instead of reporting a software number as a GPU one.

Every probe prints `renderer <name>` from WEBGL_debug_renderer_info's UNMASKED_RENDERER_WEBGL, and
under GM_GPU=1 a name that is a software rasteriser is a refusal to measure (SoftwareRasteriser),
not a warning. Only the perf wrappers put GM_GPU in the container environment, so the parity gates
(deploy/nav/backend.py) keep the software rasteriser that is the same on every host.

Caveat: the renderer string names the backend Chromium chose, not what rasterised every pixel of
the frame. The GPU blocklist can still hand part of a frame to the software rasteriser, and a
feature the backend lacks is drawn in software with the string unchanged — so this catches a silent
fallback of the whole context, not a partial one.
"""

import os

# `--use-angle=vulkan` is the arm kept. Measured on the host's device with the image's Mesa
# userspace, UNMASKED_RENDERER_WEBGL names the AMD device under both `--use-angle=vulkan`
# (through the Vulkan ICD) and `--use-angle=gl-egl` (through the gallium GL driver), and
# `--use-angle=gl` gets no WebGL2 context at all — so Vulkan is kept because it is the backend a
# Chromium on this class of GPU selects by itself, and this measurement is about what users run.
# Re-measure on another GPU before trusting another host: this table is one host's, not the flag's.
HARDWARE_FLAGS = ["--ignore-gpu-blocklist", "--use-angle=vulkan"]
# The arm every gate measured before: Chromium's own software rasteriser, no GPU process.
SOFTWARE_FLAGS = ["--disable-gpu", "--enable-unsafe-swiftshader"]
# What a Canvas2D-only probe asked for: no GL backend at all, which the software arm also implies.
NO_GL_FLAGS = ["--disable-gpu"]
# A renderer that names one of these drew on the CPU, whatever else the string says.
SOFTWARE_NAMES = ("swiftshader", "llvmpipe")

# The name of the GL backend this browser would give the studio: a throwaway context on a canvas of
# its own, never the studio's, so reading it cannot disturb a measurement in flight.
RENDERER_JS = """(() => {
  const gl = document.createElement('canvas').getContext('webgl2');
  if (!gl) return 'no webgl2 context';
  const info = gl.getExtension('WEBGL_debug_renderer_info');
  if (!info) return 'no WEBGL_debug_renderer_info';
  return String(gl.getParameter(info.UNMASKED_RENDERER_WEBGL));
})()"""


class SoftwareRasteriser(RuntimeError):
    """GM_GPU=1 asked for the host's GPU and the browser drew on the CPU instead."""


def wanted():
    """GM_GPU=1: measure on the host's GPU."""
    return os.environ.get("GM_GPU") == "1"


def broken():
    """GM_GPU_BREAK=1: keep the check, take the device away."""
    return os.environ.get("GM_GPU_BREAK") == "1"


def chrome_flags(draws_webgl=True):
    """The GL flags a probe launches Chromium with, as a fresh list the caller may extend."""
    if wanted() and not broken():
        return list(HARDWARE_FLAGS)
    return list(SOFTWARE_FLAGS) if draws_webgl else list(NO_GL_FLAGS)


def renderer(page):
    """What UNMASKED_RENDERER_WEBGL says this browser's WebGL2 backend is."""
    return page.evaluate(RENDERER_JS)


def is_software(name):
    return any(mark in name.lower() for mark in SOFTWARE_NAMES)


def check(page):
    """Print the renderer the probe drew on, and refuse a software one under GM_GPU=1.

    Raises SoftwareRasteriser, which every probe's main turns into exit 2: the number would be a
    CPU rasteriser's, and reporting it as the GPU arm's is the one failure this knob exists for.
    """
    name = renderer(page)
    print(f"renderer {name}")
    if wanted() and is_software(name):
        raise SoftwareRasteriser(
            f"GM_GPU=1 drew on a software rasteriser: {name}")
    return name