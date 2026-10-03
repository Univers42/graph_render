"""What every Python oracle arm in `harness/` owes its result file, in one place.

Three refusals, because a differential that cannot name the bytes it read is not a
differential:

- `read_manifest` recomputes the sha256 over the fixture bytes and refuses a manifest that
  does not describe them, and returns **the digest it computed**. Seven arms carried those
  five lines each and three more (the two Graphviz arms) copied `manifest["sha256"]` into
  their result instead of the digest of the file they had just read, so `graph-cli`'s
  `verdict()` compared a value with itself and a truncated fixture passed.
- `require_cases` refuses a layout that compared no case. `judge()` requires only
  `cases > 0` per layout, so an empty fixture set leaves every accumulator at its `0.0`
  initialiser and the result reads as a perfect match.
- `finite` refuses a metric that is not a number **before** it reaches a `worst`
  accumulator. `max` and `>` are both false for a NaN, so a NaN gap leaves `worst` at
  `0.0`, `json.dump` writes the token `NaN`, and the case reports as a match.

Ponytail: every refusal is `sys.exit(<str>)`, which prints the message on stderr and exits
1. These arms document no 0/1/2 exit table, and inventing one here would change the code the
gate rows read for a refusal — a refusal stays 1, the same as the eight arms that already
refuse this way.

This module imports nothing but the standard library, and writes no bytecode: `harness/`
is inside the gate's fingerprinted set (`crates/graph-cli/src/fingerprint.rs:21`), so a
`__pycache__` here would move the fingerprint for as long as it existed. The arms that
import it by name set `sys.dont_write_bytecode` first, exactly as `oracle-graphviz.py`
says for its child modules.
"""

import hashlib
import json
import math
import os
import sys

# The networkx the closed-form and fa2 differentials are measured against, recorded in
# `docs/measurements/p12-t3.md` and in the emit run's own docstring. An arm that recorded
# the version without checking it measured a different formula in another image and still
# exited 0, so the version is a refusal here and not a string in the report.
NETWORKX_PIN = "3.6"


def read_manifest(directory, name):
    """`(manifest, digest)` for `<name>` in `directory`, refusing a manifest that does not
    describe the bytes on disk.

    The digest returned is the one computed here over the file this call read, so a caller
    that records it is recording what it measured. `manifest["sha256"]` is compared, never
    returned: copying it into a result is what let a truncated fixture pass.
    """
    with open(os.path.join(directory, f"{name}-manifest.json")) as handle:
        manifest = json.load(handle)
    with open(os.path.join(directory, f"{name}.jsonl"), "rb") as handle:
        digest = hashlib.sha256(handle.read()).hexdigest()
    if digest != manifest["sha256"][f"{name}.jsonl"]:
        sys.exit(f"{name}.jsonl does not match its manifest")
    return manifest, digest


def read_json(path):
    """The JSON document at `path`."""
    with open(path) as handle:
        return json.load(handle)


def require_seeds(manifest, rows, name):
    """Refuse a fixture file that is not the one the manifest counted.

    `judge()` requires `cases > 0` and nothing above it, so a sweep over one line of a
    1000-seed fixture reports `cases: 1` and passes. The seed count is the floor.
    """
    if len(rows) != manifest["seeds"]:
        sys.exit(
            f"{name}.jsonl holds {len(rows)} cases, want the manifest's "
            f"{manifest['seeds']} seeds"
        )


def require_cases(layouts, keys, name):
    """Refuse a layout that compared no case: a differential over nothing proves nothing."""
    for key in keys:
        if layouts[key]["cases"] == 0:
            sys.exit(f"{name}.jsonl compared no {key} case")


def finite(metric, what):
    """`metric` as a float, or a refusal naming it.

    The check `max` and `>` cannot make: both are false for a NaN, so the accumulator would
    keep its `0.0` and report the case as a perfect match.
    """
    if not math.isfinite(metric):
        sys.exit(f"non-finite {what}: {metric!r}")
    return float(metric)


def networkx_version(nx):
    """`nx.__version__`, refusing one that is not `NETWORKX_PIN`."""
    if nx.__version__ != NETWORKX_PIN:
        sys.exit(f"networkx {nx.__version__}, want {NETWORKX_PIN}: re-measure the ceiling")
    return nx.__version__