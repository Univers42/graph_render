#!/usr/bin/env bash
# test-oc-tabs.sh — behaviour tests for scripts/orch/oc-tabs.sh. Plain bash, no bats, no service:
# OC points at a stub opencode that replays canned JSON for `api GET` and records any other call;
# OC_WT_NEW points at a stub worktree builder that makes the folder under GM_SCRATCH and logs its name.
# Exits 0 when every case passes, 1 otherwise. Never touches the real tabs.json.
set -uo pipefail
here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
TABS=$here/oc-tabs.sh
tmp=$(mktemp -d "${TMPDIR:-/tmp}/octabs.XXXXXX")
trap 'rm -rf "$tmp"' EXIT
mkdir -p "$tmp/sess" "$tmp/here" "$tmp/scratch/wt"
export STUB_DIR=$tmp OC=$tmp/opencode OC_TABS_FILE=$tmp/tabs.json OC_CLI_JSON=$tmp/cli.json
export OC_WT_NEW=$tmp/wt-new GM_SCRATCH=$tmp/scratch
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
if [[ ${1-} == session ]]; then
  [[ -f $STUB_DIR/down ]] && exit 1
  cat "$STUB_DIR/list.json"
  exit
fi
if [[ ${1-} == api ]]; then
  case $3 in
    /api/session/active) [[ -f $STUB_DIR/down ]] && exit 1; cat "$STUB_DIR/active.json" ;;
    /api/session\?*) jq -c "{data: .}" "$STUB_DIR/records.json" ;;
    /api/session/*) cat "$STUB_DIR/sess/${3##*/}.json" 2>/dev/null || exit 1 ;;
  esac
  exit
fi
printf '%s\n' "$*" >"$STUB_DIR/launch"
STUB
cat >"$OC_WT_NEW" <<'STUB'
#!/usr/bin/env bash
printf '%s\n' "$1" >>"$STUB_DIR/wtnew"
[[ -f $STUB_DIR/wtfail ]] && exit 1
mkdir -p "$GM_SCRATCH/wt/$1"
STUB
chmod +x "$OC" "$OC_WT_NEW"

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

printf -- '--- -a ---\n'
wt=$tmp/scratch/wt
mkdir -p "$wt/keep" "$wt/tiny"
# rec <id> <outcome|-> <output tokens> <directory> — one session record, appended to records.json
rec() { jq -c --arg i "$1" --arg o "$2" --argjson t "$3" --arg d "$4" '. + [{id: $i, title: "t-\($i)",
  tokens: {output: $t}, location: {directory: $d}} + (if $o == "-" then {} else {outcome: $o} end)]' \
  "$tmp/records.json" >"$tmp/records.next" && mv "$tmp/records.next" "$tmp/records.json"; }
echo '[{"id":"r_live","projectId":"p1"}]' >"$tmp/list.json"
echo '[]' >"$tmp/records.json"
rec r_live - 10 "$wt/gone-live"
rec r_keep succeeded 9000 "$wt/keep"
rec r_probe succeeded 2900 "$wt/tiny"
rec r_dead succeeded 9000 "$wt/merged"
rec r_redo succeeded 9000 "$wt/redo"
rec r_cut interrupted 9000 "$wt/redo"
rec r_broken interrupted 9000 "$wt/fixme"
rec r_lost failed 9000 /nowhere/at-all
jq -n --arg d "$dir" '{global: {tabs: [], unread: {}}, cwd: {($d): {tabs: [{sessionID: "r_probe", title: "p"},
  {sessionID: "r_dead", title: "d"}, {sessionID: "ses_other", title: "o"}, {sessionID: "r_keep", title: "k"}], unread: {}}}}' \
  >"$OC_TABS_FILE"
active r_live
rm -f "$tmp/launch" "$tmp/wtnew"
run -a
rc_is "-a -> 0" $? 0
check "-a keeps running, kept and repaired sessions, newest first, and drops the rest" \
  ".cwd[\"$dir\"].tabs | map(.sessionID) == [\"ses_other\",\"r_keep\",\"r_live\",\"r_broken\"]"
check "-a takes probes out of the tab list" ".cwd[\"$dir\"].tabs | all(.sessionID != \"r_probe\")"
check "-a takes dead sessions out of the tab list" ".cwd[\"$dir\"].tabs | all(.sessionID != \"r_dead\")"
if [[ $(cat "$tmp/wtnew" 2>/dev/null) == fixme ]]; then ok "-a rebuilds only the broken job's worktree, by name"
else no "-a rebuilds only the broken job's worktree, by name" "wt-new calls: $(paste -sd' ' "$tmp/wtnew" 2>/dev/null)"; fi
if [[ -d $wt/fixme ]]; then ok "the rebuilt folder exists"; else no "the rebuilt folder exists" "no $wt/fixme"; fi
if [[ $(cat "$tmp/launch" 2>/dev/null) == "-s r_live" ]]; then ok "-a opens OpenCode on the newest kept session"
else no "-a opens OpenCode on the newest kept session" "launch=$(cat "$tmp/launch" 2>/dev/null)"; fi

rm -rf "$wt/fixme" "$tmp/wtnew"
touch "$tmp/wtfail"
run -a -n
check "a worktree that cannot be rebuilt drops its session" ".cwd[\"$dir\"].tabs | all(.sessionID != \"r_broken\")"
rm -f "$tmp/wtfail"
sess r_probe t-r_probe
run -a -n r_probe
check "-a -n with an id adds that id even when it is a probe" ".cwd[\"$dir\"].tabs | any(.sessionID == \"r_probe\")"

jq 'map(select(.id == "r_dead"))' "$tmp/records.json" >"$tmp/records.next" && mv "$tmp/records.next" "$tmp/records.json"
jq -n --arg d "$dir" '{global: {tabs: [], unread: {}}, cwd: {($d): {tabs: [{sessionID: "r_dead", title: "d"}], unread: {}}}}' \
  >"$OC_TABS_FILE"
active
run -na
rc_is "-a with every session out -> 1" $? 1
check "-a with every session out still prunes the tab list" ".cwd[\"$dir\"].tabs == []"
echo '[]' >"$tmp/list.json"
run -na
rc_is "-a over an empty project -> 1" $? 1
echo 'not json' >"$tmp/list.json"
run -na
rc_is "-a over an unreadable list -> 2" $? 2
echo '[{"id":"r","projectId":"p1"}]' >"$tmp/list.json"
echo 'not json' >"$tmp/records.json"
run -na
rc_is "-a over unreadable records -> 2" $? 2
touch "$tmp/down"
run -na
rc_is "-a with the service down -> 2" $? 2
rm -f "$tmp/down"
run -x
rc_is "an unknown option -> 2" $? 2

printf 'pass=%s fail=%s\n' "$pass" "$fail"
[[ $fail -eq 0 ]]
