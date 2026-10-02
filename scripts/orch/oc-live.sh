#!/usr/bin/env bash
# oc-live.sh <worktree> [session-id] — is an OpenCode job live in <worktree>?
# Asks the service which session drains it owns (GET /api/session/active), then each one's
# directory (GET /api/session/<id>), and compares that directory with <worktree>.
# Exit 0 = a live session works in <worktree>, and its "<id> <title>" is printed on stdout.
# Exit 1 = the service was asked and no draining session works there.
# Exit 2 = could not ask: no port, no password, service down, or an answer we cannot read.
#         2 is never read as 1 — a worktree nobody could query is not a free worktree.
# The optional session-id pins the answer to that session when the service still lists it; when the
# id is stale the whole drain set is scanned again, so a busy worktree is never called free
# because the label's .session-id went out of date.
# Secret: the password is read from ~/.config/opencode/service.json into a shell variable (never
# exported, never printed) and handed to curl on stdin with `curl -K -`, so it never reaches argv
# and never shows up in ps.
# Test seams: OC_LIVE_PORT (skip port discovery), OC_LIVE_SS (the port discovery command),
# OC_LIVE_CONFIG (the service.json path), OC_LIVE_CURL (the curl command), OC_LIVE_TIMEOUT.
# Ponytail: "active" is the service's foreground *drain* set, not "a session exists": a live
# session idling between turns (waiting for a permission, a prompt or the 30s poll) is absent from
# it, so exit 1 can be returned for a worktree an agent still owns — which is why oc-job.sh keeps
# its pgrep fence as a second opinion. It lies the other way too: a drain belongs to the one
# OpenCode process that owns it, so a service restart empties the set while the journal keeps
# growing, and exit 1 is not proof of death. A session's directory is the cwd its client reported,
# compared as a string with trailing slashes stripped and symlinks left unresolved.
set -uo pipefail

# ask <what> — exit 2 with a reason that never contains the password
ask() {
  printf 'oc-live: %s\n' "$1" >&2
  exit 2
}

# dir <path> — the same directory with trailing slashes removed ('/' stays '/')
dir() {
  local d=$1
  while [[ ${#d} -gt 1 && $d == */ ]]; do d=${d%/}; done
  printf '%s' "$d"
}

# port — the service port: OC_LIVE_PORT, else the port of the listening `opencode serve --service`.
# Not the first opencode listener: standalone runs and other sessions listen too, and on 2026-09-30
# the first one was a stranger, so every job was refused with "service down?".
port() {
  local p=${OC_LIVE_PORT-} cmd line pid
  if [[ -z $p ]]; then
    read -r -a cmd <<<"${OC_LIVE_SS:-ss}"
    while IFS= read -r line; do
      pid=$(sed -n 's/.*pid=\([0-9]*\).*/\1/p' <<<"$line")
      [[ -n $pid ]] || continue
      tr '\0' ' ' <"/proc/$pid/cmdline" 2>/dev/null | grep -q -- ' --service' || continue
      p=$(awk '{print $4}' <<<"$line")
      p=${p##*:}
      break
    done < <("${cmd[@]}" -ltnp 2>/dev/null | grep opencode)
  fi
  [[ $p =~ ^[0-9]+$ ]] || return 1
  printf '%s' "$p"
}

# secret — the service password, or nothing when it cannot be used in a curl config
secret() {
  local cfg=${OC_LIVE_CONFIG:-$HOME/.config/opencode/service.json} pw
  pw=$(jq -er '.password // empty' "$cfg" 2>/dev/null) || return 1
  [[ -n $pw && $pw != *$'\n'* ]] || return 1
  printf '%s' "$pw"
}

# get <port> <path> <password> — one authenticated GET, stdout is the body, nonzero = no answer
get() {
  local p=$1 path=$2 pw=$3 esc body
  esc=${pw//\\/\\\\}
  esc=${esc//\"/\\\"}
  body=$("$curl" -sS -f -K - --max-time "$tmo" "http://127.0.0.1:$p$path" \
    <<<"user = \"opencode:$esc\"" 2>/dev/null) || return 1
  printf '%s' "$body"
}

[[ $# -ge 1 && -n ${1-} ]] || { echo "usage: oc-live.sh <worktree> [session-id]" >&2; exit 2; }
# Both sides canonical: a worktree may be a symlink (wt-new.sh GM_WT_STORE), and a session keeps the path
# its client started in.
want=$(dir "$(realpath -m -- "$1")")
pin=${2-}
tmo=${OC_LIVE_TIMEOUT:-5}
curl=${OC_LIVE_CURL:-curl}
p=$(port) || ask "no opencode service port (ss -ltnp | grep opencode found nothing)"
pw=$(secret) || ask "no usable password in ${OC_LIVE_CONFIG:-$HOME/.config/opencode/service.json}"
body=$(get "$p" /api/session/active "$pw") || ask "GET /api/session/active failed (service down?)"
jq -e '(.data|type) == "object"' >/dev/null 2>&1 <<<"$body" \
  || ask "GET /api/session/active did not answer with a session set"
ids=$(jq -r '.data | keys[]' <<<"$body")
if [[ -n $pin ]]; then
  if printf '%s\n' "$ids" | grep -qxF -- "$pin"; then ids=$pin; fi
fi
[[ -n $ids ]] || exit 1
seen=0
while IFS= read -r id; do
  [[ -n $id ]] || continue
  body=$(get "$p" "/api/session/$id" "$pw") || continue
  d=$(jq -r '.data.location.directory // empty' <<<"$body" 2>/dev/null)
  title=$(jq -r '.data.title // ""' <<<"$body" 2>/dev/null)
  [[ -n $d ]] || continue
  seen=$((seen + 1))
  if [[ $(dir "$(realpath -m -- "$d")") == "$want" ]]; then
    printf '%s %s\n' "$id" "${title//[$'\n\t']/ }"
    exit 0
  fi
done <<<"$ids"
((seen > 0)) || ask "no draining session could be looked up"
exit 1
