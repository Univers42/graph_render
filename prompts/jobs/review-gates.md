# Job review-gates (agent build, review only, docs)

Why: the user asked for a review of all the code until it conforms (2026-10-01). A gate is the evidence every other job relies on. A gate that can pass when it should fail turns every green row into an unverified claim.

Scope: `crates/graph-cli/src/**`: the gates (`hashgate*`, `runner.rs`, `fingerprint.rs`, `evidence*`, `capabilities*`, `oracle_python*`, `oracle_fixtures*`, `forcecheck*`, `stress*`, `codegen.rs`, `command.rs`, `main.rs`), and `scripts/scigraphs-conformance.sh` with `harness/scigraphs-conformance*`.

Rules, checks and output shape: exactly as `prompts/jobs/review-core-post.md` says (read it first),
with `docs/reviews/review-gates.md` as the report and the only path you write. The question per gate is: what input makes it exit 0 when it should not (a skipped arm counted as a pass, a timeout read as success, an empty comparison set, a tolerance that admits any value, a fingerprint that misses a path the result depends on, a negative control that cannot turn red). Each such path is a BLOCKER with the input that shows it. Exit codes: 0 passed, 1 ran and failed, 2 could not run; any other mapping is a finding.
Review `origin/develop` as it is when you start; fan out one `explore` subagent per module in ONE
message, then merge, deduplicate and verify each finding yourself.

Done when: every module in scope is named in the report with its finding count (0 is a valid count);
every BLOCKER/MAJOR row has a command and its output or a reference `file:line`.
