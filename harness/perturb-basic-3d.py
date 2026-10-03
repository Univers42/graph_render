"""Negative control for oracle-basic-3d: copies the fixtures, perturbs ONE coordinate far
past its ceiling, re-seals the manifest, and leaves the result to the unchanged arm. The
COMPARISON, not the sealing, must be what turns red.

Same shape as harness/perturb-closed-form.py, and for the same reason it exists: a negative
control that perturbs nothing passes vacuously, so a run of this differential with nothing
wrong in it proves only that the harness can add up.

Which coordinates are perturbed, and why not `layout.basic3d.cube`'s interior: the interior is
NOT compared (it is drawn from this crate's own Mulberry32 against the reference's
module-level global RandomState, so the two disagree by design), so moving it would turn
nothing red and this control would pass for the wrong reason. The perturbations go into the
**sphere's** `z` and the **spiral's** `x` — closed forms with no stream, gated at 1e-6 — and by
0.01, which is 10 000 times the ceiling. Two functions, because the arm now gates four and a
control that only reaches the first of the closed forms says nothing about the three after
it; `spiral` is the one whose comparison is an arc-length inversion rather than a
transcription, so it is the row most worth proving can go red.

    docker run … ge-python-oracle python3 harness/perturb-basic-3d.py src dst
"""
import hashlib
import json
import os
import shutil
import sys
import tempfile

if len(sys.argv) != 3:
    sys.exit("usage: perturb-basic-3d.py <fixtures-dir> <perturbed-dir>")
src, dst = sys.argv[1], sys.argv[2]

# The os.replace() writes below rely on this invariant, which is why the argument handling
# above is left exactly as it is: `dst` is scratch under target/, it is always recreated
# from scratch by the rmtree+copytree pair below, and it is NEVER a committed fixture.
# A torn write there costs one rerun; it can never corrupt a tree anyone diffs.
shutil.rmtree(dst, ignore_errors=True)
shutil.copytree(src, dst)


def replace(path, text):
    """Put `text` at `path` atomically: a temp file in the SAME directory, then
    os.replace(), which is a rename within one filesystem and therefore all-or-nothing.
    A reader of `dst` sees either the whole old file or the whole new one."""
    fd, tmp = tempfile.mkstemp(dir=os.path.dirname(path), prefix=".perturb-", suffix=".tmp")
    try:
        with os.fdopen(fd, "w") as out:
            out.write(text)
        os.replace(tmp, path)
    except BaseException:
        os.unlink(tmp)
        raise


path = os.path.join(dst, "basic-3d.jsonl")
before = open(path, "rb").read()
lines = before.decode().splitlines()
case = json.loads(lines[1])
case["sphere"]["z"][0] += 0.01
case["spiral"]["x"][0] += 0.01
lines[1] = json.dumps(case, sort_keys=True, separators=(",", ":"))
replace(path, "\n".join(lines) + "\n")
after = open(path, "rb").read()
# A control that perturbs nothing passes vacuously, so the perturbation itself is checked
# here rather than left to the downstream comparison to notice.
if after == before:
    sys.exit(f"{path} was not changed by the perturbation")

manifest_path = os.path.join(dst, "basic-3d-manifest.json")
manifest = json.load(open(manifest_path))
manifest["sha256"]["basic-3d.jsonl"] = hashlib.sha256(after).hexdigest()
replace(manifest_path, json.dumps(manifest, sort_keys=True, separators=(",", ":")))

print(json.dumps({"perturbed": path, "delta_bytes": len(after) - len(before)}))