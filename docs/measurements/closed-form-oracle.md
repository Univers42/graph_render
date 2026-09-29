# Closed-form layouts against networkx 3.6

Measured 2026-09-29 on the p12-t1 tree, 1000 gate seeds (`n = 2 + seed % 600`, one connected
random graph each).

```sh
scripts/orch/gr cargo run -q -p graph-cli -- emit-closed-form-fixtures --seeds 1000
docker run --rm --user 0:0 -v "$PWD:/w" -w /w ge-python-oracle python3 harness/oracle-closed-form.py target/closed-form-fixtures
scripts/orch/gr cargo run -q -p graph-cli -- oracle-closed-form
```

| layout | cases | worst max abs coordinate gap | ceiling |
|---|---|---|---|
| `layout.circular.ring` | 1000 | 2.654e-7 | 1e-6 |
| `layout.spiral` | 1000 | 2.980e-8 | 1e-7 |
| `layout.bipartite` | 1000 | 2.980e-8 | 1e-7 |

The ring's gap is networkx narrowing its angles to `f32`; the other two sit at the snapshot's
`f32` rounding. Each ceiling is the next power of ten above its worst case.

Ponytail: the bipartite arm takes the node sets from our own output, so it checks geometry given a
partition, not SciGraphs' partition rule, and compares each column as sorted values, not node by
node. Empty, single-node and disconnected graphs are not in the gate model.

Negative control: `oracle-closed-form --dir target/does-not-exist` exits 2.
