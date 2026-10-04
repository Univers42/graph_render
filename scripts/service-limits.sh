#!/usr/bin/env bash
# service-limits.sh — the memory budget of one worker slot, measured on the image (Verdict
# condition 3, docs/reviews/review-svc-r2.md new condition 1).
#
#   scripts/service-limits.sh
#
# What it proves: with the container capped at PER_SLOT_BYTES + BASE_BYTES, rounded up to a whole
# MiB and read from server/graph-server/src/config/slots.rs (never written here), one slot takes
# the worst request the default limits admit, twice, and answers 200 both times with the kernel
# never OOM-killing it: the budget fits the container, which no row proved before.
#
# The two requests, one after the other, both `layout.circular.radial` + `post.style.orthogonal`:
#   1. `source=contract`, body = the densest contract document at most GRAPH_MAX_BODY admits;
#   2. `source=studio`,   body = the densest studio document at most the same limit admits.
# Each source gets its own densest graph because one node count does not fit both bodies at that
# limit: a studio document costs ~483 B per node and edge pair, a contract document ~165 B per
# record (docs/measurements/service-caps.md "Memory per slot"), so the contract graph is ~2.9x
# larger in nodes. These are the two documents the per-slot ingest terms were measured on.
# The contract generator is the one in crates/graph-wasm/src/memory_measure/ingest_peak.rs, byte
# for byte, so its ingest heap is the measured 18.25x the body; the studio document is the ladder's
# own generator, `graph-cli bench --n N --seed 1 --emit-scale-fixture`.
#
# Env:
#   SERVICE_LIMITS_MEM   a container limit such as `1g` in place of M. It also picks the report
#                       directory `negctl` instead of `current`, and it is the negative control:
#                       one slot does not fit under it and the kernel kills the process.
#
# Report: target/service-limits/<tag>/report.txt, one `PASS|FAIL <check> <detail>` line for each of
#   status-contract, status-studio, oom (OOMKilled false and still running) and peak (the cgroup's
#   memory.peak at most M), plus `#` lines for M, the two body sizes, n, m and the peak in bytes.
#
# Exit: 0 every check PASS · 1 a check FAILed · 2 could not run (no image, no port, no `listening`).
#
# Caveat: one run on one host, and `memory.peak` is the cgroup's high-water mark over the whole
# container — the process, its stacks and every page the kernel kept from the bodies and the two
# response faces. It is an upper bound on what one slot costs, not its RSS: it never comes down
# between the two requests, so the second is charged the first one's peak. A layout or post past the
# two named here is not measured, and neither is a second slot.
set -euo pipefail

here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
root=$(git -C "$here" rev-parse --show-toplevel)
cd "$root"

MIB=1048576
slots=server/graph-server/src/config/slots.rs
tag=current
[[ -n ${SERVICE_LIMITS_MEM-} ]] && tag=negctl
report=target/service-limits/$tag/report.txt
mkdir -p "${report%/*}"
: >"$report"
fails=0
name="svc-limits-$$-$tag"
work=target/service-limits/$tag
trap 'docker rm -f "$name" >/dev/null 2>&1 || true' EXIT

log() { printf '\033[1m[service-limits]\033[0m %s\n' "$*" >&2; }
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

# The port the container published, from `docker port`.
published_port() {
  local mapped
  mapped=$(docker port "$name" 8080/tcp 2>/dev/null | head -1) || true
  printf '%s\n' "${mapped##*:}"
}

# `listening` within $1 tenths of a second, or exit 2: the server refusing to start is a run that
# could not happen, not a check that failed.
await_listening() {
  local tenths=$1 i
  for ((i = 0; i < tenths; i++)); do
    docker logs "$name" 2>&1 | grep -q '"event":"listening"' && return 0
    sleep 0.1
  done
  docker logs "$name" 2>&1 | tail -3 >&2 || true
  die "no listening line within $((tenths / 10))s"
}

# The densest contract document at most $1 bytes: ingest_peak.rs's generator, record for record,
# stopped one record short of the limit so the finished document is under it and never refused 413.
contract_body() {
  local limit=$1 out=$2 records
  records=$(
    awk -v limit="$limit" -v out="$out" '
      BEGIN {
        head = "{\"version\":1,\"source\":\"rows\",\"collections\":[{\"id\":\"task\",\"name\":\"Tasks\",\"titleField\":\"name\",\"fields\":[{\"id\":\"name\",\"name\":\"Name\",\"role\":\"title\",\"link\":null},{\"id\":\"up\",\"name\":\"Up\",\"role\":\"parent\",\"link\":null},{\"id\":\"labels\",\"name\":\"Labels\",\"role\":\"tags\",\"link\":null},{\"id\":\"effort\",\"name\":\"Effort\",\"role\":\"weight\",\"link\":null},{\"id\":\"blocks\",\"name\":\"Blocks\",\"role\":\"link\",\"link\":{\"collection\":\"task\",\"cardinality\":\"many\",\"symmetric\":false}}]}],\"records\":["
        printf "%s", head >out
        len = length(head)
        for (i = 0; ; i++) {
          rec = sprintf("{\"id\":\"r%d\",\"collection\":\"task\",\"deleted\":false,\"updatedAt\":%d,\"values\":{\"name\":\"Task %d\",\"labels\":[\"g%d\"],\"effort\":%d", i, i, i, i % 16, i % 7 + 1)
          if (i > 0) rec = rec sprintf(",\"up\":\"r%d\",\"blocks\":[\"r%d\"]", int((i - 1) / 2), int((i * 2654435769) / 128) % i)
          rec = rec "}}"
          if (len + length(rec) + 2 > limit) break
          if (i > 0) { printf "," >out; len++ }
          printf "%s", rec >out
          len += length(rec)
        }
        printf "]}" >out
        close(out)
        printf "%d\n", i
      }'
  )
  printf '%s\n' "$records"
}

# The ladder generator, `graph-cli bench --emit-scale-fixture`, at $2 nodes into $1.
emit_studio() {
  scripts/orch/gr target/release/graph-cli bench --n "$2" --seed 1 \
    --emit-scale-fixture "/w/$1" >/dev/null ||
    die "graph-cli could not emit $1 (build it: scripts/orch/gr cargo build --release -p graph-cli)"
}

# The densest studio document at most $1 bytes, as `<path> <nodes> <bytes>`: the size per node at
# 10 000 nodes, scaled, then shrunk 5% per try until the emitted file is under the limit.
# Caveat: shrink-only, so the graph is up to 5% under-dense in nodes against the true maximum,
# which is the direction that hides memory rather than inventing it.
studio_body() {
  local limit=$1 path=target/service-limits/$tag/body-studio.json per_node n size
  emit_studio target/service-limits/$tag/body-probe.json 10000
  per_node=$(($(stat -c %s target/service-limits/$tag/body-probe.json) / 10000))
  n=$((limit * 95 / 100 / per_node))
  for _ in 1 2 3 4; do
    emit_studio "$path" "$n"
    size=$(stat -c %s "$path")
    if ((size <= limit)); then break; fi
    n=$((n * 95 / 100))
  done
  printf '%s %s %s\n' "$path" "$n" "$size"
}

# One POST of $2 with source=$1. Sets `status` to the HTTP code, or 000 when the connection was
# refused or reset: a slot the kernel killed mid-request answers neither.
post() {
  local raw
  raw=$(curl -sS -o /dev/null -w '%{http_code}' -K <(printf 'header = "Authorization: Bearer %s"\n' "$key") \
    --data-binary "@$2" \
    "http://127.0.0.1:$port/v1/layout?layout=layout.circular.radial&post=post.style.orthogonal&source=$1") ||
    raw=000
  status=$raw
}

# n and m of the last request, from the server's own log line, as `<n> <m>`. The line is sorted by
# key, so `m` comes before `n` and neither is read out of the other's pattern.
logged_size() {
  local line n m
  line=$(docker logs --tail 1 "$name" 2>&1)
  n=$(sed -nE 's/.*"n":([0-9]+),.*/\1/p' <<<"$line")
  m=$(sed -nE 's/.*"m":([0-9]+),.*/\1/p' <<<"$line")
  printf '%s %s\n' "${n:-unknown}" "${m:-unknown}"
}

per_slot=$(slot_bytes PER_SLOT_BYTES)
base=$(slot_bytes BASE_BYTES)
budget=$(( (per_slot + base + MIB - 1) / MIB * MIB))
limit=${SERVICE_LIMITS_MEM:-$budget}
image=$(scripts/service.sh image) || die "no image: scripts/service.sh build"
mkdir -p "$work"
install -m 0640 /dev/null "$work/keys"
key=$(scripts/service.sh keygen svc-limits-gate "$work/keys") || die "keygen refused"
load_start=$(cut -d' ' -f1 /proc/loadavg)
log "image $image, M $budget B (per slot $per_slot + base $base, rounded up), limit $limit"

note "image $image"
note "M $budget bytes = per_slot $per_slot + base $base, rounded up to a whole MiB"
note "container limit $limit"
note "GRAPH_WORKERS=1, GRAPH_MAX_BODY default (64 MiB), GRAPH_AUTH on with a minted key"
note "load1 start $load_start ($(cat /proc/loadavg))"

scripts/orch/drun -d --name "$name" --memory "$limit" --memory-swap "$limit" \
  --read-only --cap-drop ALL --security-opt no-new-privileges \
  --group-add "$(stat -c %g "$work/keys")" -v "$(readlink -f "$work/keys"):/run/graph/keys:ro" \
  -e GRAPH_API_KEYS_FILE=/run/graph/keys -e GRAPH_WORKERS=1 \
  -p 127.0.0.1::8080 "$image" >/dev/null || die "drun refused the container"
await_listening 300
port=$(published_port)
[[ $port =~ ^[0-9]+$ ]] || die "the container published no port on 8080"
log "container $name on 127.0.0.1:$port"

records=$(contract_body $((64 * MIB)) "$work/body-contract.json") || die "no contract body"
contract_bytes=$(stat -c %s "$work/body-contract.json")
((contract_bytes <= 64 * MIB)) || die "the contract body is $contract_bytes bytes, past the 64 MiB limit"
note "contract body $work/body-contract.json $contract_bytes bytes, $records records"
read -r studio_path studio_n studio_bytes <<<"$(studio_body $((64 * MIB)))"
note "studio body $studio_path $studio_bytes bytes, $studio_n nodes"

post contract "$work/body-contract.json"
contract_status=$status
contract_size=$(logged_size)
check status-contract "$([[ $contract_status == 200 ]] && echo 0 || echo 1)" \
  "HTTP $contract_status, n m = ${contract_size:-unknown}"
post studio "$studio_path"
studio_status=$status
studio_size=$(logged_size)
check status-studio "$([[ $studio_status == 200 ]] && echo 0 || echo 1)" \
  "HTTP $studio_status, n m = ${studio_size:-unknown}"

peak=$(docker exec "$name" cat /sys/fs/cgroup/memory.peak 2>/dev/null) || peak=
state=$(docker inspect -f '{{.State.OOMKilled}} {{.State.Running}}' "$name" 2>/dev/null) || state="unknown unknown"
read -r oom running <<<"$state"
note "memory.peak ${peak:-unreadable} bytes (M $budget)"
check oom "$([[ $oom == false && $running == true ]] && echo 0 || echo 1)" "OOMKilled $oom, running $running"
check peak "$([[ -n $peak && $peak =~ ^[0-9]+$ && $peak -le $budget ]] && echo 0 || echo 1)" \
  "memory.peak ${peak:-unreadable} at most $budget"
note "load1 end $(cut -d' ' -f1 /proc/loadavg) ($(cat /proc/loadavg))"

log "$(grep -c '^FAIL' "$report" || true) failing checks in $report"
((fails == 0))
