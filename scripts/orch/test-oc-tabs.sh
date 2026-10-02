#!/usr/bin/env bash
# test-oc-tabs.sh — behaviour tests for scripts/orch/oc-tabs.sh. Plain bash, no bats, no service:
# OC points at a stub opencode that replays canned JSON for `api GET` and records any other call.
# Exits 0 when every case passes, 1 otherwise. Never touches the real tabs.json.
set -uo pipefail
here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
TABS=$here/oc-tabs.sh
tmp=$(mktemp -d "${TMPDIR:-/tmp}/octabs.XXXXXX")
trap 'rm -rf "$tmp"' EXIT
mkdir -p "$tmp/sess" "$tmp/here"
export STUB_DIR=$tmp OC=$tmp/opencode OC_TABS_FILE=$tmp/tabs.json OC_CLI_JSON=$tmp/cli.json
dir=$(cd "$tmp/here" && pwd -P)
pass=0
fail=0

ok() { pass=$((pass + 1)); printf 'ok    %s\n' "$1"; }
no() { fail=$((fail + 1)); printf 'FAIL  %s: %s\n' "$1" "$2"; }
# check <name> <jq-expr that must be true on tabs.json>
check() { if jq -e "$2" "$OC_TABS_FILE" >/dev/null 2>&1; then ok "$1"; else no "$1" "$(cat "$OC_TABS_FILE")"; fi; }
rc_is() { if [[ $2 == "$3" ]]; then ok "$1"; else no "$1" "rc=$2 want=$3"; fi; }
# unchanged <name> <want> — tabs.json still holds exactly <want>
unchanged() { if [[ $(cat "$OC_TABS_FILE") == "$2" ]]; then ok "$1"; else no "$1" "$(cat "$OC_TABS_FILE")"; fi; }
run() { (cd "$tmp/here" && "$TABS" "$@" >/dev/null 2>&1); }
# sess <id> <title> [parent]
sess() { jq -cn --arg i "$1" --arg t "$2" --arg p "${3-}" \
  '{data: ({id: $i, title: $t} + (if $p == "" then {} else {parentID: $p} end))}' >"$tmp/sess/$1.json"; }
active() { jq -cn '{data: (reduce $ARGS.positional[] as $i ({}; .[$i] = {type: "running"}))}' --args "$@" >"$tmp/active.json"; }

cat >"$OC" <<'STUB'
#!/usr/bin/env bash
if [[ ${1-} == api ]]; then
  case $3 in
    /api/session/active) [[ -f $STUB_DIR/down ]] && exit 1; cat "$STUB_DIR/active.json" ;;
    /api/session/*) cat "$STUB_DIR/sess/${3##*/}.json" 2>/dev/null || exit 1 ;;
  esac
  exit
fi
printf '%s\n' "$*" >"$STUB_DIR/launch"
STUB
chmod +x "$OC"

sess ses_a job-a
sess ses_b job-b
sess ses_kid explore ses_b
jq -n --arg d "$dir" '{global: {tabs: [], unread: {}}, cwd: {"/elsewhere": {tabs: [{sessionID: "ses_x", title: "x"}], unread: {}},
  ($d): {tabs: [{sessionID: "ses_old", title: "old"}], unread: {}}}}' >"$OC_TABS_FILE"
active ses_a ses_b ses_kid
run
rc_is "live sessions -> 0" $? 0
check "live sessions are appended after the existing tab" \
  ".cwd[\"$dir\"].tabs | map(.sessionID) == [\"ses_old\",\"ses_a\",\"ses_b\"]"
check "a subagent session becomes its parent's tab, once" ".cwd[\"$dir\"].tabs | map(.title) == [\"old\",\"job-a\",\"job-b\"]"
check "another directory's tabs are kept" '.cwd["/elsewhere"].tabs[0].sessionID == "ses_x"'
if [[ $(cat "$tmp/launch" 2>/dev/null) == "-s ses_a" ]]; then ok "opens OpenCode on the first live session"
else no "opens OpenCode on the first live session" "launch=$(cat "$tmp/launch" 2>/dev/null)"; fi

rm -f "$tmp/launch"
run -n
check "a second run adds no duplicate" ".cwd[\"$dir\"].tabs | length == 3"
if [[ -e $tmp/launch ]]; then no "-n never opens OpenCode" "launch=$(cat "$tmp/launch")"; else ok "-n never opens OpenCode"; fi

active
run -n ses_a
rc_is "an id given with no live session -> 0" $? 0
before=$(cat "$OC_TABS_FILE")
run -n
rc_is "no live session -> 1" $? 1
unchanged "no live session leaves the file alone" "$before"
run -n ses_gone
rc_is "an unknown id alone -> 1" $? 1

touch "$tmp/down"
run -n
rc_is "service down -> 2" $? 2
rm -f "$tmp/down"

active ses_a
echo '{"tabs":{"scope":"global"}}' >"$OC_CLI_JSON"
run -n
check "tabs.scope global writes the global list" '.global.tabs | map(.sessionID) == ["ses_a"]'
rm -f "$OC_CLI_JSON"

echo 'not json' >"$OC_TABS_FILE"
run -n
rc_is "an unreadable tabs.json -> 2" $? 2
unchanged "an unreadable tabs.json is never overwritten" "not json"

rm -f "$OC_TABS_FILE"
run -n
check "a missing tabs.json is created" ".cwd[\"$dir\"].tabs[0].sessionID == \"ses_a\""

printf 'pass=%s fail=%s\n' "$pass" "$fail"
[[ $fail -eq 0 ]]
