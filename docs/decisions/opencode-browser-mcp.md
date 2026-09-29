# ADR — A browser MCP server for the OpenCode front-end agents

Status: **accepted with conditions** (2026-09-29). The devil verdict and its conditions are at the end of this file.

## Context

The studio's UI defects (a blank strip on the right edge, a HUD past the viewport at 375 px) are
claims about rendered pixels. An agent that only reads CSS cannot prove or refute them. The
OpenCode `ux` and `ux-probe` agents need a real browser they can drive, and every other job must
not pay for it.

The prebuilt vendor browser images are ruled out by the Docker-only rule. The Claude-side browser
MCP on this host is held by another session.

## Decision

- **Image.** `deploy/mcp-browser.Dockerfile` builds `gm-mcp-browser` from `debian:trixie-slim` with
  Debian's `chromium` and `nodejs`. `@playwright/mcp` is pinned to 0.0.83 by
  `deploy/mcp-browser/package-lock.json` and installed with `npm ci --ignore-scripts`; no browser
  download.
- **Wiring.** `opencode.json` `mcp.pw` starts it as a stdio server per OpenCode instance:
  `docker run --pull never -i --rm --network host --user 0:0 -v /goinfre/dlesieur/mcp-out:/out
  gm-mcp-browser --allowed-origins <the three studio dev ports on 127.0.0.1 and localhost>`.
  `--pull never` because `/goinfre` (the docker root here) is wiped on a host change, and the
  image name is not ours on docker.io. Rebuild it after a host change (`prompts/RESUME.md`).
- **Server name `pw`, not `browser`.** OpenCode 2.0.18 runs tools in code mode: the model calls
  one `execute` tool whose JavaScript reaches `tools.<server>.<tool>(args)`. OpenCode already owns
  a `tools.browser.*` namespace (its desktop-app browser, disconnected here, and not governed by
  `browser_*` rules). A server named `browser` merged into it; the smoke run's agent called the
  desktop tools and got `[browser.disconnected]`.
- **Tool ids.** OpenCode names an MCP tool `<server>_<tool>`, and every Playwright tool already
  starts with `browser_`, so the permission ids are `pw_browser_navigate`, and so on. The
  last matching rule wins (patterns are anchored, `*` is `.*`), so an agent's rules must come
  after the global deny they override.
- **Deny by default.** The top-level permission `"pw_*": "deny"` hides the tools from every
  agent. Only `.opencode/agents/ux.md` and `ux-probe.md` re-allow them.
- **Unsafe tools stay off.** Both agents deny `pw_browser_run_code_unsafe` and
  `pw_browser_file_upload`, after their `pw_*` allow. `ux-probe` also denies `bash`,
  since it edits nothing. The server cannot drop `browser_run_code_unsafe` itself: it is a `core`
  capability, which `--caps` always keeps, so the client-side deny is the only lever short of a
  filtering stdio proxy.
- **Screenshots take an absolute `/out/<label>/<name>.png`.** The server resolves a relative
  filename against the client's root, which OpenCode sends as the worktree path. That path does
  not exist in the container, so the file died with it. `/out` is the bind mount
  (`/goinfre/dlesieur/mcp-out` on the host). Both agent files say so.
- **Skills come from `skills.paths`.** Every worktree's `.claude` submodule is uninitialised, so
  OpenCode found no skills there, and the bunnies never had the house skills. `opencode.json`
  points `skills.paths` at the main checkout's `.claude/skills`, which is read, never written.

## Accounting (minimalism ladder, rung 6: a new dependency)

- **Why not a lower rung.** The repo already has `gm-chromium` + `deploy/perf/cdp.py` (studio
  branch) for scripted probes. It is a script runner, not something an agent can drive step by
  step. No dependency in any manifest speaks MCP.
- **What it pulls in.** `@playwright/mcp` 0.0.83 → `playwright` and `playwright-core`
  1.64.0-alpha (4 lockfile entries). They live only in the image.
- **Who maintains it.** The Playwright project (Microsoft).
- **Scope.** A test instrument. It is not a dependency of any package, crate or shipped artifact,
  and no gate row depends on it.
- **Removal path.** Delete the `mcp` key and the `pw_*` permission lines in `opencode.json`,
  the two agent files, `deploy/mcp-browser*`, and the image.

## Security

- **Host network.** Under rootless docker, `--network host` is the rootlesskit network namespace,
  run with slirp4netns `--disable-host-loopback`. The container reaches the docker-published ports
  (the studio on 5174, and other projects' containers such as 4300 and 27019). It does not reach
  host-loopback services such as the OpenCode API. Every port it reaches is already open to the
  host user's shell, so the container adds no reach.
  - The container sees no secret: its only mount is `/out`, and its env is `HOSTNAME HOME PATH PWD`.
  - `--allowed-origins` narrows what the page may request. The server's own help says this is
    *not* a security boundary and does not cover redirects. It is defence in depth only.
  - `browser_run_code_unsafe` is full Node RCE in the container: the devil's probe reached
    `process` through `page.constructor.constructor`, escaping `node:vm`. That is why it is denied.
  - The boundary is the host user account. Both agents that can drive the browser (`ux`) also hold
    a shell, and page content is untrusted input.
- **Rootless docker.** Container root is the invoking host user, so `--user 0:0` grants nothing
  beyond that user. A non-root image user maps to an unowned subuid and cannot write `/out`. On a
  rootful daemon the flag must be dropped (the Dockerfile header says so).
- **Sandbox.** Chromium runs with `--no-sandbox`, as it must inside an unprivileged container. The
  container is the sandbox. The profile is in memory (`--isolated`) and dies with the container.

## Token cost

`tools/list` returns 25 tools and 21,332 bytes of schema (measured 2026-09-29, 0.0.83). In code
mode those schemas are not sent with each request; the model reads them on demand through
`search`. First-step input tokens, same host, 2026-09-29:

| Run | `ux` (pw allowed) | `builder` (pw denied) |
|---|---:|---:|
| smoke 1 | 6,470 | 6,090 |
| smoke 3 | 7,762 | 7,315 |

The gap is the agent prompt and the task text, not the schemas. The top-level deny therefore
saves no per-request tokens. It is kept for what it does do: a denied tool is absent from the
catalog and cannot be called.

## Measured

A stdio probe on 2026-09-29, with the flags above:
- `browser_navigate` to `http://127.0.0.1:5174/` succeeded (title "graph-motor studio");
- `https://example.com/` failed with `net::ERR_BLOCKED_BY_CLIENT`;
- `browser_take_screenshot` wrote `/out/probe-375.png` as the host user.

Ponytail: `playwright-core` is built for its own Chromium revision and drives Debian's. A protocol
skew shows up as a failed tool call in the smoke run, not at build time.

Ponytail: `timeout: 20000` applies to every MCP call, catalog and execution alike. A cold start
measured 1.15 s, but a `browser_wait_for` longer than 20 s fails (visibly, not silently).

Ponytail: `npm audit` and a Chromium image scan have not been run; known CVEs are UNKNOWN. The
base image is a tag, not a digest, and the Debian packages are unpinned.

Ponytail: every OpenCode instance starts one container, Rust jobs included, even though they are
denied the tools (63 MiB and 8 pids idle, measured).

The OpenCode smoke run (2026-09-29, `ux` against a `builder` control, journals
`target/wf/uxsmoke3.jsonl` and `ctlsmoke3.jsonl` in the scratch worktrees):

| Check | Result |
|---|---|
| `skill` loads `frontend` | PASS |
| two `explore` subagents in one message | PASS |
| `tools.pw.browser_resize`, `_navigate`, `_snapshot`, `_take_screenshot` | PASS, title "graph-motor studio" |
| the screenshot at `/goinfre/dlesieur/mcp-out/uxsmoke/uxsmoke-375.png` | PASS, 31,245 bytes |
| `tools.pw.browser_run_code_unsafe` called by name from `ux` | refused: `Unknown tool 'pw.browser_run_code_unsafe'` |
| `builder` sees any `tools.pw.*` | none; `tools.pw.browser_navigate` is `Unknown tool` |

The static check agrees: replaying the anchored last-match rule over `opencode debug agents`
gives `deny` for `pw_browser_run_code_unsafe` and `pw_browser_file_upload` in all 13 agents, and
`allow` for `pw_browser_navigate` only in `ux` and `ux-probe`.

## Not verified yet

- Whether the origin filter follows a redirect from an allowed origin to a blocked one.
- `pw_browser_file_upload` was refused statically only; no live call was made.

## Verdict

Devil, 2026-09-29: **PROCEED-WITH-CONDITIONS**. Scores: blast radius 3, reversibility 2, cost on
failure 3, confidence 4. The worst axis was confidence: the original deny ids matched nothing.

| Condition | Done |
|---|---|
| C1. Deny ids `browser_browser_run_code_unsafe` and `browser_browser_file_upload`, after the allow | yes, renamed to `pw_browser_*` with the server (a deviation in ids, same intent); refused live |
| C2. `bash: deny` on `ux-probe` | yes; its last shell rule is `shell * deny` |
| C3. This ADR: real ids, rule order, rootless netns, no "tool set" boundary, Ponytails | yes |
| C4. `--pull never`, and the image in the host-change recipe | yes |
| C5. Stage only this change's paths | yes |

Rejected as conditions, and kept as optional hardening: dropping `--network host`, and
`--cap-drop ALL --read-only` (untested with Chromium; under rootless docker container root adds
nothing over the agent's own shell).
