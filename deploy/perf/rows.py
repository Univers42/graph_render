"""The studio perf budgets, and the verdict on one measured report.

The budgets were written down before the first measurement (the approved plan, §4); the
baseline file holds what the first studio reached and is only ever a comparison point.
"""

IDLE_CALLBACKS_PER_SECOND = 0
BLOCK_MS = 50
JS_P95_MS = 4
JS_NODES = 2000
FPS_GAIN = 2
FPS_DPR = 2
FPS_NODES = (120, 2000)
# The frame clock stops at 60 Hz, so twice a 31 fps baseline cannot be measured. Above
# this floor a case is at the cap and passes.
FPS_AT_CAP = 54
RECORDED_NODES = 20000


def worst_fps(case):
    return min(phase["fps"] for phase in case["phases"])


def worst_js_p95(case):
    return max(phase["jsP95Ms"] for phase in case["phases"])


def _row(name, expectation, measured, verdict, gating=True):
    return {"row": name, "expectation": expectation, "measured": measured,
            "verdict": verdict, "gating": gating}


def _verdict(passed):
    return "PASS" if passed else "FAIL"


def _idle(report):
    idle = report["idle"]
    expectation = f"{IDLE_CALLBACKS_PER_SECOND} animation callbacks/s when parked"
    if "notRun" in idle:
        return _row("perf-idle", expectation, idle["notRun"], "NOT-RUN")
    rate = idle["callbacksPerSecond"]
    return _row("perf-idle", expectation, f"{rate}/s", _verdict(rate <= IDLE_CALLBACKS_PER_SECOND))


def _block(report):
    expectation = f"main thread blocked <= {BLOCK_MS} ms by any layout"
    runs = [(case["nodes"], run) for case in report["block"] for run in case.get("layouts", [])]
    if not runs or any("notRun" in case for case in report["block"]):
        return _row("perf-block", expectation, "a case did not run", "NOT-RUN")
    failed = [f"{run['id']}@{nodes}: {run['error']}" for nodes, run in runs if run["error"]]
    if failed:
        return _row("perf-block", expectation, "; ".join(failed), "FAIL")
    nodes, worst = max(runs, key=lambda pair: pair[1]["blockMs"])
    measured = f"{worst['blockMs']} ms ({worst['id']} at {nodes} nodes)"
    return _row("perf-block", expectation, measured, _verdict(worst["blockMs"] <= BLOCK_MS))


def _js(report):
    expectation = f"p95 JS per frame <= {JS_P95_MS} ms at {JS_NODES} nodes"
    cases = [case for case in report["frames"] if case["nodes"] == JS_NODES]
    if not cases or any("notRun" in case for case in cases):
        return _row("perf-js", expectation, "a case did not run", "NOT-RUN")
    worst = max(worst_js_p95(case) for case in cases)
    return _row("perf-js", expectation, f"{worst} ms", _verdict(worst <= JS_P95_MS))


def _fps(report, baseline):
    expectation = f">= {FPS_GAIN}x baseline at DPR {FPS_DPR} (or the {FPS_AT_CAP} fps cap)"
    if baseline is None:
        return _row("perf-fps", expectation, "no baseline file", "NOT-RUN")
    parts = []
    passed = True
    for case in report["frames"]:
        if case["dpr"] != FPS_DPR or case["nodes"] not in FPS_NODES:
            continue
        if "notRun" in case:
            return _row("perf-fps", expectation, case["notRun"], "NOT-RUN")
        before = baseline["fps"][f"{case['nodes']}@{case['dpr']}"]
        floor = min(FPS_GAIN * before, FPS_AT_CAP)
        now = worst_fps(case)
        passed = passed and now >= floor
        parts.append(f"{case['nodes']} nodes: {now} fps (was {before}, floor {floor})")
    return _row("perf-fps", expectation, "; ".join(parts), _verdict(passed and bool(parts)))


def _recorded(report):
    parts = []
    for case in report["frames"]:
        if case["nodes"] != RECORDED_NODES:
            continue
        value = case["notRun"] if "notRun" in case else f"{worst_fps(case)} fps"
        parts.append(f"DPR {case['dpr']}: {value}")
    return _row("perf-20k", "recorded, not gated", "; ".join(parts), "RECORDED", gating=False)


def judge(report, baseline):
    return [_idle(report), _block(report), _js(report), _fps(report, baseline), _recorded(report)]


def baseline_of(report):
    fps = {f"{case['nodes']}@{case['dpr']}": worst_fps(case)
           for case in report["frames"] if "notRun" not in case}
    return {"commit": report["commit"], "browser": report["browser"],
            "viewport": report["viewport"], "fps": fps}


def _frame_lines(report):
    lines = ["| nodes | DPR | canvas | worst fps | JS mean ms | JS p95 ms |", "|---:|---:|---|---:|---:|---:|"]
    for case in report["frames"]:
        if "notRun" in case:
            lines.append(f"| {case['nodes']} | {case['dpr']} | not run: {case['notRun']} | | | |")
            continue
        mean = max(phase["jsMeanMs"] for phase in case["phases"])
        size = "x".join(str(side) for side in case["canvas"])
        lines.append(f"| {case['nodes']} | {case['dpr']} | {size} | {worst_fps(case)} "
                     f"| {mean} | {worst_js_p95(case)} |")
    return lines


def _block_lines(report):
    lines = ["| nodes | layout | wall ms | blocked ms |", "|---:|---|---:|---:|"]
    for case in report["block"]:
        for run in case.get("layouts", []):
            note = f" ({run['error']})" if run["error"] else ""
            lines.append(f"| {case['nodes']} | `{run['id']}`{note} | {run['wallMs']} | {run['blockMs']} |")
    return lines


def table(report):
    head = [f"# studio-perf — {report['label']}", "",
            f"commit `{report['commit']}` · driver `{report['driver']}` · {report['browser']} · "
            f"viewport {report['viewport'][0]}x{report['viewport'][1]} · software raster", "",
            "| row | expectation | measured | verdict |", "|---|---|---|---|"]
    rows = [f"| `{row['row']}` | {row['expectation']} | {row['measured']} | {row['verdict']} |"
            for row in report["rows"]]
    return "\n".join([*head, *rows, "", *_frame_lines(report), "", *_block_lines(report), ""])
