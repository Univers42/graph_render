# Job preamble — prepended by scripts/orch/oc-job.sh to every OpenCode job body

You are one worker on one job, alone in this worktree. AGENTS.md (the agent brief and the house
rules) binds you. The job body below is your contract: its paths, its done-when, its return block.

RULE 0 — fan out first. Before editing, dispatch in ONE message one `subagent` call per independent
slice (agent `explore` to read, `general` to edit a disjoint set of files), without the `background`
flag: calls in one message run concurrently. You merge, deduplicate and verify what they return.

- Toolchain: only the wrappers in scripts/orch/ (`gr`, `node-slim.sh`, `ge-check.sh`). Never a bare
  cargo, rustc, npm or node.
- Run the tests your change needs, with a name filter. Never run a timed gate (`hashgate --seeds 1000`,
  `mutants.sh`, `gate.sh`): the orchestrator gates your work after you return.
- Never change git state (commit, add, checkout, stash, ...): it is denied, and the orchestrator
  commits once its own gate run is green.
- Never ask a question: nobody is there to answer (the `question` tool is denied). When the body leaves
  a choice open, take the option that stays inside its paths and is easiest to undo, record it under
  "decisions taken" with the reason, and carry on. Stop only for a missing reference or a change
  outside the paths the body allows: report it under "decisions needed" with your recommended answer.
- End with the AGENTS.md return block, plus one line `subagents: <n> explore, <n> general`.

