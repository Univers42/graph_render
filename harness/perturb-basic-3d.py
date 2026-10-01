"""Negative control for oracle-basic-3d: copies the fixtures, perturbs ONE coordinate far
past its ceiling, re-seals the manifest, and leaves the result to the unchanged arm. The
COMPARISON, not the sealing, must be what turns red.

Same shape as harness/perturb-closed-form.py, and for the same reason it exists: a negative
control that perturbs nothing passes vacuously, so a run of this differential with nothing
wrong in it proves only that the harness can add up.

Which coordinate is perturbed, and why not `layout.basic3d.cube`'s interior: the interior is
NOT compared (it is drawn from this crate's own Mulberry32 against the reference's
module-level global RandomState, so the two disagree by design), so moving it would turn
nothing red and this control would pass for the wrong reason. The perturbation goes into the
**sphere's** `z` — a closed form with no stream, gated at 1e-6 — and by 0.01, which is 10 000
times the ceiling.

    docker run … ge-python-oracle python3 harness/perturb-basic-3d.py src dst
"""
import hashlib
import json
import os
import shutil
import sys

src, dst = sys.argv[1], sys.argv[2]
shutil.rmtree(dst, ignore_errors=True)
shutil.copytree(src, dst)
path = os.path.join(dst, "basic-3d.jsonl")
lines = open(path).read().splitlines()
case = json.loads(lines[1])
case["sphere"]["z"][0] += 0.01
lines[1] = json.dumps(case, sort_keys=True, separators=(",", ":"))
open(path, "w").write("\n".join(lines) + "\n")
manifest_path = os.path.join(dst, "basic-3d-manifest.json")
manifest = json.load(open(manifest_path))
manifest["sha256"]["basic-3d.jsonl"] = hashlib.sha256(open(path, "rb").read()).hexdigest()
json.dump(manifest, open(manifest_path, "w"))