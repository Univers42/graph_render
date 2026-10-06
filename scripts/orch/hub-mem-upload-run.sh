#!/usr/bin/env bash
# hub-mem-upload-run.sh — `scripts/orch/hub-mem.sh upload` (Decision 4, spec §5.3, N4).
#
# Builds and starts a real graph-server as the motor, starts the hub against it capped at 1 GiB,
# runs the client case that fills one workspace to GRAPH_HUB_MAX_DOC_BYTES, and reads the six
# `layout-upload` lines the relay wrote. The whole command, expanded, is the one
# `docs/measurements/hub-memory.md` records.
#
#   scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start \
#     && GRAPH_HUB_MOTOR_URL=http://<motor bridge ip>:8080 \
#        GRAPH_HUB_MAX_DOC_BYTES=67108864 GRAPH_HUB_MAX_RECORD_BYTES=1048576 \
#        HUB_MOTOR_KEY_FILE=target/hub-mem/motor-key HUB_RUN_MEMORY=1g \
#        scripts/orch/hub-run.sh start && scripts/orch/hub-mem.sh upload
#
# The input is the smallest record the contract admits, so the document holds as many records as the
# cap allows and the upload is the relay's worst case rather than its best (see
# server/graph-hub/tests/memory/upload.rs for the shape and the id width).
#
# Pass condition (spec §5.3, N4): the slowest of the five timed uploads finishes in under 8000 ms,
# two seconds under graph-server's GRAPH_BODY_TIMEOUT_MS default of 10 000, with no 408 in the
# motor's log and every Graph-Seq equal to the /graph ETag at the same cursor. A miss is a STOP
# (§5.3): target/hub-mem/upload.txt records the numbers and nothing is retuned.
#
# Exit: 0 the pass condition held · 1 it did not, and upload.txt names which part · 2 could not run
#
# Caveat: this is loopback between two containers on one host, so it measures the hub's own
# streaming cost and the socket between them, not a network. The knob a miss would name is the
# relay's chunk size, which upload.txt records beside the time.
# Caveat: GM_HUB_BREAK=throttle-upload (row negctl-throttle-upload) makes this verb fail on purpose,
# and its upload.txt then records a 408 at the motor and a 502 MotorBodyTimeout at the hub.

set -uo pipefail
here=$(dirname "$(readlink -f "$0")")
root=$(git -C "$here" rev-parse --show-toplevel)
cd "$root" || exit 2
# shellcheck source=scripts/orch/hub-mem-upload.sh
source "$here/hub-mem-upload.sh"

out=target/hub-mem
features=db-tests,negctl,test-hooks
case_name=upload::a_capped_workspace_uploads_five_times_inside_the_motor_body_timeout
# §5.3's pass condition: the slowest of the five, two seconds under graph-server's 10 s body timeout.
budget_ms=8000
cap_bytes=67108864
record_bytes=1048576

hub() { "$here/hub-run.sh" "$@"; }
client() {
  "$here/gr" -e GM_HUB_PG_URL="$("$here/hub-pg.sh" url)" \
    -e HUB_MEM_MAX_DOC_BYTES="$cap_bytes" cargo test --manifest-path server/Cargo.toml \
    -p graph-hub --features "$features" --test memory "$@"
}

# Every container this verb started, removed on every exit path including a signal.
cleanup() {
  hub reset >/dev/null 2>&1
  docker rm -f "$(motor_name)" >/dev/null 2>&1
  return 0
}
trap cleanup EXIT

# target/hub-mem/upload.txt, written on every path that gets as far as having numbers, so a failed
# run is readable rather than only its exit code.
report() {
  mkdir -p "$out" || return 1
  {
    printf 'command=%s\n' "$command"
    printf 'cap_bytes=%s max_record_bytes=%s budget_ms=%s\n' "$cap_bytes" "$record_bytes" "$budget_ms"
    printf 'input=%s\n' "${input_line:-input unknown}"
    printf 'upload_ms=%s\n' "${ms_line:-no uploads read}"
    printf 'median_upload_ms=%s slowest_upload_ms=%s\n' "${median_ms:-unknown}" "${slowest_ms:-unknown}"
    printf 'chunks=%s chunk_bytes=%s\n' "${chunks:-unknown}" "${chunk_bytes:-unknown}"
    # `graph_seq` is the ETag every run's `Graph-Seq` equalled, and `no-run-line` when the case
    # never reached its first `/layout`: the client case asserts the equality itself and panics on a
    # mismatch, so the presence of this line **is** the assertion having held.
    printf 'motor_408=%s graph_seq=%s\n' "${motor_408:-unknown}" "${seq_line:-no-run-line}"
    printf 'test_exit=%s verdict=%s\n' "${test_rc:-2}" "${verdict:-could not run}"
    printf 'failed=%s\n' "${why:-could not run}"
  } >"$out/upload.txt"
  cat "$out/upload.txt"
}

# 2 with the reason on stderr: could not run.
bail() { echo "hub-mem: upload: $*" >&2; report; exit 2; }

command='scripts/orch/hub-pg.sh reset && scripts/orch/hub-pg.sh start && GRAPH_HUB_MOTOR_URL=http://<motor>:8080 GRAPH_HUB_MAX_DOC_BYTES=67108864 GRAPH_HUB_MAX_RECORD_BYTES=1048576 HUB_RUN_MEMORY=1g scripts/orch/hub-run.sh start && scripts/orch/hub-mem.sh upload'

client --no-run >/dev/null 2>&1 || bail "the client case did not build"
"$root/scripts/service.sh" build >/dev/null || bail "scripts/service.sh build failed"
motor_credentials || bail "the motor key could not be minted"

# This worktree's database, from empty. The fill's idempotency keys are derived from the batch index
# and its records from the same ids, so a workspace left by an earlier run would answer 422 on the
# first batch — "the key was used before with a different request body" — and the run would read as
# an upload failure. The gate row resets the database too; doing it here as well makes the verb
# standalone, which is what `scripts/hub.sh upload-measurement` needs.
"$here/hub-pg.sh" reset >/dev/null || bail "the hub database did not reset"
"$here/hub-pg.sh" start >/dev/null || bail "the hub database did not start"

# The motor first: the hub refuses to start without a reachable GRAPH_HUB_MOTOR_URL, and a hub that
# starts against nothing answers 503 to every /layout, which would read as an upload failure.
SERVICE_PORT=0 SERVICE_DETACH="$(motor_name)" "$root/scripts/service.sh" run "$out/motor-keys" \
  >/dev/null || bail "the motor did not start"
ip=$(motor_ip)
[ -n "$ip" ] || bail "the motor has no bridge address"
await_motor "$ip" || bail "the motor never answered /healthz"

# The hub, at §6's cap and Decision 4's input size. GRAPH_HUB_MAX_RECORD_BYTES is the default's own
# value, named because the document's records are measured against it.
# The hub's whole environment for this run, exported once so `hub reset` and `hub start` agree:
# hub-run.sh fixes the environment when the container is created, and `start` reuses a container
# that exists, so a changed value needs the reset that precedes it.
export GRAPH_HUB_MOTOR_URL="http://$ip:8080"
export GRAPH_HUB_MAX_DOC_BYTES=$cap_bytes GRAPH_HUB_MAX_RECORD_BYTES=$record_bytes
export HUB_MOTOR_KEY_FILE="$root/$out/motor-key" HUB_RUN_MEMORY=1g
export GRAPH_HUB_DB_URL
GRAPH_HUB_DB_URL=$("$here/hub-pg.sh" url) || bail "the hub's database is not up"
hub reset >/dev/null 2>&1
hub start >/dev/null || bail "the hub did not start against the motor"

# The case: the fill, then one warm-up and five timed uploads, each asserted 200 with Graph-Seq
# equal to the /graph ETag. Its own stdout carries the two HUB_MEM upload lines.
run_log=$out/upload-case.log
client -- "$case_name" --exact --ignored --nocapture 2>&1 | tee "$run_log"
test_rc=${PIPESTATUS[0]}

input_line=$(sed -nE 's/^.*(HUB_MEM upload cap=.*)$/\1/p' "$run_log" | head -1)
seq_line=$(sed -nE 's/^.*(HUB_MEM upload etag=.*)$/\1/p' "$run_log" | head -1)
[ -n "$input_line" ] || input_line='input unknown (the case printed no HUB_MEM upload cap= line)'

# The six lines, then the five after the warm-up.
#
# A count other than five is a **verdict**, not a setup failure: row `negctl-throttle-upload` runs
# this verb with the throttled relay, where the motor's body timeout answers 408 before any upload
# finishes, so no upload reaches its tail and no line is written. That is the control working, and
# it has to exit 1 with the reason named rather than 2 "could not run" — measured: with the bail
# here, the control exited 2 and named nothing.
all_lines=$(upload_lines)
count=$(printf '%s\n' "$all_lines" | grep -c upload_ms || true)
timed=$(timed_ms)
n=$(printf '%s\n' "$timed" | grep -c '[0-9]' || true)
if [ "$n" = 5 ]; then
  ms_line=$(printf '%s' "$timed" | tr '\n' ' ')
  median_ms=$(printf '%s\n' "$timed" | median)
  slowest_ms=$(printf '%s\n' "$timed" | sort -n | tail -1)
else
  ms_line="$timed"
  median_ms=none
  slowest_ms=none
fi

# The chunk size Decision 4 names as the knob: the walk yields one record per chunk plus a head and a
# tail, so the chunks of one upload are `records + 2` and the mean chunk is `bytes / chunks`.
#
# Caveat: a mean and not the largest chunk. The ids are a fixed width (tests/memory/upload.rs), so
# every record is the same size and the mean is within a few bytes of the largest; a document with
# mixed-width ids would make this an average over a range, which is why the case does not write one.
records=$(printf '%s\n' "$all_lines" | sed -nE 's/.*"records":([0-9]+).*/\1/p' | tail -1)
bytes=$(printf '%s\n' "$all_lines" | sed -nE 's/.*"bytes":([0-9]+).*/\1/p' | tail -1)
if [ -n "$records" ] && [ -n "$bytes" ]; then
  chunks=$((records + 2))
  chunk_bytes=$((bytes / chunks))
else
  chunks=none
  chunk_bytes=none
fi

motor_408=$(motor_408s)

# The verdict, and which part failed. Every clause is named so a reader of upload.txt learns *which*
# condition broke rather than only that one did.
#
# The time clause is skipped when there is no time to judge: a run with no completed upload has
# already failed every other clause, and "slowest upload none ms" would read as a measurement rather
# than as its absence.
why=
[ "$test_rc" -eq 0 ] || why="the client case failed (exit $test_rc)"
if [ "$n" = 5 ]; then
  [ "$slowest_ms" -lt "$budget_ms" ] ||
    why="${why:+$why; }slowest upload ${slowest_ms} ms is not under ${budget_ms} ms"
else
  why="${why:+$why; }the hub's log holds $n completed uploads, not 5 (the relay writes a line only for an upload that reached its tail)"
fi
[ "$motor_408" = 0 ] || why="${why:+$why; }the motor logged $motor_408 408"
[ -n "$seq_line" ] || why="${why:+$why; }no Graph-Seq line: the case did not confirm every run against the /graph ETag"
[ "$count" = 6 ] || why="${why:+$why; }the hub's log holds $count layout-upload lines, not 6"
if [ -z "$why" ]; then
  verdict=pass
  rc=0
else
  verdict=fail
  rc=1
fi
report
exit "$rc"