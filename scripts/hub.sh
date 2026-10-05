#!/usr/bin/env bash
# hub.sh — graph-motor's hub as one Docker image: graph-hub and nothing else (docs/deploy/hub.md).
#
#   scripts/hub.sh image                     build the image graph-hub:<tag> from the staged binary
#   scripts/hub.sh run                       run the last build on 127.0.0.1:${HUB_PORT:-8080},
#                                            the three credential files mounted read-only
#   scripts/hub.sh keygen NAME [KEYFILE]      a hub key on stdout; its file line appended to
#                                            KEYFILE at mode 0640, or printed on stderr without one
#   scripts/hub.sh upload-measurement        measure the /layout upload (Task 10 of the plan)
#   scripts/hub.sh test [cargo test args...]  cargo test -p graph-hub through scripts/orch/gr
#
# image stages target/hub/release/graph-hub into target/hub-stage/bin/, the image's whole build
# context, and tags it graph-hub:<tag>, where <tag> is the first 16 hex of sha256 over `sha256sum`
# of every staged file sorted by path (LC_ALL=C) — scripts/service.sh:44-46 does the same over its
# stage. target/hub-image/name records the tag for run. Every cargo invocation is scripts/orch/gr,
# in ge-rust; a bare cargo never runs here.
#
# run needs three files: HUB_KEYS_FILE and HUB_GRANTS_FILE (the hub's own credentials) and
# HUB_MOTOR_KEY_FILE (the plaintext motor key, one line). Each is bind-mounted read-only and named
# to the process as GRAPH_HUB_KEYS_FILE / GRAPH_HUB_GRANTS_FILE / GRAPH_HUB_MOTOR_KEY_FILE.
# GRAPH_HUB_DB_URL and GRAPH_HUB_MOTOR_URL are secrets with a password in them: they pass from the
# environment through -e, never as a command-line argument, and are never printed here.
#
# HUB_IMAGE_BREAK=bin scripts/hub.sh image   negative control for gate row negctl-hub-image: the
#                                            build asks cargo for --bin graph-server, so no graph-hub
#                                            is staged, nothing is built, and the script exits 1.
#
# Caveat: `test` needs a database (scripts/orch/hub-pg.sh start) and `upload-measurement` needs two
# (the hub's and the motor's); upload-measurement refuses with exit 2 until Task 10 lands it.
#
# Exit: 0 done · 1 a step failed · 2 misuse, a missing file, or a verb not landed yet.
set -euo pipefail

here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
root=$(git -C "$here" rev-parse --show-toplevel)
cd "$root"
stage=target/hub-stage
record=target/hub-image/name
mkdir -p target/hub-image

log() { printf '\033[1m[hub]\033[0m %s\n' "$*" >&2; }
die() { log "$*"; exit 2; }
usage() { sed -n '2,31p' "${BASH_SOURCE[0]}" >&2; exit 2; }

# The same hash as scripts/service.sh:44-46: sorted by path, so the order is fixed.
content_hash() {
  (cd "$1" && find . -type f -print0 | LC_ALL=C sort -z | xargs -0 sha256sum) | sha256sum | cut -c1-16
}

image_name() {
  [[ -s $record ]] || die "no build recorded: run scripts/hub.sh image first"
  cat "$record"
}

# The bin argument is the only thing the break moves, so the break cannot hide in a path.
# WHY `-p graph-hub`: `default-members` is `["graph-server"]` (server/Cargo.toml), so a build in
# `server/` with only `--bin` resolves against graph-server and reports "no bin target named
# graph-hub in default-run packages". The package must be named.
build_bin() {
  local bin=$1
  scripts/orch/gr bash -c \
    "cd server && CARGO_TARGET_DIR=/w/target/hub cargo build --release --locked -p graph-hub --bin $bin"
}

image() {
  local image
  # Exists only for gate row negctl-hub-image, which needs the failure to name the wrong bin: it
  # must stage no graph-hub and write target/hub-image/negctl-bin.log holding graph-server.
  if [[ ${HUB_IMAGE_BREAK-} == bin ]]; then
    log "HUB_IMAGE_BREAK=bin: building --bin graph-server, so target/hub-stage/bin/graph-hub is never staged"
    rm -rf "$stage"
    mkdir -p "$stage"
    rc=0
    # The break's own line goes into the log too, so the log names the wrong bin even when cargo
    # prints nothing but "Finished".
    {
      log "hub: cargo build --release --locked --bin graph-server"
      build_bin graph-server
    } 2>&1 | tee target/hub-image/negctl-bin.log || rc=$?
    log "HUB_IMAGE_BREAK=bin: cargo exited ${rc}; no image built (negctl-hub-image)"
    exit 1
  fi
  log "graph-hub, release, in ge-rust"
  build_bin graph-hub >&2
  rm -rf "$stage"
  mkdir -p "$stage/bin"
  cp target/hub/release/graph-hub "$stage/bin/graph-hub"
  image=graph-hub:$(content_hash "$stage")
  log "docker build $image"
  docker build -q -f deploy/hub.Dockerfile -t "$image" "$stage" >/dev/null
  printf '%s\n' "$image" >"$record"
  log "$image: $(docker image inspect -f '{{.Size}}' "$image") bytes"
  printf '%s\n' "$image"
}

# run: the three credential files are required — a hub with no keys, or with keys and no grants,
# answers 401 or 403 to everything and reads as a bug rather than a deployment mistake (§5.2).
run() {
  local image name
  for name in HUB_KEYS_FILE HUB_GRANTS_FILE HUB_MOTOR_KEY_FILE; do
    [[ -n ${!name-} ]] || die "$name is unset: the hub needs keys, grants and the motor key"
    [[ -f ${!name} ]] || die "$name is not a file: ${!name}"
  done
  image=$(image_name)
  docker image inspect "$image" >/dev/null 2>&1 \
    || die "no image $image: run scripts/hub.sh image first"
  scripts/orch/drun --rm --read-only --cap-drop ALL --security-opt no-new-privileges \
    -v "$(readlink -f "$HUB_KEYS_FILE"):/run/graph/keys:ro" \
    -v "$(readlink -f "$HUB_GRANTS_FILE"):/run/graph/grants:ro" \
    -v "$(readlink -f "$HUB_MOTOR_KEY_FILE"):/run/graph/motor-key:ro" \
    -e GRAPH_HUB_KEYS_FILE=/run/graph/keys \
    -e GRAPH_HUB_GRANTS_FILE=/run/graph/grants \
    -e GRAPH_HUB_MOTOR_KEY_FILE=/run/graph/motor-key \
    -e GRAPH_HUB_DB_URL -e GRAPH_HUB_MOTOR_URL \
    -p "127.0.0.1:${HUB_PORT:-8080}:8080" "$image"
}

# Caveat: the key and its file line are graph-server's format (server/graph-server/src/keys.rs:165-181),
# minted by graph-server's own keygen, so one implementation mints every key in the fleet.
keygen() {
  local name=${1-} keys=${2-} line
  [[ $name =~ ^[A-Za-z0-9._-]+$ ]] || usage
  # stdout (the key) goes straight through; only stderr (the file line) is captured.
  { line=$(scripts/orch/gr bash -c \
      "cd server && CARGO_TARGET_DIR=/w/target/server cargo run --quiet --release --locked -p graph-server --bin graph-server -- keygen $name" 2>&1 >&3 3>&-); } 3>&1
  line=$(grep -E "^$name [0-9a-f]{64}$" <<<"$line") || {
    log "keygen printed no file line"
    exit 1
  }
  if [[ -z $keys ]]; then
    printf '%s\n' "$line" >&2
    return
  fi
  [[ -e $keys ]] || install -m 0640 /dev/null "$keys"
  printf '%s\n' "$line" >>"$keys"
  log "appended the line for $name to $keys"
}

test_hub() {
  local url
  url=$(scripts/orch/hub-pg.sh url) || die "no hub database: run scripts/orch/hub-pg.sh start first"
  scripts/orch/gr -e GRAPH_HUB_DB_URL="$url" cargo test --manifest-path server/Cargo.toml \
    -p graph-hub "$@"
}

# The measurement row of docs/measurements/hub-memory.md lands with Task 10 of the plan: it needs
# the hub's database and a motor, and until then there is no honest thing to run here.
upload_measurement() {
  die "upload-measurement lands with the memory row of Task 10 (docs/measurements/hub-memory.md); use scripts/orch/hub-mem.sh upload"
}

case ${1-} in
image) image ;;
run) run "${@:2}" ;;
keygen) keygen "${2-}" "${3-}" ;;
test) shift || true; test_hub "$@" ;;
upload-measurement) upload_measurement "${@:2}" ;;
--help | -h) sed -n '2,31p' "${BASH_SOURCE[0]}" ;;
*) usage ;;
esac