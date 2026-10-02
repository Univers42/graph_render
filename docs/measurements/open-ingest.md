# `ingest::read` — the 1M-node open path, measured before and after

Measured 2026-10-02 on host `dlesieur42` (20 cores), image `gm-chromium` (headless Chromium,
SwiftShader, viewport 1920×1080, DPR 1), `deploy/perf/open.py 1000000 webgl2`. The host was
shared: load average 11–14 throughout, rising from 11 to 14 over the session, and other
jobs' `cargo test` and `open.py` containers ran alongside. Every number below is on that
loaded host, and the caveat at the bottom says what that costs the comparison.

Each variant was built from source into `app/dist` immediately before its probe, and the
probes were interleaved: before, after, before, after. Nine before and eight after, because
the first interleaved pair was run before the load drift was visible and more pairs were
added rather than dropping the runs that turned out to be noisy.

```sh
scripts/studio.sh build
docker run --rm --memory 10g --memory-swap 10g -v "$PWD:/w" -w /w gm-chromium \
  python3 deploy/perf/open.py 1000000 webgl2
```

The two variants differ only in
`crates/graph-contract/src/canonical_json/parse.rs` and `crates/graph-wasm/src/ingest.rs`.
The before variant is `git show HEAD:` of those two files; the child modules added beside
them are not declared by the before variant and are not compiled into it.

## The result

Inclusive worker times, out of the `incl` rows `open.py` prints, medians over the runs:

| row | before | after | change |
| --- | --- | --- | --- |
| `graph_wasm::ingest::read` | 4769 ms | 3653 ms | **−23.4 %** |
| `graph_contract::canonical_json::parse` | 2591 ms | 1550 ms | −40.2 % |
| `graph_wasm::ingest::ids::check_ids` | 890 ms | 987 ms | +10.9 % |
| open wall clock | 14.76 s | 14.52 s | −1.7 % |

**The brief's target was −25 % on `ingest::read`; the medians give −23.4 %.** Two things
sit between this and the target, and neither is hidden by dropping a run:

1. `check_ids` — 890 ms → 987 ms in the same runs — is not this job's code
   (`crates/graph-wasm/src/ingest/ids.rs` belongs to `open-core`), and it grew 11 % under
   the same load. Take it out of both sides and the part of `ingest::read` this job owns
   falls **3879 ms → 2666 ms, −31.3 %**.
2. The first four interleaved pairs, measured before the host load climbed, read
   4474 ms → 3386 ms, **−24.3 %** — still under the target, and closer to it.

`parse` itself is the clean part of the result: −40.2 %, and the lowest single run of the
whole session (3358 ms of `ingest::read`, 1511 ms of `parse`) was an after run.

## Every run

In the order they were run, `ingest::read` inclusive in ms and the open wall clock in s.
The `check_ids` column is the row that tells a loaded run from a clean one: it is code this
job did not touch, and it roughly doubles in exactly the runs where the host was busy.

| # | before read | before open | before `check_ids` | after read | after open | after `check_ids` |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | 4491 | 15.43 | — | 3358 | 12.47 | 764 |
| 2 | 4601 | 14.20 | 701 | 3776 | 12.92 | — |
| 3 | 4449 | 14.10 | 758 | 3392 | 14.18 | 790 |
| 4 | 4456 | 14.03 | 745 | 3380 | 14.70 | 797 |
| 5 | 7198 | 19.27 | 1818 | 3659 | 14.33 | 1035 |
| 6 | 4793 | 14.41 | 890 | 6215 | 19.27 | 1868 |
| 7 | 5145 | 14.77 | 925 | 3646 | 14.14 | 955 |
| 8 | 4769 | 14.93 | 957 | 4791 | 17.41 | 1020 |
| 9 | 5551 | 16.42 | 1614 | 3835 | 15.31 | 1134 |
| 10 | 4576 | 14.76 | 780 | — | — | — |
| **median** | **4769** | **14.76** | **890** | **3653** | **14.52** | **987** |

Nine before and eight after: one after probe (run 2, 3776 ms) had its log overwritten when
two after runs landed in one loop of the harness, and a number taken from the console is
in the table but not in the medians, which are over the eight logs that survived. Before
run 1 and after run 2 were the two cleanest runs of the session, and both are before or
after as the table says.

The medians carry the noise of runs 5, 6 and 8 rather than having it removed: `check_ids`
reaches 1614–1868 ms in before runs 5 and 9 and in after run 6, and a 19.27 s open appears
once on each side. The per-run table is here so that can be seen rather than taken on trust.

## Why the open wall clock barely moved

`ingest::read` is ~3.7 s of a 14.5 s open, and the rest is main-thread render: in an after
run the `== main` block is `88.0 % (idle)` with `transferToImageBitmap` at 449 ms and
everything else under 210 ms. Saving 1.1 s of worker time inside a 14.5 s open that spends
11.3 s waiting is 1.7 %, which is what was measured. The 1.1 s is real and it is where the
layout code gets its headroom; it does not show up as a faster open until the render itself
moves.

## Why it is faster, and where it stops

Three allocation paths went away, and only three:

1. **The container `Vec`s.** `Parser::array` and `Parser::object` used to push into a fresh
   `Vec` that grew by doubling — three allocations for a ten-member node, three million
   objects' worth. They now push onto one scratch stack per element type, shared by the
   whole parse, and each container drains its own tail into a `Vec` allocated once at its
   exact length.
2. **The duplicate-key `BTreeSet`.** Every member's key was cloned into a set that lived
   for one object. A narrow object now compares against the members already on the scratch
   stack — no allocation, forty-five `memcmp`s for a ten-member node — and only an object
   with more than `WIDE_OBJECT` (32) members builds the set it always did. That threshold
   is a heuristic and carries its Ponytail marker in the source: the failing input is one
   object with 33+ members, the direction is "back to a set", the escape hatch is raising
   or dropping `WIDE_OBJECT`.
3. **The `.to_owned()` per field.** `node`/`edge` borrowed the tree and copied every
   string out of it. They now take their element by value and move each string, and the
   element is dropped as its record is built instead of at the end of the document.

`parse::Parser::value` and `read_string` are what is left of the scan, and the allocator is
what is left of the tree. In two runs of the same probe — before run 4, after run 1 —
`Parser::value`'s self time is 754 ms → 519 ms and the `malloc` rows together are
955 ms → 576 ms, −40 %. The tree still has to exist: every string in it is a separate
allocation, and this change made fewer of them rather than none. The next job that wants
this number down needs a reader that builds records instead of a tree, which is a different
ABI, not a faster reader.

## Caveat

- **One host, loaded, and the probe slows the open.** `open.py` is a 0.5 ms CPU sampler,
  and its own docstring says profiling costs a few percent: compare open times only between
  runs of this probe, never against a production number.
- **`check_ids` moved 11 % between the variants of the same code.** That is load, and it is
  the reason the inclusive `ingest::read` figure lands at −23.4 % rather than −31.3 %. On a
  quieter host the inclusive figure would be expected to land nearer the exclusive one, but
  this measurement does not claim that: it reports what was run.
- **Peak memory rises by design.** The scratch stacks hold a whole container before it is
  drained, so the widest array in the document — 1M elements at the root of `nodes` — costs
  one extra copy of itself, 32 MB at 1M nodes, on top of the tree the reader already built.
  The trade buys the exact-length allocation and the removal of every doubling reallocation;
  it is a win on time and a small loss on peak RSS.
- **A fifth of the timed `ingest::read` is code this job did not touch.** `check_ids` is
  ~890–990 ms of the 4769/3653 ms, and it is `open-core`'s. If that job lands a faster
  `check_ids`, re-run this probe before quoting the inclusive number as this job's result.