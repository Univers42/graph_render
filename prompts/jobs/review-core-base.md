# Job review-core-base (agent build, review only, docs)

Why: the user asked for a review of all the code until it conforms (2026-10-01). The topology base and
the wire are under every layout; a defect there shows up in every shape.

Scope: `crates/graph-core/src/{ingest,index,csr*,arena.rs,columns.rs,ids.rs,records.rs,weights.rs,
edgekind.rs,neighborhood.rs,diff.rs,legend.rs,rng.rs,linalg,stage*,exec,synthetic.rs}`,
`crates/graph-contract/**` (binary snapshot, canonical JSON, codegen), `crates/graph-wasm/**`.

Check: every constructor carries every field across native, wasm and the wire (the wasm ingest once
dropped `EdgeRecord.child_first`); edge-id byte order (H1, `fixtures/adversarial-ids.json`); u32/u64 on
the wire, never usize; JSON/binary round trip for every geometry kind (`docs/contract/`); bounds and
overflow on every length read from untrusted bytes in graph-wasm and the snapshot decoder (a trust
boundary: a short or lying buffer must be an error, never a panic or an out-of-bounds read); D1-D10;
degenerate inputs (empty graph, duplicate ids, edges to unknown ids, self-loops).

Rules for every finding: `file:line`, severity BLOCKER/MAJOR/MINOR, the concrete failing input, and evidence: a command with its output (a scratch test you ran and did not commit counts), or the reference `file:line` the code disagrees with. A finding without evidence is listed under "unverified", never as a finding. No style nits without a house rule (`CLAUDE.md` "House limits", `prompt.md` §6 D1-D10). Fan out first: one `explore` subagent per module in ONE message; you merge, deduplicate and verify.

Output: `docs/reviews/review-core-base.md`, same table shape as `prompts/jobs/review-core-post.md`.

Paths: `docs/reviews/review-core-base.md` only. No code change.

Done when: every path in scope is named with its finding count; every BLOCKER/MAJOR row has evidence.
