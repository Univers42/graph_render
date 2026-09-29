#!/usr/bin/env bash
# graph-motor studio — build the wasm motor, stage the assets the app serves, and
# run it in the node:22-slim container.
#
#   scripts/studio.sh            dev server on 0.0.0.0:5173 (the usual case)
#   scripts/studio.sh build      production build into app/dist
#   scripts/studio.sh test       the app's node:test unit tests
#   scripts/studio.sh check      typecheck + tests + production build
#   scripts/studio.sh serve      dev server, skipping the cargo build (reuse the
#                                graph_wasm.wasm already in app/public/)
#
# Two toolchains, both in Docker, never on the host: the wasm build goes through
# the Rust helper (`gr`, which also carries the 8g memory cap and the cargo
# registry volume), and npm/node go through `node:22-slim` the way
# orch/bin/node-slim.sh does — with `-p 5173:5173` added, which is the only
# difference from that helper, since a studio you cannot open is not a studio.
set -euo pipefail

here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
root=$(git -C "$here" rev-parse --show-toplevel 2>/dev/null || echo "$here/..")
root=$(cd -- "$root" && pwd)
gr=${GR:-/goinfre/dlesieur/orch/bin/gr}
node_image=${NODE_IMAGE:-node:22-slim}
port=${STUDIO_PORT:-5173}
command=${1:-serve}

export CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-4}
export RUST_TEST_THREADS=${RUST_TEST_THREADS:-4}

log() { printf '\033[1m[studio]\033[0m %s\n' "$*"; }

# The wasm module the SDK loads, straight from the release build.
build_wasm() {
  log "building graph-wasm (wasm32-unknown-unknown, release)"
  "$gr" cargo build -p graph-wasm --target wasm32-unknown-unknown --release
}

# Everything the browser cannot reach on its own: the wasm module, and the
# engine's fixtures (the studio lists them; Vite serves whatever is in public/).
stage_assets() {
  local wasm="$root/target/wasm32-unknown-unknown/release/graph_wasm.wasm"
  if [[ ! -f "$wasm" ]]; then
    log "MISSING $wasm — run scripts/studio.sh without 'serve' to build it"
    return 1
  fi
  mkdir -p "$root/app/public"
  cp "$wasm" "$root/app/public/graph_wasm.wasm"
  log "staged app/public/graph_wasm.wasm ($(wc -c <"$wasm") bytes)"
  rm -rf "$root/app/public/fixtures"
  cp -R "$root/fixtures" "$root/app/public/fixtures"
  log "staged app/public/fixtures"
}

# node:22-slim with the repo at /w (orch/bin/node-slim.sh's mount), plus the
# port. The in-container shell is bash, not the image's default sh: `install_deps`
# is exported from this script and uses bash syntax.
# Published only for the dev server: a second `check` run must not fail because
# a studio is already up on 5173.
publish=()

in_node() {
  # `-t` only when there is a terminal to attach to: `docker run -it` refuses
  # outright otherwise, which would break `scripts/studio.sh build` from a script.
  local tty=()
  [[ -t 0 && -t 1 ]] && tty=(-it)
  docker run --rm "${tty[@]}" \
    -v "$root:/w" -w "/w/app" "${publish[@]}" "$node_image" "$@"
}

# `npm ci` when there is a lockfile (reproducible), `npm install` when there is
# not (a fresh checkout of the studio alone). Checked INSIDE the container, where
# the repo is actually mounted, rather than against a host path.
install_deps() {
  if [ -f package-lock.json ]; then
    npm ci --no-audit --no-fund
  else
    log "no package-lock.json — installing instead of ci"
    npm install --no-audit --no-fund
  fi
}

case "$command" in
  serve)
    stage_assets
    publish=(-p "$port:$port")
    log "dev server on http://localhost:$port"
    in_node bash -c "set -e
$(declare -f log install_deps)
install_deps
npx vite --host 0.0.0.0 --port $port"
    ;;
  build)
    stage_assets
    log "production build into app/dist"
    in_node bash -c "$(declare -f log install_deps); install_deps; npx vite build"
    ;;
  test)
    log "unit tests (node --test --experimental-strip-types)"
    in_node bash -c 'node --test --experimental-strip-types tests/*.test.ts'
    ;;
  check)
    log "typecheck"
    in_node bash -c 'npx tsc --noEmit -p tsconfig.json'
    stage_assets
    log "unit tests"
    in_node bash -c 'node --test --experimental-strip-types tests/*.test.ts'
    log "render smoke test (six geometry kinds, stubbed DOM)"
    in_node bash -c 'node --experimental-strip-types --experimental-loader /w/tests/ts-extension-loader.mjs tests/paint.dom.ts'
    log "live motor check (real wasm, every registered layout)"
    in_node bash -c 'node --experimental-strip-types --experimental-loader /w/tests/ts-extension-loader.mjs scripts/verify-motor.ts'
    log "production build"
    in_node bash -c "$(declare -f log install_deps); install_deps; npx vite build"
    log "ok"
    ;;
  *)
    echo "usage: scripts/studio.sh [serve|build|test|check]" >&2
    exit 2
    ;;
esac
