"""The `gpu-mesh` harness's argv, its fixture list, the page call and the printed line.

`gpu-mesh.py` keeps the browser — the flag sets, the launches, the refusal; this file is the
part that is plain data in and text out, so it moved here when `--pass` would have pushed the
harness past 300 lines.

`--pass charge|link|collide` picks the module the page imports, `/target/gpu-js/gpu/<pass>.js`,
and its `run<Pass>(request, fault)` export. Charge is the default and its line is unchanged
byte for byte; every other pass returns a `PassReport` (`src/gpu/pass-report.ts`), and its line
prints every field of that.
"""
import json
from pathlib import Path

# The sizes `--only` accepts, as the fixture names' middle word. `1m` is the 1M pair.
SIZES = ("1k", "10k", "50k", "1m")
# The passes `--pass` accepts, each a module `gpu/<pass>.js` with a `run<Pass>` export.
PASSES = ("charge", "link", "collide")


def parse_args(argv):
    """`<arm> <dir> [--only 1k,10k,50k,1m] [--break <fault>] [--pass <pass>]`, or None.

    Returns `(arm, directory, only, fault, pass_name)`; `pass_name` defaults to `charge`.
    """
    if len(argv) < 3 or argv[1] not in ("software", "hardware"):
        return None
    flags = {"--only": None, "--break": None, "--pass": "charge"}
    rest = argv[3:]
    while rest:
        if rest[0] not in flags or len(rest) < 2:
            return None
        flags[rest[0]] = rest[1]
        rest = rest[2:]
    only = None if flags["--only"] is None else flags["--only"].split(",")
    if only is not None and not all(size in SIZES for size in only):
        return None
    if flags["--pass"] not in PASSES:
        return None
    return argv[1], argv[2], only, flags["--break"], flags["--pass"]


def fixtures(directory, only):
    """Every `mesh-*.gmfx` in `directory` that `--only` keeps, as the name without its suffix.

    The suffix is the page's to append: `path.name` already ends in `.gmfx`, and a harness that
    passed it through made the page fetch `mesh-1k-start.gmfx.gmfx` — a 404, whose HTML error
    page then failed the fixture's own magic check and looked like a kernel fault.
    """
    names = sorted(path.name for path in Path(directory).glob("mesh-*.gmfx"))
    stems = [name[: -len(".gmfx")] for name in names]
    if only is None:
        return stems
    return [stem for stem in stems if stem.split("-")[1] in only]


def call_js(name, arm, fault, pass_name):
    """`window.gpuMesh(name, arm, fault, pass)` as one expression, each argument a JS literal.

    `json.dumps`, not `!r`: `!r` writes the five letters `None` and the page answers
    `ReferenceError: None is not defined` — a harness bug in the costume of a kernel failure, on
    every fixture at once.
    """
    return (f"window.gpuMesh({json.dumps(name)}, {json.dumps(arm)}, {fault_js(fault)}, "
            f"{json.dumps(pass_name)})")


def fault_js(fault):
    """The fault argument: JS `undefined` when the harness was given no `--break`."""
    return "undefined" if fault is None else json.dumps(fault)


def line(report, ms):
    """One fixture's line: every report field, and the wall time the harness measured.

    A `ChargeReport` is the one with a `side`; anything else is a `PassReport`.
    """
    fields = charge_fields(report, ms) if "side" in report else pass_fields(report, ms)
    return f"{' '.join(report['failures'])} {fields}" if report["failures"] else fields


def charge_fields(report, ms):
    """Every `ChargeReport` field, in the order G1b printed them."""
    return (
        f"n={report['n']} state={report['state']} side={report['side']} "
        f"rmsAbs={report['rmsAbs']:.6g} rmsRef={report['rmsRef']:.6g} "
        f"rmsRel={report['rmsRel']:.6g} maxAbs={report['maxAbs']:.6g} "
        f"depositedUnits={report['depositedUnits']} repeatEqual={report['repeatEqual']} "
        f"boundsExact={report['boundsExact']} maxAbsGuard={report['maxAbsGuard']:.6g} "
        f"marks={report['marks'] or '(absent)'} fallback={report['fallback']} wallMs={ms * 1000:.1f}"
    )


def pass_fields(report, ms):
    """Every `PassReport` field: the pass's exactness checks as `name=True|False`."""
    exact = ",".join(f"{name}={held}" for name, held in sorted(report["exact"].items()))
    return (
        f"kind={report['kind']} n={report['n']} state={report['state']} "
        f"rmsAbs={report['rmsAbs']:.6g} rmsRef={report['rmsRef']:.6g} "
        f"rmsRel={report['rmsRel']:.6g} maxAbs={report['maxAbs']:.6g} "
        f"repeatEqual={report['repeatEqual']} exact={exact or '(none)'} "
        f"marks={report['marks'] or '(absent)'} fallback={report['fallback']} wallMs={ms * 1000:.1f}"
    )
