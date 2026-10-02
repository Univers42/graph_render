#!/usr/bin/env bash
# oc-tabs.sh [-a] [-n] [session-id...] — OpenCode sessions as the tabs of one OpenCode window.
# Default: the sessions the background service is running (GET /api/session/active).
# -a: every top-level session of this directory's project, newest first (`opencode session list`).
# Adds them and any id given to the TUI's saved tab list for this directory, then opens OpenCode on
# the first one. -n only adds the tabs: a window already open here reloads the list without restart.
# Exit 0 = tabs written (and the TUI exited 0), 1 = no session to show, 2 = could not ask the service.
# The tab list is ~/.local/state/opencode/latest/tui/tabs.json, read on start and on every change
# (OpenCode 2.0.18 watches its tui/ directory). Tabs live under "cwd"/<directory>, or "global" when
# cli.json sets tabs.scope to global. Test seams: OC (the opencode binary), OC_TABS_FILE, OC_CLI_JSON.
# Caveat: the write is tmp+rename, not OpenCode's own file lock; an open window that saves its tabs
# in the same few milliseconds keeps its copy and drops ours. Run the script again to add them back.
# Caveat: "running" is the service's drain set, which skips a session idling between turns (waiting
# for a permission or a prompt); pass such a session's id as an argument, or use -a.
# Caveat: -a lists one project, the one this directory belongs to; sessions of other projects are
# left out (run it from that project), and so is anything past the newest 1000000.
set -uo pipefail
OC=${OC:-$HOME/.opencode/bin/opencode}
file=${OC_TABS_FILE:-${XDG_STATE_HOME:-$HOME/.local/state}/opencode/latest/tui/tabs.json}
cli=${OC_CLI_JSON:-${XDG_CONFIG_HOME:-$HOME/.config}/opencode/cli.json}
open=1
all=0
while getopts an flag; do
  case $flag in
    a) all=1 ;;
    n) open=0 ;;
    *) echo "usage: oc-tabs.sh [-a] [-n] [session-id...]" >&2; exit 2 ;;
  esac
done
shift $((OPTIND - 1))

# api <path> — one GET through `opencode api`, which carries the service's credentials itself
api() { timeout 30 "$OC" api GET "$1" 2>/dev/null; }

# tab <id> — the {sessionID,title} entry of the id's top-level session; a subagent shows as its parent
tab() {
  api "/api/session/$1" | jq -ce '.data | {sessionID: (.parentID // .id), title: (.title // "")}'
}

# found — the tabs to add besides the ids given, one JSON object per line
found() {
  local live ids id
  if ((all)); then
    timeout 30 "$OC" session list --format json -n 1000000 2>/dev/null |
      jq -c '.[] | {sessionID: .id, title: (.title // "")}'
    ids=("${PIPESTATUS[@]}")
    return $((ids[0] || ids[1]))
  fi
  live=$(api /api/session/active) || return 1
  mapfile -t ids < <(jq -r '.data | keys[]' <<<"$live" 2>/dev/null)
  for id in "${ids[@]}"; do tab "$id" || echo "oc-tabs: no session $id" >&2; done
}

list=$(found) || { echo "oc-tabs: the OpenCode service did not answer" >&2; exit 2; }
tabs=$( { for id in "$@"; do tab "$id" || echo "oc-tabs: no session $id" >&2; done; printf '%s\n' "$list"; } |
  jq -sc 'reduce .[] as $t ([]; if any(.[]; .sessionID == $t.sessionID) then . else . + [$t] end)')
[[ $tabs != '[]' ]] || { echo "oc-tabs: no session to show" >&2; exit 1; }

scope=$(jq -r '.tabs.scope // "cwd"' "$cli" 2>/dev/null) || scope=cwd
mkdir -p "$(dirname "$file")"
old=$(cat "$file" 2>/dev/null) || old='{"global":{"tabs":[],"unread":{}},"cwd":{}}'
if ! jq --arg scope "$scope" --arg dir "$(pwd -P)" --argjson add "$tabs" '
  (if $scope == "global" then ["global"] else ["cwd", $dir] end) as $p
  | setpath($p; (getpath($p) // {tabs: [], unread: {}})
    | .tabs += [$add[] as $t | select(all(.tabs[]; .sessionID != $t.sessionID)) | $t])
' <<<"$old" >"$file.oc-tabs" || ! mv "$file.oc-tabs" "$file"; then
  echo "oc-tabs: cannot write $file" >&2
  exit 2
fi
jq -r '.[] | "\(.sessionID)  \(.title)"' <<<"$tabs"
((open)) || exit 0
exec "$OC" -s "$(jq -r '.[0].sessionID' <<<"$tabs")"
