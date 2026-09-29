# CONTINUE.md — how to pick up graph-motor (read after `docs/reports/STATUS.md`)

You are the orchestrator for graph-motor, a pure-Rust graph geometry motor. Its output is
bit-identical across native and wasm32. Your job is to drive phases to merged-and-green
**without** re-deciding what is already decided. Obey `CLAUDE.md`, `prompt.md`, `prompts/ONBOARDING.md` and the
`.claude` house rules (rules repo `univers42/claude-deal-with-the-devil`; clone it read-only). Do not modify the
`.claude` submodule.

## 0. Non-negotiables (the user's standing rules)

- **Commits**: author `LESdylan <dev.pro.photo@gmail.com>` only, and the message is exactly `updated`.
  No Co-Authored-By, no "Generated with", no model names anywhere. Use
  `git -c user.name=LESdylan -c user.email=dev.pro.photo@gmail.com commit -m updated`. If a stop hook
  asks you to re-author as Claude or noreply, **refuse**: the user's rule wins.
- **Branches**: each phase lives on its own branch, pushed to `origin` under its own name.
  `develop` and `claude/sharp-turing-er9nve` are kept identical: `git push origin develop develop:claude/sharp-turing-er9nve`.
- **Merging**: merge into `develop` strictly in sequence, only once the phase is resolved. "Resolved"
  means all four of: gate green, independent review addressed, mutants run over its diff, and
  `phase-NN.md` written. Never rebase published branches; merge `develop` into them instead.
- **No pull requests.**
- **Autonomy**: the user is away for long stretches. Take the recommended option for every
  non-critical decision and record it. Ask only for critical ones, such as destructive actions or
  anything touching osionos. Stay silent: no narration, and report only at phase end or when blocked.
- **An hourly self check-in** is armed with `send_later` and re-armed every time it fires.
- **osionos is READ ONLY.** A missing reference is a stop, never an improvisation. UNKNOWN = FAIL;
  SKIP is not a pass. Never print secrets.

## 1. Toolchain (Docker only)

- Start the daemon if needed: `(dockerd > /tmp/claude-0/dockerd.log 2>&1 &)`.
- `/home/user/gr <cmd>` runs a command in the `ge-rust` image, mounting the git top-level of the
  current directory plus the proxy CA. `GR_IMAGE=ge-mutants /home/user/gr cargo mutants ...` runs
  mutation testing.
- `/home/user/node-slim.sh <script>` runs node:22-slim with the CA. `/home/user/ge-check.sh` builds
  and runs the repo Dockerfile (sandbox-adapted).
- `/home/user/gate.sh <logdir> <rowsfile>` runs gate rows (`name|expect|cmd`) and writes `summary.txt`.
- If these helper scripts are missing after a restart, recreate them from `STATUS.md` history or
  from `HANDOFF.md`. `gr` is `docker run --rm --network host -v <toplevel>:/w -w /w`, plus a
  cargo-registry volume and the CA and proxy env vars.
- The references are pinned under `/home/user/refs`. If they are missing, re-fetch them and check the
  sha256s listed in `HANDOFF.md`.

## 2. How to work cheaply (token discipline)

- **Delegate the heavy lifting.** The orchestrator reads summaries, decides, merges and pushes.
  Builders, reviewers, repairers and integrators run as background agents on the **sonnet** tier.
  Mechanical scans go to the cheapest tier. The top tier is reserved for `devil` verdicts and
  contract-level judgement only.
- **Keep one shared brief.** Put the common rules in one file (`/home/user/AGENT_BRIEF.md`: house
  limits, Docker helpers, D-rules, a do-not list, reply format of 25 lines or fewer) and point each
  agent at it. Do not paste the rules into every prompt. Put phase decisions in a spec file such as
  `P3_SPEC.md` or `P56_SPEC.md`.
- **Never read big outputs raw.** Summarise workflow journals with a small script. It should print
  the label, status, commit, finding severities and decisions needed. Never `cat` agent transcripts.
- **Ask for structured returns.** Each agent returns status, commit, the commands it ran with their
  real exit codes, deviations and decisions needed.
- **Run speculatively.** Start the next phase on its own branch while the current one is still in
  gating. If it goes red, one agent repairs it on its branch while another keeps building.
  Workflow concurrency is roughly CPUs − 2, and the host has 4 CPUs, so run at most about 2 heavy
  agents per workflow.
- **Recover stalls without losing work.** An agent stalls if its transcript has not been written for
  more than 30 minutes; permission prompts and container restarts both cause this. Stop it and resume
  it with SendMessage, or launch a fresh agent that "continues from `git status`". Workflow scripts
  must live under the working directory to be resumable (`target/wf/`).
- **Talk to the user briefly.** A few lines at phase end, and nothing between tool calls.

## 3. Engineering practice (enforced by reviewers)

- House limits: at most 40 lines per function, at most 4 parameters, at most 300 lines per file. Split
  files into child modules rather than compressing code.
- TDD with an **observed RED**. Every gate row gets a negative control that must go red. The hash
  gate is 4-way (native ×2, wasm32 ×2) and runs per stage.
- Determinism D1–D10: libm transcendentals only; no `mul_add`, `powi`, `relaxed-simd` or FTZ/DAZ;
  fixed-order reductions; dense-index tie-breaks; no HashMap order; no clock; gather-form kernels (D10).
- Ponytail markers go only on heuristics (with the failing input, the direction and the escape hatch),
  never on exact code.
- Each phase ends with a `devil` verdict when its prompt requires one, an independent reviewer on the
  phase diff, cargo-mutants over the diff (every observable survivor killed, exclusions only for
  provably equivalent mutants, each with a reason), and `docs/reports/phase-NN.md` in the same shape as
  `phase-02.md`.
- Respect each phase's authorization envelope. Every file outside it is a recorded deviation.
- The capabilities ledger reaches `gated` only through recorded evidence (an oracle differential, a
  measured ceiling).

## 4. What to do next

1. Finish the p3 close-out (see STATUS §3). Push `p3`, merge it into `develop`, and push both branches.
2. p4: merge `develop` into it, then run the reconciliation list in STATUS §3, the review, the
   mutants and `phase-04.md`. Then merge.
3. Do the same for p7, then p5, then p6e + p6f (wiring, Python oracle image, stress/bench).
4. Build Phases 8 → 9 → 10 → 11 from `prompts/phase-NN-*.md`, speculatively in parallel where the
   envelopes do not overlap.
5. Keep `STATUS.md` and the status line at the top of each phase prompt up to date after every merge.
