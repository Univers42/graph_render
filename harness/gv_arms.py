"""The three arms of `harness/oracle-graphviz.py`, one per entry point.

`record_main` is the plain positional call, `differential_main` is `--differential`, and
`merge_main` is the `--merge` fold over sharded differential results. They share the seed,
the closed cases and the result shape, and nothing else; `oracle-graphviz.py` keeps the flag
parser, the DOT/sizing choice and the metric, and dispatches here. The split is what lets
that file stay under the house's 300 lines while holding three arms.

Every arm that records a `sha256` gets it from `oracle_common.read_manifest`, which
recomputes it over the fixture bytes it read. `manifest["sha256"]` is compared, never
copied into a result: copying it is what let a truncated fixture pass `verdict()`, which
compares manifest against result and was therefore comparing a value with itself.

`check_shards_agree` is the merge's own refusal: every shard must have run the same tree
over the same fixtures, or the max of their worsts is a max over different questions. The
comparison is against the digest **computed here** over the fixture file on disk, so a
shard that measured a truncated fixture is caught even when the manifest was re-sealed to
match it.
"""

import hashlib
import json
import os
import sys
import tempfile

from gv_closed import gap
from gv_plain import (
    graphviz_version,
    parse_plain,
    read_json,
    read_lines,
    run_engine,
    write_dot,
    write_lines,
)
from gv_sized import sized_dot
from oracle_common import finite, read_manifest, require_seeds


def record_main(options, ours_of):
    """The plain positional call: record where the engine put every node."""
    engine = options.engine
    jsonl_path = os.path.join(options.fixtures_dir, options.fixtures)
    os.makedirs(options.out_dir, exist_ok=True)
    out_path = os.path.join(options.out_dir, f"graphviz-{engine}.jsonl")
    manifest_path = os.path.join(options.out_dir, f"graphviz-{engine}-manifest.json")

    count = 0
    with tempfile.TemporaryDirectory() as tmp:
        with open(out_path, "w") as out:
            for line in open(jsonl_path):
                rec = json.loads(line)
                dot_path = os.path.join(tmp, f"g{rec['seed']}.dot")
                if "box" in rec:
                    sized_dot(dot_path, rec)
                else:
                    write_dot(dot_path, rec["n"], rec["source"], rec["target"])
                bbox, nodes = parse_plain(run_engine(engine, dot_path, options.start), rec["n"])
                row = {
                    "seed": rec["seed"],
                    "engine": engine,
                    "bbox": {"width": bbox[0], "height": bbox[1]},
                    "nodes": nodes,
                }
                out.write(json.dumps(row) + "\n")
                count += 1

    with open(out_path, "rb") as handle:
        digest = hashlib.sha256(handle.read()).hexdigest()
    manifest = {
        "engine": engine,
        "start": options.start,
        "seeds": count,
        "sha256": {f"graphviz-{engine}.jsonl": digest},
        "graphviz": graphviz_version(),
    }
    with open(manifest_path, "w") as f:
        json.dump(manifest, f, indent=1)
    print(f"{engine}: {count} seeds -> {out_path}")
    return 0


def differential_main(options, ours_of, engine_arms, closed_cases):
    """`--differential`: compare the native arm in `<engine>.jsonl` with Graphviz's own.

    One shard writes `<engine>-result-shard<I>.json` into `out_dir` plus the points it
    measured; with a single shard it writes `<engine>-result.json` into `fixtures_dir`,
    which is what `graph-cli oracle-graphviz` reads. [`merge_main`] is the fold.
    """
    engine, shards, shard = options.engine, options.shards, options.shard
    os.makedirs(options.out_dir, exist_ok=True)
    manifest, digest = read_manifest(options.fixtures_dir, engine)
    fixtures = read_lines(os.path.join(options.fixtures_dir, f"{engine}.jsonl"))
    require_seeds(manifest, fixtures, engine)
    mine = [r for at, r in enumerate(fixtures) if at % shards == shard]
    worst, theirs = 0.0, []
    with tempfile.TemporaryDirectory() as tmp:
        for record in mine:
            points = engine_arms(engine, tmp, record, options.start)
            worst = max(worst, finite(gap(ours_of(record, engine), points), f"{engine} gap"))
            theirs.append({"seed": record["seed"], "n": record["n"], "points": points})
        # The closed cases cost one small graph each and are the same in every shard, so
        # only shard 0 pays for them and the others record no verdict.
        closed, skipped = closed_cases(engine, tmp, options.start) if not shard else ({}, [])
    suffix = "" if shards == 1 else f"-shard{shard}"
    write_lines(os.path.join(options.out_dir, f"graphviz-{engine}{suffix}.jsonl"), theirs)
    exact = all(row["exact"] for row in closed.values()) if closed else None
    result = {
        "fingerprint": manifest["fingerprint"],
        "sha256": digest,
        "oracle": f"Graphviz {graphviz_version()} {engine} -Tplain -Gstart={options.start}",
        "layouts": {engine: {"cases": len(theirs), "worst": worst}},
        "closed": closed,
    }
    if skipped:
        result["closed_skipped"] = skipped
    if exact is not None:
        result["closed_exact"] = exact
    where, name = (options.out_dir, f"{engine}-result{suffix}.json") if shards > 1 else (
        options.fixtures_dir,
        f"{engine}-result.json",
    )
    with open(os.path.join(where, name), "w") as out:
        json.dump(result, out, indent=1)
    print(
        f"{engine} shard {shard}/{shards}: {len(theirs)} seeds, worst {worst:.3e} points; "
        f"closed {len(closed)} of {len(closed) + len(skipped)} exact: {exact}"
    )
    return 0 if exact is not False else 1


def merge_main(fixtures_dir, engine, out_dir, shards):
    """`--merge`: fold the per-shard results into the one `<engine>-result.json`.

    The fold is `max` over the worsts and `sum` over the cases, both order-free. It
    **refuses** a sweep whose shards do not add up to the manifest's seed count, and a shard
    whose fixtures or tree are not the ones on disk: a merge that quietly dropped a shard
    would report a smaller sweep than it ran.
    """
    parts = [
        read_json(os.path.join(out_dir, f"{engine}-result-shard{at}.json"))
        for at in range(shards)
    ]
    manifest, digest = read_manifest(fixtures_dir, engine)
    check_shards_agree(parts, manifest, digest, engine)
    cases = sum(part["layouts"][engine]["cases"] for part in parts)
    if cases != manifest["seeds"]:
        sys.exit(f"{cases} cases over {shards} shards, want {manifest['seeds']} seeds")
    worst = max(part["layouts"][engine]["worst"] for part in parts)
    result = {
        "fingerprint": manifest["fingerprint"],
        "sha256": digest,
        "oracle": parts[0]["oracle"],
        "layouts": {engine: {"cases": cases, "worst": worst}},
        "closed": parts[0]["closed"],
        "closed_exact": parts[0].get("closed_exact"),
        "shards": shards,
    }
    if parts[0].get("closed_skipped"):
        result["closed_skipped"] = parts[0]["closed_skipped"]
    with open(os.path.join(fixtures_dir, f"{engine}-result.json"), "w") as out:
        json.dump(result, out, indent=1)
    print(
        f"{engine}: {cases} seeds over {shards} shards, worst {worst:.3e} points; "
        f"closed {len(result['closed'])} exact: {result['closed_exact']}"
    )
    return 0 if result["closed_exact"] is not False else 1


def check_shards_agree(parts, manifest, digest, engine):
    """Every shard must have run the same tree over the same fixtures."""
    for at, part in enumerate(parts):
        if part["fingerprint"] != manifest["fingerprint"]:
            sys.exit(f"shard {at} ran against another tree: re-emit and re-run")
        if part["sha256"] != digest:
            sys.exit(f"shard {at} ran against other fixtures than these")