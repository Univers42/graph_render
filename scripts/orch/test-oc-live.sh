#!/usr/bin/env bash
# test-oc-live.sh — behaviour tests for scripts/orch/oc-live.sh and for the oc-status.sh state rules.
# Plain bash, no bats and no listener: the port, the port-discovery command, the service.json path
# and the curl command are injected (OC_LIVE_PORT, OC_LIVE_SS, OC_LIVE_CONFIG, OC_LIVE_CURL) and
# curl is a stub script that replays canned JSON out of a fixture directory.
# Exits 0 when every case passes, 1 otherwise. Never contacts the real service.
set -uo pipefail
here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
LIVE=$here/oc-live.sh
STATUS=$here/oc-status.sh
tmp=$(mktemp -d "${TMPDIR:-/tmp}/oclive.XXXXXX")
trap 'rm -rf "$tmp"' EXIT
WT=$tmp/wt
OTHER=$tmp/other
PW=unit-test-password
export STUB_DIR=$tmp STUB_ARGV=$tmp/argv
mkdir -p "$WT" "$OTHER" "$tmp/sess" "$tmp/root/wtA/target/wf" "$tmp/bin"
printf '{"password":"%s"}\n' "$PW" >"$tmp/service.json"
pass=0
fail=0

ok() { pass=$((pass + 1)); printf 'ok    %s\n' "$1"; }
no() { fail=$((fail + 1)); printf 'FAIL  %s: %s\n' "$1" "$2"; }

# is <name> <want-rc> <want-stdout> <cmd> [args...]
is() {
  local name=$1 wrc=$2 wout=$3 out rc
  shift 3
  out=$("$@" 2>"$tmp/err")
  rc=$?
  if [[ $rc == "$wrc" && $out == "$wout" ]]; then ok "$name"
  else no "$name" "rc=$rc want=$wrc out=$(printf '%q' "$out") want=$(printf '%q' "$wout") err=$(head -c 160 "$tmp/err" | tr '\n' ' ')"; fi
}

# sess <id> <directory> <title>
sess() {
  printf '{"data":{"id":"%s","title":"%s","location":{"directory":"%s"}}}\n' "$1" "$3" "$2" \
    >"$tmp/sess/$1.json"
}

# active <id>...  (no id = the service reports no draining session)
active() {
  local json='{"data":{' id first=1
  for id in "$@"; do
    ((first)) || json+=','
    first=0
    json+="\"$id\":{\"type\":\"running\"}"
  done
  printf '%s}}\n' "$json" >"$tmp/active.json"
}

# the fake curl: drains the curl -K - config, records its argv, replays the fixtures
cat >"$tmp/curl-stub" <<'STUB'
#!/usr/bin/env bash
set -uo pipefail
cfg=$(cat) || exit 4
[[ -n $cfg ]] || { echo "stub: empty curl config on stdin" >&2; exit 3; }
printf '%s' "$*" >"$STUB_ARGV"
url=${*: -1}
case $url in
  */api/session/active)
    [[ ${STUB_ACTIVE_RC-0} -eq 0 ]] || exit "${STUB_ACTIVE_RC}"
    cat "$STUB_DIR/active.json"
    ;;
  */api/session/*)
    id=${url##*/}
    [[ -f $STUB_DIR/sess/$id.json ]] || exit 22
    cat "$STUB_DIR/sess/$id.json"
    ;;
  *) echo "stub: unexpected url $url" >&2; exit 5 ;;
esac
STUB
chmod +x "$tmp/curl-stub"
# a ss that finds no listening opencode service
printf '#!/bin/sh\nexit 0\n' >"$tmp/bin/ss"
chmod +x "$tmp/bin/ss"

# run <args...> — oc-live.sh against the fixtures
run() {
  OC_LIVE_PORT=4099 OC_LIVE_SS=ss OC_LIVE_CURL="$tmp/curl-stub" \
    OC_LIVE_CONFIG="$tmp/service.json" "$LIVE" "$@"
}

# st — oc-status.sh over the fake root, with the same fixtures
st() {
  OC_LIVE_PORT=4099 OC_LIVE_SS=ss OC_LIVE_CURL="$tmp/curl-stub" \
    OC_LIVE_CONFIG="$tmp/service.json" OC_LIVE_BIN="$LIVE" "$STATUS" "$tmp/root"
}

want_state() {
  local name=$1 want=$2 got
  got=$(st | awk 'NR==1{print $3}')
  if [[ $got == "$want" ]]; then ok "$name"; else no "$name" "state=$got want=$want"; fi
}

printf -- '--- oc-live.sh ---\n'
sess ses_a "$WT" wt-job
sess ses_b "$OTHER" other-job
active ses_a ses_b
is "live session in the worktree -> 0 + id and title" 0 "ses_a wt-job" run "$WT"
is "the same session seen from its own worktree -> 0" 0 "ses_b other-job" run "$OTHER"
is "a session live only elsewhere -> 1" 1 "" run "$tmp/nomatch"
is "pinned session id live in the worktree -> 0" 0 "ses_a wt-job" run "$WT" ses_a
is "pinned session id live elsewhere -> 1" 1 "" run "$WT" ses_b
is "pinned id stale, another session is in the worktree -> 0" 0 "ses_a wt-job" run "$WT" ses_gone
is "unknown worktree -> 1" 1 "" run "$tmp/nowhere"
ln -s "$WT" "$tmp/wt-link"
is "worktree named through a symlink (wt-new.sh GM_WT_STORE) -> 0" 0 "ses_a wt-job" run "$tmp/wt-link"
sess ses_l "$tmp/wt-link" via-link
active ses_l
is "session started through the symlink, asked by the real path -> 0" 0 "ses_l via-link" run "$WT"

sess ses_c "$WT/" wt-trailing
active ses_c
is "directory with a trailing slash still matches -> 0" 0 "ses_c wt-trailing" run "$WT"
is "trailing slash on the argument still matches -> 0" 0 "ses_c wt-trailing" run "$WT/"
sess ses_d "$WT//" wt-two
active ses_d
is "directory with two trailing slashes still matches -> 0" 0 "ses_d wt-two" run "$WT"

active
is "no active session -> 1" 1 "" run "$WT"
is "no active session, pinned id -> 1" 1 "" run "$WT" ses_a

sess ses_a "$WT" wt-job
active ses_a
is "happy path still passes" 0 "ses_a wt-job" run "$WT"
if grep -qF "$PW" "$tmp/argv" 2>/dev/null; then
  no "password never reaches curl's argv" "found the password in: $(cat "$tmp/argv")"
else ok "password never reaches curl's argv"; fi
if grep -qF -- '-K -' "$tmp/argv"; then ok "password is handed to curl on stdin (-K -)"
else no "password is handed to curl on stdin (-K -)" "argv=$(cat "$tmp/argv")"; fi

export STUB_ACTIVE_RC=7
is "curl fails -> 2" 2 "" run "$WT"
unset STUB_ACTIVE_RC

printf '{"_tag":"UnauthorizedError","message":"Authentication required"}\n' >"$tmp/active.json"
is "unauthorized answer -> 2 (never 1)" 2 "" run "$WT"
active ses_a

rm -f "$tmp/sess/ses_a.json"
is "every session lookup fails -> 2" 2 "" run "$WT"
sess ses_a "$WT" wt-job

is "no listening port -> 2" 2 "" \
  env OC_LIVE_SS="$tmp/bin/ss" OC_LIVE_CURL="$tmp/curl-stub" OC_LIVE_CONFIG="$tmp/service.json" "$LIVE" "$WT"
# two opencode listeners: a stranger first, the service second; only the service's port is asked
bash -c 'exec -a opencode sleep 30' &
stranger=$!
bash -c 'exec -a "opencode serve --service" sleep 30' &
service=$!
printf '#!/bin/sh\necho "LISTEN 0 512 127.0.0.1:1111 0.0.0.0:* users:((\\"opencode\\",pid=%s,fd=9))"\necho "LISTEN 0 512 127.0.0.1:4099 0.0.0.0:* users:((\\"opencode\\",pid=%s,fd=9))"\n' \
  "$stranger" "$service" >"$tmp/bin/ss2"
chmod +x "$tmp/bin/ss2"
active
is "port discovery skips a non-service listener" 1 "" \
  env OC_LIVE_SS="$tmp/bin/ss2" OC_LIVE_CURL="$tmp/curl-stub" OC_LIVE_CONFIG="$tmp/service.json" "$LIVE" "$WT"
if [[ $(<"$tmp/argv") == *127.0.0.1:4099/* ]]; then ok "the service's port is the one asked"
else no "the service's port is the one asked" "argv=$(<"$tmp/argv")"; fi
kill "$stranger" "$service" 2>/dev/null
active ses_a
is "no service.json -> 2" 2 "" \
  env OC_LIVE_PORT=4099 OC_LIVE_CURL="$tmp/curl-stub" OC_LIVE_CONFIG="$tmp/absent.json" "$LIVE" "$WT"
printf '{"password":"a\nb"}\n' >"$tmp/nl.json"
is "password with a newline -> 2" 2 "" \
  env OC_LIVE_PORT=4099 OC_LIVE_CURL="$tmp/curl-stub" OC_LIVE_CONFIG="$tmp/nl.json" "$LIVE" "$WT"
is "no argument -> 2" 2 "" run

printf -- '--- oc-status.sh ---\n'
wf=$tmp/root/wtA/target/wf
sess ses_a "$tmp/root/wtA" wtA-job
sess ses_b "$WT" wt-job
printf '{}\n' >"$wf/lab1.jsonl"
printf 'ses_a\n' >"$wf/lab1.session-id"
printf '0\n' >"$wf/lab1.rc"
active ses_a
want_state "a journal with .rc is done(rc)" "done(rc=0)"
rm -f "$wf/lab1.rc"
touch "$wf/lab1.jsonl"
want_state "live session, fresh journal -> RUNNING" RUNNING
touch -d '-40 minutes' "$wf/lab1.jsonl"
want_state "live session, journal older than 30min -> STALLED" STALLED
touch "$wf/lab1.jsonl"
active ses_b
want_state "live session in another worktree -> DEAD" DEAD
active ses_a
export STUB_ACTIVE_RC=7
want_state "service cannot be asked -> UNKNOWN" "UNKNOWN"
unset STUB_ACTIVE_RC

printf -- '--- oc-job.sh fence ---\n'
JOB=$here/oc-job.sh
mkdir -p "$tmp/bin2"
cat >"$tmp/bin2/oc-run.sh" <<'STUB'
#!/usr/bin/env bash
set -uo pipefail
printf '%s\n' "$*" >"$STUB_MARK"
exit 42
STUB
chmod +x "$tmp/bin2/oc-run.sh"
printf 'do nothing\n' >"$tmp/body.txt"
export STUB_MARK=$tmp/mark STUB_LIVE_RC_FILE=$tmp/live-rc
: >"$STUB_MARK"
cat >"$tmp/live-stub" <<'STUB'
#!/usr/bin/env bash
set -uo pipefail
printf '%s\n' "session ses_x busy"
exit "$(cat "$STUB_LIVE_RC_FILE")"
STUB
chmod +x "$tmp/live-stub"
printf '0\n' >"$STUB_LIVE_RC_FILE"
job() { OC_LIVE_BIN="$tmp/live-stub" OC_JOB_BIN="$tmp/bin2" "$JOB" lab "$tmp/nowhere" build "$tmp/body.txt"; }
# job_rc <oc-live rc> — the same call with a different oc-live.sh answer
job_rc() { printf '%s\n' "$1" >"$STUB_LIVE_RC_FILE"; job; }
is "a live session in the worktree refuses the job" 3 "refused: session ses_x busy already works in $tmp/nowhere" job_rc 0
if [[ -s $STUB_MARK ]]; then no "the refused job never ran" "oc-run.sh was called"; else ok "the refused job never ran"; fi
is "an unaskable service refuses the job" 3 "refused: cannot prove $tmp/nowhere is free: session ses_x busy" job_rc 2
if [[ -s $STUB_MARK ]]; then no "an unaskable service never starts a job" "oc-run.sh was called"; else ok "an unaskable service never starts a job"; fi
is "an oc-live.sh that fails for any other reason refuses" 3 "refused: oc-live.sh failed (rc=9): session ses_x busy" job_rc 9
is "a free worktree lets the job run (negative control)" 2 "job rc=42" job_rc 1
if [[ -s $STUB_MARK ]]; then ok "the free worktree really did call oc-run.sh"; else no "the free worktree really did call oc-run.sh" "no marker"; fi

printf 'pass=%s fail=%s\n' "$pass" "$fail"
[[ $fail -eq 0 ]]
