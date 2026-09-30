# shellcheck shell=bash
# Sourced by the orch and studio scripts: GM_SCRATCH, the host-local scratch root holding the
# worktrees (wt/), the pinned references (refs/), logs, locks and browser screenshots (mcp-out/).
# /goinfre/$USER where the host has /goinfre (the 42 hosts' fast local disk), $HOME/goinfre elsewhere
# (host dlesieur42, 2026-09-30, has no /goinfre). A GM_SCRATCH already set in the environment wins.
# Nothing under it is versioned: a host change loses it, and scripts/orch/wt-new.sh plus
# scripts/orch/fetch-refs.sh rebuild it.
if [[ -z ${GM_SCRATCH-} ]]; then
  if [[ -d /goinfre ]]; then GM_SCRATCH=/goinfre/$USER; else GM_SCRATCH=$HOME/goinfre; fi
fi
export GM_SCRATCH
