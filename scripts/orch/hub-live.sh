#!/usr/bin/env bash
# hub-live.sh — one fresh hub (and optionally a motor) for the SDK's LIVE rows, then CMD.
#
#   hub-live.sh [--motor] CMD [ARGS...]
#
# Resets this worktree's hub database (scripts/orch/hub-pg.sh) and hub container
# (scripts/orch/hub-run.sh), starts both, mints a second key `writer` granted only
# `* write:rows-file` beside hub-run's `tester` (`* admin`), and runs CMD under `hub-run.sh run`.
# The writer's plaintext is target/hub-run/writer-key at 0600, read by the tests and never printed;
# hub-run's own key stays target/hub-run/key. `--motor` also starts the graph-motor container
# (scripts/orch/hub-mem-upload.sh's helpers, key material under target/hub-live/) and points the
# hub at it, which is what `/layout` needs. Every container is removed on exit, pass or fail.
#
# Exit: CMD's own status · 1 a container could not be started · 2 usage
#
# Caveat: the database is reset on every call, so two calls in one worktree never share state and
# a row cannot depend on a previous row's records; that is the point, and it costs one PostgreSQL
# start (seconds) per row.
# Caveat: the writer is added by stopping the hub and starting it again, because the keys and
# grants files are read at start (and on SIGHUP); a hub that fails to restart is exit 1, not a
# silent run under the old grants.
set -uo pipefail
here=$(dirname "$(readlink -f "$0")")
root=$(git -C "$here" rev-parse --show-toplevel) || exit 2
cd "$root" || exit 2
out=target/hub-live
# shellcheck source=scripts/orch/hub-mem-upload.sh
source "$here/hub-mem-upload.sh"

motor=0
if [[ ${1-} == --motor ]]; then
  motor=1
  shift
fi
(($#)) || {
  echo "usage: hub-live.sh [--motor] CMD [ARGS...]" >&2
  exit 2
}

cleanup() {
  "$here/hub-run.sh" reset >/dev/null 2>&1
  docker rm -f "$(motor_name)" >/dev/null 2>&1
}
trap cleanup EXIT

fail() {
  echo "hub-live: $*" >&2
  exit 1
}

start_motor() {
  scripts/hub.sh image >/dev/null || return 1
  scripts/service.sh build >/dev/null || return 1
  motor_credentials || return 1
  docker rm -f "$(motor_name)" >/dev/null 2>&1
  SERVICE_PORT=0 SERVICE_DETACH="$(motor_name)" scripts/service.sh run "$out/motor-keys" >/dev/null || return 1
  local ip
  ip=$(motor_ip)
  await_motor "$ip" || return 1
  export GRAPH_HUB_MOTOR_URL="http://$ip:8080" HUB_MOTOR_KEY_FILE="$root/$out/motor-key"
}

# The writer's key goes into the files hub-run mounted, under umask 077 so its plaintext is 0600.
add_writer() {
  "$here/hub-run.sh" stop || return 1
  (umask 077 && scripts/hub.sh keygen writer target/hub-run/keys >target/hub-run/writer-key) || return 1
  [[ -s target/hub-run/writer-key ]] || return 1
  printf 'writer * write:rows-file\n' >>target/hub-run/grants
  "$here/hub-run.sh" start >/dev/null
}

"$here/hub-pg.sh" reset >/dev/null || fail "the hub database could not be reset"
"$here/hub-pg.sh" start >/dev/null || fail "the hub database did not start"
if ((motor)); then
  start_motor || fail "the motor did not start"
fi
GRAPH_HUB_DB_URL=$("$here/hub-pg.sh" url) || fail "no hub database url"
export GRAPH_HUB_DB_URL
"$here/hub-run.sh" reset >/dev/null 2>&1
"$here/hub-run.sh" start >/dev/null || fail "the hub did not start"
add_writer || fail "the writer key could not be added"
"$here/hub-run.sh" run "$@"
