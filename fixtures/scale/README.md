# `fixtures/scale/` — generators, not blobs

`n220.json` is committed. The other three sizes are **not**, and that is the point
(`prompts/phase-09-scale-bench.md` §1): a benchmark whose input cannot be reproduced is an
anecdote, and a 10⁶-node fixture is ~160 MB of JSON that reproduces itself exactly from
one command. The generator is the artefact; this directory holds the command, the
constants it uses, and the one sample small enough to read.

## The generator

```sh
# node count, seed and output path; measures nothing.
gr cargo run --release -p graph-cli -- bench --n 1000000 --seed 0 \
  --emit-scale-fixture fixtures/scale/n1m.json
```

The model is **not** a second implementation of the benchmark graph. At or below the
model's own 100 000-node cap the fixture *is* `graph_core::seeded_model(seed, n,
REFERENCE_DEGREE)` — the same preferential-attachment graph every arm of the campaign
lays out, byte for byte, which `crates/graph-cli/src/bench/tests.rs`'s
`a_scale_fixture_at_or_below_the_cap_is_the_synthetic_model` pins. Its constants live in
`crates/graph-core/src/synthetic.rs`, not here: seed `0x051042` (mulberry32), 8 databases,
5 groups, 12 emoji and 4 icon names, `Math.floor(rnd() * rnd() * i)` preferential
attachment plus 5% `note_link` extras, edge strength `0.4 + rnd() * 0.4`, weights against
reference degree 8.

Above the cap the fixture is **whole components of that same model, concatenated** —
100 000 nodes each, every node and edge id prefixed `c<component>/` so nothing collides.
`a_scale_fixture_past_the_cap_is_whole_prefixed_components` pins that a 250 000-node
fixture is 100 000 + 100 000 + 50 000 and not a truncated 250 000, which would silently
change the degree distribution at exactly the size whose distribution is the point.

**What that costs, stated rather than hidden:** independent components have no edges
between them, so `n1m.json` is *easier* than one preferential-attachment graph of 10⁶
nodes. Any ceiling it measures is a lower bound on difficulty, never an upper one. The
prefix also makes the string arena a few bytes per node larger than the capped model's,
so the arena column for a concatenated fixture reads slightly pessimistic — the direction
that never flatters the number.

## The format

The provisional ingest document `crates/graph-wasm/src/ingest.rs` reads: `version` 1,
then `nodes` and `edges`, every member named (a present `null` where a field may be
absent, never an omitted key), no member beyond the ten and nine the reader requires, and
`kind` by exact name. So one file can be laid out by all three arms: `graph-cli bench
--n`-style natively, and through the wasm ABI and the JS SDK as it stands.
`an_emitted_scale_fixture_is_the_provisional_ingest_shape` pins the shape by parsing the
emitted document back and comparing every record's member list.

Numbers are written in full round-trip precision and a non-finite one is refused at write
time rather than written as the invalid JSON token `NaN`.

## Sizes

| file | n | committed | why |
|---|---:|---|---|
| `n220.json` | 220 | yes (98 KB) | today's real graph size, and small enough to read |
| `n10k.json` | 10 000 | no | ~2.7 MB; regenerate with the command above |
| `n100k.json` | 100 000 | no | ~27 MB |
| `n1m.json` | 1 000 000 | no | ~270 MB, and ten concatenated components |
