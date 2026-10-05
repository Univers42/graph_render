#!/usr/bin/env bash
# hub-run.sh — the graph-hub container every container-level hub row runs against.
#
#   hub-run.sh reset | start | stop | kill | restart | url
#   hub-run.sh run CMD [ARGS...]      run CMD, serving its step requests until it exits
#   hub-run.sh ack-file STEP          print the path of STEP's acknowledgement file
#
# The container runs the image of scripts/hub.sh image (rebuilt incrementally on every start),
# through scripts/orch/drun only, read-only, with every capability dropped and no port published:
# a test in a scripts/orch/gr container reaches it at its bridge IP, written to target/hub-run/url
# on every start because a restarted container may get a new one. Its database is
# scripts/orch/hub-pg.sh's; start that first.
#
# Instance: one hub per worktree, named gm-hub-<worktree dir> (GM_HUB_RUN_NAME overrides).
# Credentials: target/hub-run/{keys,grants} at 0640 hold one key `tester` granted `* admin`; its
# plaintext is target/hub-run/key at 0600, read by the tests and never printed. A motor is mounted
# only when HUB_MOTOR_KEY_FILE names its key file (GRAPH_HUB_MOTOR_URL then names the motor).
# Every GRAPH_HUB_* variable set in the caller's environment passes through by name, never by
# value on a command line; GRAPH_HUB_DB_URL defaults to `hub-pg.sh url`.
# GM_HUB_BREAK=<name>[,<name>] runs a graph-hub built with --features negctl (target/hub-negctl),
# bind-mounted over the image's binary, with the break named in its environment.
#
# Step handshake (`run`): a test asks for a container action by writing one verb (kill, stop, start
# or restart) to target/hub-steps/<step>.req; the runner deletes the request, acts, and writes the
# verb's exit status to target/hub-steps/<step>.ack. The test waits for the ack, then re-reads
# target/hub-run/url.
#
# Exit: 0 the verb did what it says · 1 it could not (docker, the build or the hub failed) · 2 usage
#
# Caveat: the environment, the database URL and the break are fixed when the container is
# created, and `start` reuses an existing container; after a hub-pg.sh reset, or to change
# GM_HUB_BREAK or any GRAPH_HUB_* value, run `hub-run.sh reset` first.
# Caveat: `run` polls every 50 ms, so an action lands up to 50 ms after its request, and a request
# written after CMD has exited is never served (it is reported on stderr and deleted).

set -uo pipefail
here=$(dirname "$(readlink -f "$0")")
root=$(git -C "$here" rev-parse --show-toplevel)
cd "$root" || exit 2
drun=$here/drun
name=${GM_HUB_RUN_NAME:-gm-hub-$(basename "$root")}
state=target/hub-run
steps=target/hub-steps
negctl_bin=target/hub-negctl/release/graph-hub

die() { echo "hub-run: $*" >&2; exit 2; }
fail() { echo "hub-run: $*" >&2; return 1; }

# The key and its file line come from scripts/hub.sh keygen, so one implementation mints them.
credentials() {
  [ -s "$state/key" ] && [ -s "$state/keys" ] && [ -s "$state/grants" ] && return 0
  rm -rf "$state" && mkdir -p "$state" || return 1
  (umask 077 && "$root/scripts/hub.sh" keygen tester "$state/keys" >"$state/key") \
    || { fail "keygen failed"; return 1; }
  install -m 0640 /dev/null "$state/grants" || return 1
  printf 'tester * admin\n' >"$state/grants"
}

negctl_build() {
  "$here/gr" bash -c "cd server && CARGO_TARGET_DIR=/w/target/hub-negctl \
    cargo build --release --locked -p graph-hub --bin graph-hub --features negctl" >&2 \
    || fail "the negctl build failed"
}

ip() {
  docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' "$name" 2>/dev/null
}

url() {
  local addr
  addr=$(ip)
  [ -n "$addr" ] || { echo "hub-run: no running $name; run hub-run.sh start first" >&2; return 1; }
  printf 'http://%s:8080' "$addr" >"$state/url" || return 1
  cat "$state/url"
}

# Every argument here is a name, a path or a flag: no secret value reaches `ps`.
create() {
  local image args=() var
  image=$(cat target/hub-image/name) || return 1
  for var in $(compgen -e | grep '^GRAPH_HUB_'); do
    case $var in GRAPH_HUB_KEYS_FILE | GRAPH_HUB_GRANTS_FILE | GRAPH_HUB_MOTOR_KEY_FILE) ;;
    *) args+=(-e "$var") ;; esac
  done
  if [ -n "${HUB_MOTOR_KEY_FILE-}" ]; then
    [ -f "$HUB_MOTOR_KEY_FILE" ] || die "HUB_MOTOR_KEY_FILE is not a file"
    args+=(-v "$(readlink -f "$HUB_MOTOR_KEY_FILE"):/run/graph/motor-key:ro"
      -e GRAPH_HUB_MOTOR_KEY_FILE=/run/graph/motor-key)
  fi
  if [ -n "${GM_HUB_BREAK-}" ]; then
    args+=(-v "$root/$negctl_bin:/usr/local/bin/graph-hub:ro" -e GM_HUB_BREAK)
  fi
  # WHY --group-add: the image runs as 10001, and the credential files are 0640 for the host user,
  # which KeySet::load and Grants::load require; the host user's group is how 10001 reads them.
  "$drun" -d --name "$name" --read-only --cap-drop ALL --security-opt no-new-privileges \
    --group-add "$(id -g)" \
    -v "$root/$state/keys:/run/graph/keys:ro" -v "$root/$state/grants:/run/graph/grants:ro" \
    -e GRAPH_HUB_KEYS_FILE=/run/graph/keys -e GRAPH_HUB_GRANTS_FILE=/run/graph/grants \
    ${args[@]+"${args[@]}"} "$image" >/dev/null
}

# Caveat: 60 polls of half a second is a guess above a cold start on a loaded host; failing here
# means the hub never answered /healthz, and the redacted log tail on stderr says why.
wait_healthy() {
  local i
  for ((i = 0; i < 120; i++)); do
    docker exec "$name" /usr/local/bin/graph-hub healthcheck >/dev/null 2>&1 && return 0
    [ "$(docker inspect -f '{{.State.Running}}' "$name" 2>/dev/null)" = true ] || break
    sleep 0.5
  done
  echo "hub-run: $name did not become healthy; its log, URLs redacted:" >&2
  docker logs --tail 20 "$name" 2>&1 | sed -E 's#[a-z]+://[^ "]*#<url>#g' >&2
  return 1
}

start() {
  "$root/scripts/hub.sh" image >/dev/null || { fail "scripts/hub.sh image failed"; return 1; }
  credentials || return 1
  if [ -n "${GM_HUB_BREAK-}" ]; then negctl_build || return 1; fi
  boot
}

# The container half of start, without the builds: a step request lands here, so a test's kill and
# restart cost a container start, not a cargo run.
boot() {
  if docker inspect "$name" >/dev/null 2>&1; then
    docker start "$name" >/dev/null || return 1
  else
    if [ -z "${GRAPH_HUB_DB_URL-}" ]; then
      GRAPH_HUB_DB_URL=$("$here/hub-pg.sh" url) || { fail "no hub database; run hub-pg.sh start"; return 1; }
      export GRAPH_HUB_DB_URL
    fi
    create || return 1
  fi
  wait_healthy || return 1
  url >/dev/null
}

# SIGTERM, then the drain: the hub has GRAPH_HUB_TIMEOUT_MS (default 30 s) to finish what it
# admitted, so docker waits longer than that before its SIGKILL.
stop() { docker stop -t "${HUB_RUN_STOP_SECS:-40}" "$name" >/dev/null 2>&1; return 0; }
kill9() { docker kill "$name" >/dev/null 2>&1; return 0; }
restart() { stop && start; }
reset() {
  docker rm -f "$name" >/dev/null 2>&1
  rm -rf "$state" "$steps"
}

act() {
  case $1 in
  kill) kill9 ;;
  stop) stop ;;
  start) boot ;;
  restart) stop && boot ;;
  *) echo "hub-run: step request names no verb: $1" >&2; return 2 ;;
  esac
}

# One pass over the pending requests: each is deleted before it is served, so it runs once.
serve_requests() {
  local req verb rc
  for req in "$steps"/*.req; do
    [ -e "$req" ] || continue
    verb=$(tr -d '[:space:]' <"$req")
    rm -f "$req"
    act "$verb"
    rc=$?
    printf '%s\n' "$rc" >"${req%.req}.ack.tmp" && mv "${req%.req}.ack.tmp" "${req%.req}.ack"
  done
}

# WHY the step directory is made here, by the host user: the test runs as root in its container,
# and a directory it made would refuse the runner's acks.
run() {
  [ $# -gt 0 ] || die "run needs a command"
  rm -rf "$steps" && mkdir -p "$steps" || return 1
  "$@" &
  local pid=$! rc req
  while kill -0 "$pid" 2>/dev/null; do
    serve_requests
    sleep 0.05
  done
  wait "$pid"
  rc=$?
  for req in "$steps"/*.req; do
    [ -e "$req" ] || continue
    echo "hub-run: $req arrived after the command exited; not served" >&2
    rm -f "$req"
  done
  return "$rc"
}

case "${1-}" in
reset) reset ;;
start) start ;;
stop) stop ;;
kill) kill9 ;;
restart) restart ;;
url) url ;;
run) shift; run "$@" ;;
ack-file) [ $# -eq 2 ] || die "ack-file needs a step name"; printf '%s\n' "$root/$steps/$2.ack" ;;
--help | -h) sed -n '2,/^$/p' "$0" | sed -e 's/^# \{0,1\}//' -e '/^$/d'; exit 0 ;;
*) die "unknown verb: ${1-}" ;;
esac
