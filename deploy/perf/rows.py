"""The studio perf budgets, and the verdict on one measured report.

The budgets were written down before the first measurement (the approved plan, §4); the
baseline file holds what the first studio reached and is only ever a comparison point.
"""

import os

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
RECORDED_NODES = 10000
# Arrow heads are one fill; glow is two layers per colour, so its budget is set by the palette.
ARROW_FILL_BUDGET = 1
# Segments per edge stroke, the CHUNK in packages/graph-render/src/canvas2d/edges.ts. Each
# style needs ceil(its edges / EDGE_CHUNK) strokes, which is at most one per style plus
# floor(all edges / EDGE_CHUNK).
# Ponytail: the lit pass's edges are not in drawnEdges, so a focused hub with more than
# EDGE_CHUNK lit edges reads as over budget (a false FAIL, the safe direction).
EDGE_CHUNK = 2048
# Mirrored by MIXED_EDGE_BUDGET in packages/graph-render/src/canvas2d/edgeGradient.ts: past
# this many mixed edges a frame draws them in their mean colour, batched per colour pair, and
# never takes a gradient. So the gradient strokes a frame may add are counted against it.
MIXED_EDGE_BUDGET = 512
STATS_NODES = (2000, 10000)
# STUDIO_PERF_BREAK=1 is the negative control: the stroke budget becomes one less than what
# was measured, so the row must fail.
BREAK = "STUDIO_PERF_BREAK"
# Ponytail: the counter rows are exact counts read from the view, not timings, so they hold
# on a loaded host. They count one settled frame per case: a frame that draws a sample while
# the view moves (edges.ts MOVING_BUDGET) is not measured, and a hover redraw is not driven
# (probes/stats.js says why).


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
    if not parts:
        return _row("perf-fps", expectation, f"no case at DPR {FPS_DPR}", "NOT-RUN")
    return _row("perf-fps", expectation, "; ".join(parts), _verdict(passed))


def _stats_cases(report):
    cases = [case for case in report["frames"] if case["nodes"] in STATS_NODES and case["dpr"] == 1]
    return cases if cases and all("stats" in case for case in cases) else None


def _counter_row(report, name, expectation, check):
    cases = _stats_cases(report)
    if cases is None:
        return _row(name, expectation, "the stats probe did not run", "NOT-RUN")
    results = [(case["nodes"], case["stats"], *check(case["stats"])) for case in cases]
    measured = "; ".join(f"{nodes} nodes: {text}" for nodes, _, _, text in results)
    return _row(name, expectation, measured, _verdict(all(ok for _, _, ok, _ in results)))


def _edge_batch(report):
    def check(stats):
        broken = os.environ.get(BREAK) == "1"
        # In the gradient mode each mixed edge takes a gradient of its own, and there are
        # never more of them than the renderer's budget says.
        gradient = stats["gradientStrokes"]
        allowed = stats["edgeStyles"] + stats["drawnEdges"] // EDGE_CHUNK + min(gradient, MIXED_EDGE_BUDGET)
        budget = stats["strokeCalls"] - 1 if broken else allowed
        ok = (stats["drawnEdges"] > 0 and stats["strokeCalls"] <= budget
              and stats["arrowFills"] <= ARROW_FILL_BUDGET
              and (gradient == 0 or stats["mixedEdges"] <= MIXED_EDGE_BUDGET))
        return ok, (f"{stats['strokeCalls']} strokes for {stats['edgeStyles']} style(s), {stats['drawnEdges']} edges, "
                    f"{gradient} gradient stroke(s) for {stats['mixedEdges']} mixed edge(s), "
                    f"{stats['arrowFills']} arrow fill(s), {stats['glowFills']} glow fill(s) (budget {budget})")
    return _counter_row(report, "perf-edge-batch",
                        f"stroke() calls per frame <= edge styles + edges / {EDGE_CHUNK} + gradient strokes "
                        f"(mixed edges <= {MIXED_EDGE_BUDGET}); arrow fills <= {ARROW_FILL_BUDGET}", check)


def _sprite_cache(report):
    def check(stats):
        return stats["spritesSecondFrame"] == 0 and stats["redrawFrames"] > 0, (
            f"{stats['spritesSecondFrame']} sprites rasterised on the second frame ({stats['drawnLabels']} labels, "
            f"{stats['redrawFrames']} frame(s))")
    return _counter_row(report, "perf-sprite-cache", "0 label sprites rasterised on a second identical frame", check)


def _label_layout(report):
    def check(stats):
        ok = (stats["layoutRunsRedraw"] == 0 and stats["layoutRunsParked"] == 0
              and stats["layoutRunsZoom"] >= 1 and stats["redrawFrames"] > 0)
        return ok, (f"redraw {stats['layoutRunsRedraw']}, parked {stats['layoutRunsParked']}, "
                    f"after zoom {stats['layoutRunsZoom']}")
    return _counter_row(report, "perf-label-layout",
                        "label layout runs: 0 over a redraw, 0 over a parked frame, >= 1 after a zoom", check)


def _recorded(report):
    parts = []
    for case in report["frames"]:
        if case["nodes"] != RECORDED_NODES:
            continue
        value = case["notRun"] if "notRun" in case else f"{worst_fps(case)} fps, p95 JS {worst_js_p95(case)} ms"
        parts.append(f"DPR {case['dpr']}: {value}")
    edges = next((case["stats"]["edges"] for case in report["frames"] if case["nodes"] == RECORDED_NODES and "stats" in case), "?")
    # Ponytail: pan/zoom p95 on a shared, software-rastered host; recorded, never gated.
    return _row("perf-10k", f"{RECORDED_NODES} nodes / {edges} edges pan/zoom, recorded not gated",
                "; ".join(parts), "RECORDED", gating=False)


def judge(report, baseline):
    return [_idle(report), _block(report), _js(report), _fps(report, baseline),
            _edge_batch(report), _sprite_cache(report), _label_layout(report), _recorded(report)]


def baseline_of(report):
    fps = {f"{case['nodes']}@{case['dpr']}": worst_fps(case)
           for case in report["frames"] if "notRun" not in case}
    return {"commit": report["commit"], "browser": report["browser"],
            "viewport": report["viewport"], "fps": fps}


def _frame_lines(report):
    lines = ["| nodes | DPR | layout | open ms | canvas | worst fps | JS mean ms | JS p95 ms | long tasks | longest ms "
             "| React commits | React renders | at open |", "|---:|---:|---|---:|---|---:|---:|---:|---:|---:|---:|---:|---|"]
    for case in report["frames"]:
        if "notRun" in case:
            lines.append(f"| {case['nodes']} | {case['dpr']} | | | not run: {case['notRun']} | | | | | | | | |")
            continue
        phases = case["phases"]
        mean = max(phase["jsMeanMs"] for phase in phases)
        size = "x".join(str(side) for side in case["canvas"])
        at_open = case.get("reactAtOpen") or {}
        lines.append(f"| {case['nodes']} | {case['dpr']} | `{case.get('layout', '?')}` | {case.get('openMs', '?')} "
                     f"| {size} | {worst_fps(case)} "
                     f"| {mean} | {worst_js_p95(case)} | {_total(phases, 'longTasks')} "
                     f"| {max(phase['longestTaskMs'] for phase in phases)} | {_total(phases, 'reactCommits')} "
                     f"| {_total(phases, 'reactRendered')} "
                     f"| {at_open.get('commits', '?')} commits, {at_open.get('rendered', '?')} renders |")
    return lines


def _total(phases, key):
    values = [phase[key] for phase in phases]
    return "?" if None in values else sum(values)


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
