"""Negative control for oracle-hierarchical-3d: copies the fixtures, perturbs ONE coordinate
far past its ceiling, re-seals the manifest, and leaves the result to the unchanged arm. The
COMPARISON, not the sealing, must be what turns red.

Same shape as harness/perturb-basic-3d.py and harness/perturb-closed-form.py: a negative
control that perturbs nothing passes vacuously.

Which coordinate is perturbed, and why `z`: the layout puts the BFS LEVEL on z and the disk
in x/y (hierarchical.py:140, :144 — the function's own docstring says the opposite and is
stale), so moving `z` moves a whole level at once and is the coarsest perturbation available.
It is moved by 0.01 against a 1e-6 ceiling, 10 000 times over.

    docker run … ge-python-oracle python3 harness/perturb-hierarchical-3d.py src dst
"""
import hashlib
import json
import os
import shutil
import sys

src, dst = sys.argv[1], sys.argv[2]
shutil.rmtree(dst, ignore_errors=True)
shutil.copytree(src, dst)
path = os.path.join(dst, "hierarchical-3d.jsonl")
lines = open(path).read().splitlines()
case = json.loads(lines[1])
case["hierarchical-3d"]["z"][0] += 0.01
lines[1] = json.dumps(case, sort_keys=True, separators=(",", ":"))
open(path, "w").write("\n".join(lines) + "\n")
manifest_path = os.path.join(dst, "hierarchical-3d-manifest.json")
manifest = json.load(open(manifest_path))
manifest["sha256"]["hierarchical-3d.jsonl"] = hashlib.sha256(
    open(path, "rb").read()
).hexdigest()
json.dump(manifest, open(manifest_path, "w"))