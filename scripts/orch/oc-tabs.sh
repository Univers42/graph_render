#!/usr/bin/env bash
# oc-tabs.sh [-a|-p|-d] [-n] [session-id...] — OpenCode sessions as the tabs of one OpenCode window.
# Default: the sessions the background service is running (GET /api/session/active).
# -a: every top-level session of this directory's project, newest first, sorted on each run from the
# service's own records: running -> tab; probe (finished with under 3000 output tokens) -> out; dead
# (succeeded, retried later in the same folder, or its job branch already in origin/develop, and its
# folder is gone) -> out; broken (any other session whose folder is gone) -> its job worktree is rebuilt by wt-new.sh, which checks the
# branch out again, then tab; any other (a finished job whose worktree is still there) -> tab.
# "Out" also removes the session from the saved tab list. No session is ever deleted.
# -p / -d: -a narrowed to the sessions in progress (not succeeded: running, waiting, failed or
# interrupted) or done (succeeded); the other kind is out too and its worktree is not rebuilt.
# Either one implies -a, and -pd is -a.
# Adds the tabs and any id given to the TUI's saved tab list for this directory, then opens OpenCode
# on the first one. -n only writes the list: a window already open here reloads it without restart.
# Exit 0 = tabs written (and the TUI exited 0), 1 = no session to show, 2 = could not ask the service.
# The tab list is ~/.local/state/opencode/latest/tui/tabs.json, read on start and on every change
# (OpenCode 2.0.18 watches its tui/ directory). Tabs live under "cwd"/<directory>, or "global" when
# cli.json sets tabs.scope to global. Test seams: OC (the opencode binary), OC_TABS_FILE, OC_CLI_JSON,
# OC_WT_NEW (the worktree builder), GM_SCRATCH (scratch.sh; job worktrees are $GM_SCRATCH/wt/<branch>).
# Caveat: the write is tmp+rename, not OpenCode's own file lock; an open window that saves its tabs
# in the same few milliseconds keeps its copy and drops ours. Run the script again to add them back.
# Caveat: "running" is the service's drain set, which skips a session idling between turns (waiting
# for a permission or a prompt); pass such a session's id as an argument, or use -a.
# Caveat: -a lists one project, the one this directory belongs to; sessions of other projects are
# left out (run it from that project), and so is anything past the newest 1000000.
# Caveat: the probe test is a size, measured on 2026-10-02: the largest probe wrote 2008 tokens and
# the smallest finished job 5433. A real job that stops under 3000 is left out (pass its id).
# Caveat: a broken session whose folder is not $GM_SCRATCH/wt/<name> is left out with a message; only
# job worktrees can be rebuilt.
# Caveat: "in origin/develop" is read from the last fetch, and also holds for a job whose worktree was
# removed before its first commit (its branch is still a develop commit); such a job counts as dead.
set -uo pipefail
here=$(dirname "$(readlink -f "$0")")
# shellcheck source=scripts/orch/scratch.sh
source "$here/scratch.sh"
OC=${OC:-$HOME/.opencode/bin/opencode}
WT_NEW=${OC_WT_NEW:-$here/wt-new.sh}
file=${OC_TABS_FILE:-${XDG_STATE_HOME:-$HOME/.local/state}/opencode/latest/tui/tabs.json}
cli=${OC_CLI_JSON:-${XDG_CONFIG_HOME:-$HOME/.config}/opencode/cli.json}
probe=3000
open=1
all=0
kinds=()
while getopts adnp flag; do
  case $flag in
    a) all=1 ;;
    d) all=1; kinds+=("done") ;;
    p) all=1; kinds+=(progress) ;;
    n) open=0 ;;
    *) echo "usage: oc-tabs.sh [-a|-p|-d] [-n] [session-id...]" >&2; exit 2 ;;
  esac
done
shift $((OPTIND - 1))
((${#kinds[@]})) || kinds=(progress "done")
show=$(jq -nc '$ARGS.positional' --args "${kinds[@]}")

# api <path> — one GET through `opencode api`, which carries the service's credentials itself
api() { timeout 30 "$OC" api GET "$1" 2>/dev/null; }

# tab <id> — the {sessionID,title} entry of the id's top-level session; a subagent shows as its parent
tab() {
  api "/api/session/$1" | jq -ce '.data | {sessionID: (.parentID // .id), title: (.title // "")}'
}

# running — a tab per session the service is running, one JSON object per line
running() {
  local live ids id
  live=$(api /api/session/active) || return 1
  mapfile -t ids < <(jq -r '.data | keys[]' <<<"$live" 2>/dev/null)
  for id in "${ids[@]}"; do tab "$id" || echo "oc-tabs: no session $id" >&2; done
}

# records — the full records of the project's top-level sessions, newest first, as one JSON array
records() {
  local pid
  pid=$(timeout 30 "$OC" session list --format json -n 1 2>/dev/null | jq -er '.[0].projectId // ""') || return 1
  [[ -n $pid ]] || { echo '[]'; return 0; }
  api "/api/session?parentID=null&project=$pid&limit=1000000" | jq -ce '.data'
}

# classify — the records on stdin, each with .state: live, probe, dead, broken or keep (see the header)
classify() {
  local recs live dirs landed
  recs=$(cat)
  live=$(api /api/session/active | jq -ce '.data | keys') || return 1
  landed=$(git for-each-ref --merged origin/develop --format='%(refname:short)' refs/heads 2>/dev/null |
    jq -Rsc --arg wt "$GM_SCRATCH/wt/" 'split("\n")[:-1] | map($wt + .)')
  dirs=$(jq -r '.[].location.directory // empty' <<<"$recs" | sort -u |
    while IFS= read -r d; do [[ -d $d ]] && printf '%s\n' "$d"; done | jq -Rsc 'split("\n")[:-1]')
  jq -c --argjson live "$live" --argjson dirs "$dirs" --argjson landed "$landed" --argjson probe "$probe" '
    . as $all | to_entries | map(.key as $i | .value | .location.directory as $d | .state =
      if IN(.id; $live[]) then "live"
      elif .outcome and (.tokens.output // 0) < $probe then "probe"
      elif IN($d; $dirs[]) then "keep"
      elif .outcome == "succeeded" or IN($d; $landed[]) or any($all[:$i][]; .location.directory == $d)
      then "dead"
      else "broken" end)' <<<"$recs"
}

# rebuild <dir> — the job worktree <dir> back on disk through wt-new.sh; 0 once the folder exists
rebuild() {
  local name=${1#"$GM_SCRATCH/wt/"}
  [[ -d $1 ]] && return 0
  if [[ $name == "$1" || $name == */* || -z $name ]]; then
    echo "oc-tabs: cannot rebuild $1: not a job worktree" >&2
    return 1
  fi
  echo "oc-tabs: rebuilding $1" >&2
  timeout 900 "$WT_NEW" "$name" >/dev/null && [[ -d $1 ]]
}

# everything — a tab per top-level session of the project, one JSON object per line; .drop marks the
# ones to take out of the tab list, .hidden the kind -p/-d leaves out. A broken session's worktree is
# rebuilt first, or it is dropped.
everything() {
  local recs dirs d fixed=()
  recs=$(records) && recs=$(classify <<<"$recs") || return 1
  recs=$(jq -c --argjson show "$show" 'map(.hidden =
    ((if .outcome == "succeeded" then "done" else "progress" end) | IN($show[]) | not))' <<<"$recs")
  mapfile -t dirs < <(jq -r '.[] | select(.state == "broken" and (.hidden | not)) | .location.directory' \
    <<<"$recs" | sort -u)
  for d in "${dirs[@]}"; do rebuild "$d" && fixed+=("$d"); done
  jq -r 'group_by(.state) | map("\(length) \(.[0].state)") | "oc-tabs: " + join(", ")' <<<"$recs" >&2
  jq -c '.[] | {sessionID: .id, title: (.title // ""), drop: (.hidden or .state == "probe" or .state == "dead"
    or (.state == "broken" and (.location.directory | IN($ARGS.positional[]) | not)))}' \
    --args "${fixed[@]}" <<<"$recs"
}

list=$(if ((all)); then everything; else running; fi) ||
  { echo "oc-tabs: the OpenCode service did not answer" >&2; exit 2; }
tabs=$( { for id in "$@"; do tab "$id" || echo "oc-tabs: no session $id" >&2; done; printf '%s\n' "$list"; } |
  jq -sc 'reduce .[] as $t ([]; if any(.[]; .sessionID == $t.sessionID) then . else . + [$t] end)')
[[ $tabs != '[]' ]] || { echo "oc-tabs: no session to show" >&2; exit 1; }
add=$(jq -c 'map(select(.drop | not) | del(.drop))' <<<"$tabs")
gone=$(jq -c 'map(select(.drop) | .sessionID)' <<<"$tabs")

scope=$(jq -r '.tabs.scope // "cwd"' "$cli" 2>/dev/null) || scope=cwd
mkdir -p "$(dirname "$file")"
old=$(cat "$file" 2>/dev/null) || old='{"global":{"tabs":[],"unread":{}},"cwd":{}}'
if ! jq --arg scope "$scope" --arg dir "$(pwd -P)" --argjson add "$add" --argjson gone "$gone" '
  (if $scope == "global" then ["global"] else ["cwd", $dir] end) as $p
  | setpath($p; (getpath($p) // {tabs: [], unread: {}})
    | .tabs |= map(select(.sessionID | IN($gone[]) | not))
    | .tabs += [$add[] as $t | select(all(.tabs[]; .sessionID != $t.sessionID)) | $t])
' <<<"$old" >"$file.oc-tabs" || ! mv "$file.oc-tabs" "$file"; then
  echo "oc-tabs: cannot write $file" >&2
  exit 2
fi
jq -r '.[] | "\(.sessionID)  \(.title)"' <<<"$add"
[[ $add != '[]' ]] || { echo "oc-tabs: no session to show" >&2; exit 1; }
((open)) || exit 0
exec "$OC" -s "$(jq -r '.[0].sessionID' <<<"$add")"
