#!/usr/bin/env bash
# service-max-body.sh — GRAPH_MAX_BODY is inside what one slot is budgeted for, on the image
# (docs/reviews/review-svc-r3.md new condition 1).
#
#   scripts/service-max-body.sh
#   SVC_MAX_BODY=67108864 scripts/service-max-body.sh    the negative control: the default body at
#                                                       --memory 8g holds a slot, so the server
#                                                       starts and the row must fail
#
# What it proves: with GRAPH_WORKERS unset, GRAPH_MAX_BODY at its 1 GiB range ceiling and the
# container at the 8 GiB the docs call the floor, the derived budget holds no slot, so the server
# refuses to start: exit 2 and the existing `GRAPH_WORKERS: unset, and memory.max holds no slot`
# line on stderr. Before condition 1 the per-slot figure was fixed at the 64 MiB body, so this
# shape started, derived one slot, and asked ~19.6 GB of it (docs/measurements/service-caps.md
# "Memory per slot").
#
# Env:
#   SVC_MAX_BODY   the body the container is given, 1073741824 (the 1 GiB range ceiling) by
#                  default. The negative control sets it to 67108864, the default body.
#
# Report: target/service-max-body/<tag>/report.txt, one `PASS|FAIL <check> <detail>` line for the
#   exit code, the refusal line and `listening`, plus `#` lines for the image, the body, the
#   container memory and the per-slot figure the same arithmetic as `src/config/slots.rs`.
#
# Exit: 0 every check PASS · 1 a check FAILed · 2 could not run (no image, no budget constant).
#
# Caveat: a start refusal, not a budget measurement. This row says the derived slot count is 0 at
# this shape and nothing about what a slot would cost; `scripts/service-limits.sh` measures that,
# at the default body, where the per-slot figure is unchanged by condition 1.
set -euo pipefail

here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
root=$(git -C "$here" rev-parse --show-toplevel)
cd "$root"

slots=server/graph-server/src/config/slots.rs
tag=current
[[ -n ${SVC_MAX_BODY-} ]] && tag=negctl
body=${SVC_MAX_BODY:-1073741824}
memory=8g
report=target/service-max-body/$tag/report.txt
work=target/service-max-body/$tag
mkdir -p "$work"
: >"$report"
fails=0
name="svc-max-body-$$-$tag"
# How long the server may take to answer `listening` or refuse. Caveat: a fixed bound, so a host
# so loaded the container cannot start inside it reads as a failure rather than a hang.
CAP_S=60
trap 'docker rm -f "$name" >/dev/null 2>&1 || true' EXIT

log() { printf '\033[1m[svc-max-body]\033[0m %s\n' "$*" >&2; }
say() { printf '%s\n' "$*" >>"$report"; }
note() { say "# $*"; }
die() {
  log "$*"
  say "# could not run: $*"
  exit 2
}

# One check line, and the exit code it earns.
check() {
  if [[ $2 == 0 ]]; then
    say "PASS $1 $3"
  else
    say "FAIL $1 $3"
    fails=$((fails + 1))
  fi
}

# A `pub const NAME: u64 = <digits>;` line of slots.rs, as a number.
slot_bytes() {
  local found
  found=$(grep -oP "(?<=pub const $1: u64 = )[0-9_]+" "$slots") || die "slots.rs: $1 not found"
  printf '%s\n' "${found//_/}"
}

# per_slot_bytes(max_body) as src/config/slots.rs computes it: the run peak, the body, and the
# contract ingest peak scaled to the body at the measured ratio. Bash integers are 64-bit, and
# 1 GiB x 1.2 GB is well inside that.
per_slot() {
  local run ingest at=$1
  run=$(slot_bytes RUN_PEAK_BYTES)
  ingest=$(slot_bytes INGEST_PEAK_BYTES)
  printf '%s\n' $((run + at + (at * ingest + BODY_BYTES - 1) / BODY_BYTES))
}

BODY_BYTES=$(slot_bytes BODY_BYTES) || die "slots.rs: BODY_BYTES not found"
per_slot_at_body=$(per_slot "$body")
limit=$((8 * 1024 * 1024 * 1024))
# Reuse the last build when there is one, and build when there is not: the row must work on a tree
# where service-image.rows has not run.
image=$(scripts/service.sh image 2>/dev/null) || image=$(scripts/service.sh build) ||
  die "no image: scripts/service.sh build"
log "image $image"
note "image $image"

# One key in the file the server refuses to start without. Only its file line is kept: this row
# reads no response body, so the key itself goes to /dev/null and is never printed.
install -m 0640 /dev/null "$work/keys"
scripts/service.sh keygen svc-max-body-gate "$work/keys" >/dev/null || die "keygen refused"
load_start=$(cut -d' ' -f1 /proc/loadavg)
note "GRAPH_MAX_BODY $body, GRAPH_WORKERS unset, container --memory $memory --memory-swap $memory"
note "per_slot_bytes($body) = $per_slot_at_body B against $limit B of memory.max"
note "load1 start $load_start ($(cat /proc/loadavg))"

scripts/orch/drun --rm --name "$name" --memory "$memory" --memory-swap "$memory" \
  --read-only --cap-drop ALL --security-opt no-new-privileges \
  --group-add "$(stat -c %g "$work/keys")" -v "$(readlink -f "$work/keys"):/run/graph/keys:ro" \
  -e GRAPH_API_KEYS_FILE=/run/graph/keys -e "GRAPH_MAX_BODY=$body" \
  "$image" >"$work/stdout.log" 2>"$work/stderr.log" &
runner=$!

# Either `listening` or the refusal line ends the wait; anything else is a container that would
# keep running, which the trap stops.
settled=0
for _ in $(seq 1 $((CAP_S * 10))); do
  if grep -q '"event":"listening"' "$work/stdout.log" 2>/dev/null; then settled=1; break; fi
  if grep -q 'holds no slot' "$work/stderr.log" 2>/dev/null; then settled=1; break; fi
  kill -0 "$runner" 2>/dev/null || break
  sleep 0.1
done
rc=0
wait "$runner" || rc=$?
docker rm -f "$name" >/dev/null 2>&1 || true
log "the container settled=$settled, exit $rc"

served=$(grep -c '"event":"listening"' "$work/stdout.log" || true)
refusal=$(grep -c 'GRAPH_WORKERS: unset, and memory.max holds no slot' "$work/stderr.log" || true)
check exit "$([[ $rc == 2 ]] && echo 0 || echo 1)" "container exit $rc, expected 2"
check refusal "$([[ $refusal == 1 ]] && echo 0 || echo 1)" "the refusal line appears $refusal times on stderr"
check started "$([[ $served == 0 ]] && echo 0 || echo 1)" "$served listening lines, expected none"
note "stderr: $(tr '\n' ' ' <"$work/stderr.log")"
note "load1 end $(cut -d' ' -f1 /proc/loadavg) ($(cat /proc/loadavg))"

log "$(grep -c '^FAIL' "$report" || true) failing checks in $report"
((fails == 0))