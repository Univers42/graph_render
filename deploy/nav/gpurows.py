"""Rows of the studio GPU-forces gate: the toggle off on load, on ticking on the device, the
Accuracy knob saying why it does nothing there, and off handing the settle back to the CPU.

Every action goes through the studio's own `dispatch`, the path the panel's switch takes, and
every position is read through the element's `view`. The arm is read from the console line the
bridge logs once per change (`packages/graph-studio/src/motor/bridge.ts`, `noteArm`), so a row
passes only on what a person reading the console would have been told.

Under GM_GPU_BREAK=1 the browser has no hardware adapter, the toggle falls back to the CPU and
says so, and `gpu-on-ticks-on-the-device` must go red: that is the negative control.
"""
import time

from liverows import HOST, MOVING_PX, SAMPLE_S, moved, not_run, positions, row

ON_GPU = "forces tick on the GPU"
ON_CPU = "GPU forces run on the CPU"
# Caveat: opening a device and the first batch are timed by this host's driver; 20 s is a wait
# for an adapter that may answer slowly, so a slower host reads as a FAIL with the last line seen.
ARM_WAIT_S = 20.0

STATE = f"""
(() => {{
  const s = {HOST}.studio;
  const state = s.store.get();
  const gpu = s.registry.find('forces.gpu');
  const accuracy = s.registry.find('forces.accuracy');
  return {{
    gpu: gpu === undefined ? null : gpu.params[0].value(state),
    accuracy: accuracy === undefined || accuracy.available === undefined ? null : accuracy.available(state),
    error: state.error === null ? null : state.error.title + ': ' + state.error.detail,
    arm: state.log.filter((e) => e.command === 'forces.gpu' && e.ok).map((e) => e.message),
  }};
}})()
"""


def state(studio):
    return studio.page.evaluate(STATE)


def dispatch(studio, action, on):
    flag = "true" if on else "false"
    studio.page.evaluate(f"{HOST}.studio.dispatch('{action}', {{ on: {flag} }})")


def arm_line(studio, since):
    """The first arm line logged after the first `since`, or None by the deadline."""
    deadline = time.monotonic() + ARM_WAIT_S
    while time.monotonic() < deadline:
        lines = state(studio)["arm"][since:]
        if lines:
            return lines[0]
        time.sleep(0.2)
    return None


def travel(studio):
    first = positions(studio)
    time.sleep(SAMPLE_S)
    return moved(first, positions(studio))


def row_off_by_default(studio):
    expectation = "on load the GPU switch is off, Accuracy is available, and no error is shown"
    at = state(studio)
    measured = f"gpu={at['gpu']} accuracy={at['accuracy']!r} error={at['error']}"
    return row("gpu-off-by-default", expectation, measured,
               at["gpu"] is False and at["accuracy"] is None and at["error"] is None)


def row_on_the_device(studio):
    expectation = f"switched on, the console says `{ON_GPU} (<adapter>)` and nodes move over {MOVING_PX} in {SAMPLE_S}s"
    since = len(state(studio)["arm"])
    dispatch(studio, "forces.gpu", True)
    line = arm_line(studio, since)
    if line is None:
        return not_run("gpu-on-ticks-on-the-device", expectation, "no arm line", f"none in {ARM_WAIT_S}s")
    distance = travel(studio)
    at = state(studio)
    measured = f"`{line}`, travel {distance}, error={at['error']}"
    ok = line.startswith(ON_GPU) and distance is not None and distance > MOVING_PX and at["error"] is None
    return row("gpu-on-ticks-on-the-device", expectation, measured, ok)


def row_accuracy_says_why(studio):
    expectation = "with the switch on, Accuracy is unavailable with a reason"
    at = state(studio)
    measured = f"gpu={at['gpu']} accuracy={at['accuracy']!r}"
    return row("gpu-accuracy-says-why", expectation, measured,
               at["gpu"] is True and isinstance(at["accuracy"], str) and "theta" in at["accuracy"])


def row_off_hands_back(studio):
    expectation = f"switched off, Accuracy is back, Animate moves nodes over {MOVING_PX} and no error is shown"
    dispatch(studio, "forces.gpu", False)
    time.sleep(SAMPLE_S)
    dispatch(studio, "forces.animate", True)
    time.sleep(SAMPLE_S)
    distance = travel(studio)
    at = state(studio)
    measured = f"gpu={at['gpu']} accuracy={at['accuracy']!r} travel {distance} error={at['error']}"
    ok = at["gpu"] is False and at["accuracy"] is None and distance is not None and distance > MOVING_PX
    return row("gpu-off-hands-back", expectation, measured, ok and at["error"] is None)


def run_rows(studio):
    return [row_off_by_default(studio), row_on_the_device(studio), row_accuracy_says_why(studio),
            row_off_hands_back(studio)]
