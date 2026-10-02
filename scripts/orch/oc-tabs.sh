#!/usr/bin/env bash
# oc-tabs.sh [-n] [session-id...] — every live OpenCode session as a tab of one OpenCode window.
# Asks the background service for its live sessions (GET /api/session/active), adds them and any
# id given to the TUI's saved tab list for this directory, then opens OpenCode on the first one.
# -n only adds the tabs: a window already open here reloads the file and shows them without restart.
# Exit 0 = tabs written (and the TUI exited 0), 1 = no session to show, 2 = could not ask the service.
# The tab list is ~/.local/state/opencode/latest/tui/tabs.json, read on start and on every change
# (OpenCode 2.0.18 watches its tui/ directory). Tabs live under "cwd"/<directory>, or "global" when
# cli.json sets tabs.scope to global. Test seams: OC (the opencode binary), OC_TABS_FILE, OC_CLI_JSON.
# Caveat: the write is tmp+rename, not OpenCode's own file lock; an open window that saves its tabs
# in the same few milliseconds keeps its copy and drops ours. Run the script again to add them back.
# Caveat: "live" is the service's drain set, which skips a session idling between turns (waiting
# for a permission or a prompt); pass such a session's id as an argument.
set -uo pipefail
OC=${OC:-$HOME/.opencode/bin/opencode}
file=${OC_TABS_FILE:-${XDG_STATE_HOME:-$HOME/.local/state}/opencode/latest/tui/tabs.json}
cli=${OC_CLI_JSON:-${XDG_CONFIG_HOME:-$HOME/.config}/opencode/cli.json}
open=1
[[ ${1-} == -n ]] && { open=0; shift; }

# api <path> — one GET through `opencode api`, which carries the service's credentials itself
api() { timeout 30 "$OC" api GET "$1" 2>/dev/null; }

# tab <id> — the {sessionID,title} entry of the id's top-level session; a subagent shows as its parent
tab() {
  api "/api/session/$1" | jq -ce '.data | {sessionID: (.parentID // .id), title: (.title // "")}'
}

live=$(api /api/session/active) || { echo "oc-tabs: the OpenCode service did not answer" >&2; exit 2; }
mapfile -t ids < <(jq -r '.data | keys[]' <<<"$live" 2>/dev/null)
tabs=$(for id in "$@" "${ids[@]}"; do tab "$id" || echo "oc-tabs: no session $id" >&2; done |
  jq -sc 'reduce .[] as $t ([]; if any(.[]; .sessionID == $t.sessionID) then . else . + [$t] end)')
[[ $tabs != '[]' ]] || { echo "oc-tabs: no live session" >&2; exit 1; }

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
