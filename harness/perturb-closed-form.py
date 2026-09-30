"""Negative control for oracle-closed-form: copies the fixtures, shifts one ring coordinate
by 0.01 (10000x the ring ceiling), re-seals the manifest, and leaves the result to the
unchanged oracle. The comparison, not the sealing, must be what turns red."""
import hashlib, json, os, shutil, sys

src, dst = sys.argv[1], sys.argv[2]
shutil.rmtree(dst, ignore_errors=True)
shutil.copytree(src, dst)
path = os.path.join(dst, "closed-form.jsonl")
lines = open(path).read().splitlines()
case = json.loads(lines[1])
case["ring"]["x"][0] += 0.01
lines[1] = json.dumps(case, sort_keys=True, separators=(",", ":"))
open(path, "w").write("\n".join(lines) + "\n")
manifest_path = os.path.join(dst, "closed-form-manifest.json")
manifest = json.load(open(manifest_path))
manifest["sha256"]["closed-form.jsonl"] = hashlib.sha256(open(path, "rb").read()).hexdigest()
json.dump(manifest, open(manifest_path, "w"))
