#!/usr/bin/env bash
# service-image.sh — the svc-image gate: the image scripts/service.sh builds, run as it ships and
# probed from outside (deploy/nav/service.py), with the embed loaded the one supported way, through a
# host page that reverse-proxies /embed/ (docs/deploy/service.md, docs/contract/service-api.md C10-C11).
#
#   scripts/service-image.sh                          the gate
#   SERVICE_IMAGE_BREAK=headers scripts/service-image.sh   negctl: a pass-through drops COOP, COEP and
#                                                     CORP in front of the service; must fail
#   SERVICE_IMAGE_BREAK=leak scripts/service-image.sh      negctl: the scanned image is rebuilt with an
#                                                     embed/.env and a .git; must fail
#   SERVICE_IMAGE_CHECK=sdk scripts/service-image.sh       the SDK's live-check.mjs against the image
#   SERVICE_IMAGE_CHECK=sdk SERVICE_IMAGE_BREAK=key ...    negctl: live-check gets a key the server made
#                                                     but the key file does not hold; must fail
#
# Steps: build; list the image's files for leaks; make a key with the image's own keygen into a
# 0640 temp file; run the container detached (scripts/service.sh run); `docker exec id -u`; wait
# for the HEALTHCHECK; then the probe in gm-chromium on the container's network namespace:
# /healthz, /v1/meta with and without the key, the embed headers, and three browser pages (proxied
# isolated, proxied plain, and the direct cross-origin load documented to fail). The key reaches the
# probe through the environment only and is never printed. The sdk check replaces the scan and the
# probe with crates/graph-sdk-js/scripts/live-check.mjs (meta, parity with the local wasm build, the
# typed 401 and 400) in node, and exits 2 where the tree has no live-check.mjs (svc-sdk-remote).
#
# Report: target/service-image/<label>/{report.json,table.md,leaks.txt,*.png} or sdk-live.txt
# Exit: 0 every row PASS · 1 a row FAIL or NOT-RUN · 2 could not run.
set -euo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$root"
source scripts/orch/scratch.sh
check=${SERVICE_IMAGE_CHECK:-probe}
broken=${SERVICE_IMAGE_BREAK:-}
[[ $check:$broken =~ ^(probe:(|headers|leak)|sdk:(|key))$ ]] || {
  echo "service-image: SERVICE_IMAGE_CHECK=probe takes BREAK headers or leak; sdk takes key" >&2
  exit 2
}
label=${SERVICE_IMAGE_LABEL:-${broken:+negctl-$broken}}
out=target/service-image/${label:-$check}
live_check=crates/graph-sdk-js/scripts/live-check.mjs
name=svc-image-$$
flags=()
[[ $broken == headers ]] && flags=(--break)
work=$(mktemp -d)
# Caveat: a fixed cap; a host so loaded that the first probe takes longer reads as unhealthy and
# fails the row, never passes it.
HEALTH_CAP_S=60

log() { printf '\033[1m[service-image]\033[0m %s\n' "$*" >&2; }
cleanup() {
  docker rm -f "$name" >/dev/null 2>&1 || true
  rm -rf "$work"
}
trap cleanup EXIT

# The image's file names that would leak a secret or the build host, and the build host's own paths
# found inside any file. Caveat: by name and by those two strings only; a key pasted into an
# allowed file under another name is not found, so this proves the context stays the stage, not
# that no file holds a secret.
leaks() {
  local box
  box=$(docker create "$1")
  docker export "$box" | tar -t |
    grep -E '(^|/)(\.env[^/]*|\.git|[^/]*keys?|[^/]*\.(key|pem|p12|pfx))(/|$)|goinfre|^(home|tmp)/.+' || true
  docker export "$box" | grep -a -o -F -m 1 -e "$GM_SCRATCH" -e "$root" | sort -u | sed 's/^/contents: /' || true
  docker rm -f "$box" >/dev/null
}

# The leak negctl's image: the real stage plus what a careless context would carry in.
leaked_image() {
  local stage=$work/stage v
  cp -a target/service/stage "$stage"
  v=$(<"$stage/embed/VERSION")
  printf 'GRAPH_API_KEYS_FILE=/run/graph/keys\n' >"$stage/embed/.env"
  mkdir -p "$stage/embed/$v/.git" && printf 'ref: refs/heads/develop\n' >"$stage/embed/$v/.git/HEAD"
  docker build -q -f deploy/service.Dockerfile -t "graph-motor:leak-$$" "$stage" >/dev/null
  printf 'graph-motor:leak-%s\n' "$$"
}

scan() {
  local scanned=$1
  [[ $broken == leak ]] && scanned=$(leaked_image)
  leaks "$scanned" >"$out/leaks.txt"
  [[ $scanned != "$1" ]] && docker image rm -f "$scanned" >/dev/null
  return 0
}

wait_healthy() {
  local health=starting deadline=$((SECONDS + HEALTH_CAP_S))
  while [[ $health == starting && $SECONDS -lt $deadline ]]; do
    sleep 1
    health=$(docker inspect -f '{{if .State.Health}}{{.State.Health.Status}}{{else}}none{{end}}' "$name")
  done
  printf '%s\n' "$health"
}

probe() {
  local version=$1 uid=$2 health=$3 key=$4
  GRAPH_TEST_KEY=$key scripts/orch/drun --rm --network "container:$name" -e GRAPH_TEST_KEY \
    --memory 4g --memory-swap 4g -v "$root:/w" -w /w/deploy/nav gm-chromium \
    python3 service.py --image "$image" --version "$version" --uid "$uid" --health "$health" \
    --leaks "/w/$out/leaks.txt" --out "/w/$out" --commit "$(git rev-parse --short HEAD)" "${flags[@]}"
}

# The stranger key comes from the server's own keygen with no file, so it has the right format
# and no line in the key file: the authorised checks see a 401.
sdk_live() {
  local key=$1
  if [[ $broken == key ]]; then key=$(scripts/service.sh keygen svc-image-stranger 2>/dev/null) || exit 2; fi
  GRAPH_API_KEY=$key scripts/orch/drun --rm --network "container:$name" -e GRAPH_API_KEY \
    -v "$root:/w" -w /w "$GM_NODE_IMAGE" node --experimental-strip-types \
    "$live_check" http://127.0.0.1:8080 | tee "$out/sdk-live.txt"
}

can_run() {
  if [[ $check == probe ]]; then
    ensure_image gm-chromium
  elif [[ ! -f $live_check ]]; then
    log "no $live_check in this tree: it lands with svc-sdk-remote"
    return 1
  fi
}

main() {
  local version key uid health
  source scripts/orch/image.sh
  can_run || exit 2
  # The negctl rows read this report to tell their failure from any other, so a run that stops
  # early must leave none from an earlier run behind.
  rm -rf "$out" && mkdir -p "$out"
  image=$(scripts/service.sh build) || exit 1
  version=$(scripts/service.sh version)
  [[ $check == probe ]] && scan "$image"
  key=$(scripts/service.sh keygen svc-image-gate "$work/keys") || exit 2
  SERVICE_PORT=0 SERVICE_DETACH=$name scripts/service.sh run "$work/keys" >/dev/null || exit 2
  uid=$(docker exec "$name" id -u) || uid=""
  health=$(wait_healthy)
  log "$image (embed $version): uid ${uid:-unknown}, health $health"
  set +e
  if [[ $check == sdk ]]; then
    sdk_live "$key"
  else
    probe "$version" "$uid" "$health" "$key"
  fi
}

main
