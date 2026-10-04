#!/usr/bin/env bash
# service.sh — graph-motor as one Docker image: graph-server and the embed bundle (docs/deploy/service.md).
#
#   scripts/service.sh build              stage the artifacts and build the image graph-motor:<tag>
#   scripts/service.sh run KEYFILE        run the last build on 127.0.0.1:8080, KEYFILE read-only
#   scripts/service.sh keygen NAME [KEYFILE]   a new key on stdout; its file line appended to
#                                         KEYFILE, or printed on stderr without one
#   scripts/service.sh version            the embed version of the last build
#   scripts/service.sh image              the image name of the last build
#
# build stages into target/service/stage, the image's whole build context:
#   bin/graph-server          release, built in ge-rust (scripts/orch/gr) from server/
#   embed/<version>/          the bundle and both wasm builds (scripts/studio.sh embed)
#   embed/VERSION             <version> and a newline; the server serves /embed/<version>/ only
# <version> is the content hash of the bundle, and <tag> the same hash over the whole stage:
# the first 16 hex of sha256 over `sha256sum` of every file, sorted by path (LC_ALL=C). So the
# year-long immutable cache of /embed/<version>/ never holds two bundles under one name, and the
# same bytes keep their URL across commits. target/service/image records the name for run.
#
# run: SERVICE_PORT picks the host port (default 8080, 0 = any free one), SERVICE_DETACH=<name>
# runs it in the background under that container name. It starts through scripts/orch/drun (cap
# DRUN_MEM, default 8g here: one worker slot is 4.32 GiB, so 4g holds none and the server refuses
# to start, docs/measurements/service-caps.md "Memory per slot"). The root filesystem is read-only, every capability is dropped, and the
# port is published on the loopback only. KEYFILE holds
# `<name> <sha256-hex>` lines, never a key; the process runs as uid 10001 and reads it through
# the file's group, so the file needs g+r (0640); group- or world-writable is refused (C9).
#
# Exit: 0 done · 1 a step failed · 2 misuse, or the image is missing.
set -euo pipefail

here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
root=$(git -C "$here" rev-parse --show-toplevel)
cd "$root"
stage=target/service/stage
record=target/service/image
export CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-4}

log() { printf '\033[1m[service]\033[0m %s\n' "$*" >&2; }
usage() {
  sed -n '2,27p' "${BASH_SOURCE[0]}" >&2
  exit 2
}

content_hash() {
  (cd "$1" && find . -type f -print0 | LC_ALL=C sort -z | xargs -0 sha256sum) | sha256sum | cut -c1-16
}

last_build() {
  [[ -s $record && -s $stage/embed/VERSION ]] && return
  log "no build recorded: run scripts/service.sh build first"
  exit 2
}

version() {
  last_build
  cat "$stage/embed/VERSION"
}

image_of() {
  local image
  last_build
  image=$(cat "$record")
  docker image inspect "$image" >/dev/null 2>&1 || {
    log "no image $image: run scripts/service.sh build"
    exit 2
  }
  printf '%s\n' "$image"
}

build_server() {
  log "graph-server, release, in ge-rust"
  # Its own target dir: the path is this script's to know, whatever server/ configures.
  scripts/orch/gr bash -c 'cd server && CARGO_TARGET_DIR=/w/target/server cargo build --release --locked --bin graph-server'
  mkdir -p "$stage/bin"
  cp target/server/release/graph-server "$stage/bin/graph-server"
}

stage_embed() {
  local bundle=target/service/bundle v
  rm -rf "$bundle" "$stage" "$record"
  scripts/studio.sh embed "$bundle" >&2
  v=$(content_hash "$bundle")
  mkdir -p "$stage/embed"
  mv "$bundle" "$stage/embed/$v"
  printf '%s\n' "$v" >"$stage/embed/VERSION"
}

build() {
  local image
  stage_embed
  build_server >&2
  image=graph-motor:$(content_hash "$stage")
  log "docker build $image"
  docker build -q -f deploy/service.Dockerfile -t "$image" \
    --label "org.opencontainers.image.version=$(cat "$stage/embed/VERSION")" \
    --label "org.opencontainers.image.revision=$(git rev-parse HEAD)" "$stage" >/dev/null
  printf '%s\n' "$image" >"$record"
  log "$image: $(docker image inspect -f '{{.Size}}' "$image") bytes, embed $(version)"
  printf '%s\n' "$image"
}

run() {
  local keys image detach=(--rm)
  [[ -f ${1-} ]] || usage
  keys=$(readlink -f "$1")
  image=$(image_of)
  [[ -n ${SERVICE_DETACH-} ]] && detach=(-d --name "$SERVICE_DETACH")
  DRUN_MEM=${DRUN_MEM:-8g} scripts/orch/drun "${detach[@]}" --read-only --cap-drop ALL --security-opt no-new-privileges \
    --group-add "$(stat -c %g "$keys")" -v "$keys:/run/graph/keys:ro" -e GRAPH_API_KEYS_FILE=/run/graph/keys \
    -p "127.0.0.1:${SERVICE_PORT:-8080}:8080" "$image"
}

# The server makes the key, so the key format and its hash exist once (docs/contract/service-api.md).
keygen() {
  local name=${1-} keys=${2-} image line
  [[ $name =~ ^[A-Za-z0-9._-]+$ ]] || usage
  image=$(image_of)
  # stdout (the key) goes straight through; only stderr (the file line) is captured.
  { line=$(scripts/orch/drun --rm --network none "$image" keygen "$name" 2>&1 >&3 3>&-); } 3>&1
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

case ${1-} in
build) build ;;
run) run "${2-}" ;;
keygen) keygen "${2-}" "${3-}" ;;
version) version ;;
image) image_of ;;
*) usage ;;
esac
