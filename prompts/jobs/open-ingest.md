# Job open-ingest (agent build; the 1M-node open path, graph-contract parse and graph-wasm ingest)

Why: opening 1 000 000 nodes in the studio takes 14.3 s, and 5.1 s of it is `graph_wasm::ingest::read`
in the motor worker. Most of that is allocation, not parsing: the canonical JSON parser builds a
`Value` tree of about 3M objects whose every member key is its own `String`, each object's
`Vec<(String, Value)>` grows by doubling (3 allocations for a 10-member node), `node()`/`edge()` then
copy every string again with `.to_owned()`, and the whole tree is dropped at the end.

Facts (profile `deploy/perf/open.py 1000000 webgl2` on the perf-p5 tree, SwiftShader, load ~10;
inclusive worker times out of 14.77 s sampled; re-run the probe for your own baseline):
- `ingest::read` 5089 ms; inside it `canonical_json::parse` 2859 ms (`Parser::string` 772 ms),
  `check_ids` 934 ms (not yours: job open-core), `ingest::edge` 339 ms.
- Allocator rows anywhere in the worker: `dlmalloc::malloc` 741 ms, `__rdl_realloc` 648 ms,
  `RawVecInner::finish_grow` 574 ms, `do_reserve_and_handle` 378 ms, `free` 422 ms,
  `drop_in_place<canonical_json::Value>` 464 ms.
- `crates/graph-contract/src/canonical_json/parse.rs`: `Value::Object(Vec<(String, Value)>)`,
  `Value::Array(Vec<Value>)`; `Parser::value` (line ~68) pushes members into a fresh `Vec`;
  `Parser::string` (line ~174) already copies runs without escapes with one `push_str`.
- `crates/graph-wasm/src/ingest.rs` `read` (line ~51): parse the whole text, check the root
  (`version`, `nodes`, `edges`, `require_only`), then every node in order, then every edge, then
  `check_ids`. `node()` and `edge()` borrow the tree and `.to_owned()` each string.

Do:
1. RED first: a differential test in `crates/graph-wasm/src/ingest/tests.rs` (or a sibling module)
   that keeps today's `read` as a `#[cfg(test)]` reference and asserts `new read(bytes) == old
   read(bytes)` (the whole `Result`: records, or the same `IngestError` with the same text) over
   every document under `fixtures/` that ingest accepts, plus at least 2000 mutations of them
   (byte flips, truncations, a member deleted, renamed or duplicated, a type swapped, an id
   duplicated, an endpoint renamed), generated with a fixed seed and no new dependency.
2. `parse.rs`: build each object's members and each array's elements on one scratch stack shared
   by the whole parse, then move them into a `Vec` allocated once at its exact length. `parse`'s
   output must be the identical `Value` for every input, and every error the same `JsonError`.
   Its existing tests stay unchanged and green.
3. `ingest.rs`: consume the tree by value. Move each string out of its `Value` instead of copying
   it, and drop each node or edge element as soon as its record is built. Keep the validation order
   exactly as it is now: the whole text is parsed before any shape check, the root is checked before
   any node, and every node before any edge. The differential test is the judge.
4. Measure with `scripts/studio.sh build` and then `deploy/perf/open.py 1000000 webgl2` (3 runs of
   the build before and 3 after, interleaved). Write `docs/measurements/open-ingest.md` with both
   `ingest::read` inclusive times, both open times, and the command lines.

Out of bounds: `crates/graph-core`, `crates/graph-wasm/src/ingest/ids.rs` and `at.rs` (job open-core),
`packages/` (job open-synth), and any public type or signature in graph-contract. `Value` keeps its
shape. Adding a dependency is a stop.

Paths: `crates/graph-contract/src/canonical_json/parse.rs` (split it into a child module if it passes
300 lines), `crates/graph-wasm/src/ingest.rs`, `crates/graph-wasm/src/ingest/tests.rs` (or a new
`ingest/differential.rs`), `docs/measurements/open-ingest.md`.

Done when: the differential test passes, and `parse`'s and ingest's existing tests are unchanged and
pass. quick.rows is green, including `hashgate-8`, its negative control and `roundtrip` through
`cargo test`. `ingest::read` inclusive time falls by at least 25% at 1M, with the measurement file
committed. If it falls by less, report the numbers anyway: they are the result, not a failure to
hide.
