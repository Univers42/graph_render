# Job preamble — prepended by scripts/orch/oc-job.sh to every OpenCode job body

You are one worker on one job, alone in this worktree. AGENTS.md (the agent brief and the house
rules) binds you. The job body below is your contract: its paths, its done-when, its return block.

RULE 0 — fan out first. Before editing, dispatch in ONE message one `subagent` call per independent
slice (agent `explore` to read, `general` to edit a disjoint set of files), without the `background`
flag: calls in one message run concurrently. You merge, deduplicate and verify what they return.
Keep each subagent small: at most 4 files, and a reply of at most 60 lines made of conclusions with
`file:line`, never a whole file. Read a file you need verbatim yourself.
Caveat: three jobs (twopi and p12-t2 on 2026-09-30/10-01, merge-sim earlier) hung forever on an explore
asked to dump many whole files. The cap is a guess at the cause, not a measurement; a hang past 30 min
is still recovered by hand (kill, `opencode session delete`, requeue).

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

