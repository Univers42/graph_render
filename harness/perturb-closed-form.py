"""Negative control for oracle-closed-form: copies the fixtures, shifts one ring coordinate
by 0.01 (10000x the ring ceiling), re-seals the manifest, and leaves the result to the
unchanged oracle. The comparison, not the sealing, must be what turns red.

    docker run … ge-python-oracle python3 harness/perturb-closed-form.py src dst
"""
import hashlib
import json
import os
import shutil
import sys
import tempfile

if len(sys.argv) != 3:
    sys.exit("usage: perturb-closed-form.py <fixtures-dir> <perturbed-dir>")
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


path = os.path.join(dst, "closed-form.jsonl")
before = open(path, "rb").read()
lines = before.decode().splitlines()
case = json.loads(lines[1])
case["ring"]["x"][0] += 0.01
lines[1] = json.dumps(case, sort_keys=True, separators=(",", ":"))
replace(path, "\n".join(lines) + "\n")
after = open(path, "rb").read()
# A control that perturbs nothing passes vacuously, so the perturbation itself is checked
# here rather than left to the downstream comparison to notice.
if after == before:
    sys.exit(f"{path} was not changed by the perturbation")

manifest_path = os.path.join(dst, "closed-form-manifest.json")
manifest = json.load(open(manifest_path))
manifest["sha256"]["closed-form.jsonl"] = hashlib.sha256(after).hexdigest()
replace(manifest_path, json.dumps(manifest, sort_keys=True, separators=(",", ":")))

print(json.dumps({"perturbed": path, "delta_bytes": len(after) - len(before)}))