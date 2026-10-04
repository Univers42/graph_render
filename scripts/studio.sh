#!/usr/bin/env bash
# studio.sh — the standalone studio (<graph-studio> in app/), in Docker, never on the host.
#
#   scripts/studio.sh [serve]   dev server on http://127.0.0.1:5174, over a freshly built wasm
#   scripts/studio.sh wasm      build graph-wasm (release) and stage it with the fixtures
#   scripts/studio.sh build     production build into app/dist
#   scripts/studio.sh test      unit tests of both packages, and the chrome's render tests
#   scripts/studio.sh lint      eslint over app/ and packages/, --max-warnings 0
#   scripts/studio.sh check     types, tests, lint, build: the studio's merge floor
#
# Exit: 0 passed · 1 a row failed, or a test was skipped · 2 misuse, or an asset is missing.
#
# Every node container gets $REFS (default $GM_SCRATCH/refs) read-only at /refs, where
# the look tests read the pinned tables they compare the generated colour ramps against; the
# test and check commands refuse to run without it rather than skip a row that cannot pass.
#
# The dev server listens on every interface INSIDE its container and is published on the
# host's loopback only: a studio on a shared machine is not everyone's studio.
set -euo pipefail

here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
root=$(git -C "$here" rev-parse --show-toplevel)
gr=${GR:-$root/scripts/orch/gr}
# The pinned references the tests read: the look tests compare the generated colour tables
# against $GM_SCRATCH/refs/matplotlib-3.10.0/_cm_listed.py, and skip without it. A
# skipped test is not a pass, so the check would never go green on an unmounted host.
source "$(dirname "$(readlink -f "$0")")/orch/image.sh"
refs=${STUDIO_REFS:-${REFS:-$GM_SCRATCH/refs}}
port=${STUDIO_PORT:-5174}
command=${1:-serve}
packages=(graph-render graph-studio)
publish=()

log() { printf '\033[1m[studio]\033[0m %s\n' "$*"; }

# in_node <directory under the repo> <command...>
in_node() {
  local dir=$1 tty=()
  shift
  # `docker run -it` refuses without a terminal, which a gate never has. The update notifier is
  # off: its "npm install -g" advice is about the image's npm, and run on the host it changes nothing.
  [[ -t 0 && -t 1 ]] && tty=(-it)
  # HOST_API_ESCAPE_BREAK reaches the render tests only when set: the break of row host-api-escape.
  "$root/scripts/orch/drun" --rm "${tty[@]}" "${publish[@]}" -e NPM_CONFIG_UPDATE_NOTIFIER=false -e HOST_API_ESCAPE_BREAK \
    -v "$root:/w" -w "/w/$dir" -v "$refs:/refs:ro" "$GM_NODE_IMAGE" "$@"
}

# target/ is the gate output tree: scripts/orch/queue.sh land writes target/land-<label>/, and
# every browser gate writes target/studio-*/. Both container images run as root, so a cargo or
# vite build that creates it first leaves a root-owned directory the user cannot write into, and
# `queue.sh land` then fails on a worktree that never had one. Create it here, as the user, before
# anything runs in a container.
mkdir -p "$root/target"

# A row that could not run is a row that did not run: refuse rather than skip silently.
require_refs() {
  if [[ ! -f $refs/matplotlib-3.10.0/_cm_listed.py ]]; then
    log "MISSING $refs/matplotlib-3.10.0/_cm_listed.py — the look tests would SKIP."
    log "Fetch the pinned references with scripts/orch/fetch-refs.sh, or set REFS=<dir>."
    exit 2
  fi
}

build_wasm() {
  log "building graph-wasm (wasm32-unknown-unknown, release)"
  "$gr" cargo build -p graph-wasm --target wasm32-unknown-unknown --release
}

# What the browser cannot reach on its own: the wasm module and the fixtures the studio lists.
# Built every time (a no-op cargo run when fresh): staging whatever target/ held served a module
# older than the SDK, and the studio died on `exports.gm_dim is not a function` (2026-10-01).
stage_assets() {
  local wasm=$root/target/wasm32-unknown-unknown/release/graph_wasm.wasm
  local threads=$root/target/wasm-threads/wasm32-unknown-unknown/release/graph_wasm.wasm
  build_wasm || exit 2
  # The motor worker loads this one instead when the page is cross-origin isolated
  # (packages/graph-studio/src/motor/worker.ts); a page that is not keeps the serial module.
  log "building graph-wasm with threads (scripts/orch/wasm-threads.sh)"
  "$root/scripts/orch/wasm-threads.sh" || exit 2
  mkdir -p "$root/app/public"
  cp "$wasm" "$root/app/public/graph_wasm.wasm"
  cp "$threads" "$root/app/public/graph_wasm_threads.wasm"
  rm -rf "$root/app/public/fixtures"
  cp -R "$root/fixtures" "$root/app/public/fixtures"
  log "staged graph_wasm.wasm ($(wc -c <"$wasm") bytes), graph_wasm_threads.wasm ($(wc -c <"$threads") bytes) and fixtures/ into app/public"
}

install_deps() {
  [[ -x $root/app/node_modules/.bin/vite ]] && return
  log "installing app/node_modules"
  in_node app npm ci --ignore-scripts --no-audit --no-fund
}

types() {
  local config
  for config in packages/graph-render/tsconfig.json packages/graph-studio/tsconfig.json \
    packages/graph-studio/tsconfig.motor.json app/tsconfig.json; do
    log "tsc $config"
    in_node app node_modules/.bin/tsc --noEmit -p "../$config"
  done
  # Row host-api-types, its break (verdict 1): the contract with the name `focus` put back must
  # not compile, and must fail with TS2430 only; any other code means the fixture itself rotted.
  log "tsc packages/graph-studio/tests/breaks (expect TS2430 and nothing else)"
  # shellcheck disable=SC2016 # expanded by the shell inside the container
  in_node app bash -c 'out=$(node_modules/.bin/tsc --noEmit -p ../packages/graph-studio/tests/breaks/tsconfig.json) && { echo "the break compiled"; exit 1; }
codes=$(printf "%s\n" "$out" | grep -oE "error TS[0-9]+" | sort -u)
[ "$codes" = "error TS2430" ] || { printf "%s\n" "$out"; exit 1; }
echo "TS2430, as expected"'
}

# node:test counts a skipped test as not failed; here a skip is a row that did not run.
# shellcheck disable=SC2016 # expanded by the shell inside the container, not by this one
tap_verdict='
grep -E "^# (tests|pass|fail|skipped)" /tmp/tap.log
if [ "$code" -ne 0 ]; then grep -A30 -E "^ *not ok" /tmp/tap.log | grep -v "^ *at " | head -80; exit 1; fi
if ! grep -q "^# skipped 0$" /tmp/tap.log; then
  echo "a skipped test is not a pass:"; grep -E "# SKIP" /tmp/tap.log | head -20; exit 1
fi'

unit_tests() {
  local package
  for package in "${packages[@]}"; do
    log "tests packages/$package"
    in_node "packages/$package" bash -c "code=0
node --test --test-reporter=tap --experimental-strip-types tests/*.test.ts >/tmp/tap.log 2>&1 || code=\$?
$tap_verdict"
  done
}

# Row host-api-escape, its break (verdict 5): the same test over a body rendered through
# dangerouslySetInnerHTML must fail. The bundle reads the variable at run time, so no rebuild.
# shellcheck disable=SC2016 # expanded by the shell inside the container, not by this one
escape_break='
HOST_API_ESCAPE_BREAK=1 node --test --test-reporter=tap ../target/ui-tests/host-escape.test.js >/tmp/break.log 2>&1 && { echo "host-api-escape: its break passed"; exit 1; }
grep -q "^# fail [1-9]" /tmp/break.log || { tail -20 /tmp/break.log; exit 1; }
echo "host-api-escape break: red, as expected"'

# The chrome is JSX, which node does not run: bundled first, with the app's React.
render_tests() {
  log "render tests packages/graph-studio/tests/ui"
  in_node app bash -c "code=0
rm -rf ../target/ui-tests
node_modules/.bin/rolldown -c ../packages/graph-studio/ui-tests.config.mjs >/dev/null || exit 1
node --test --test-reporter=tap '../target/ui-tests/*.test.js' >/tmp/tap.log 2>&1 || code=\$?
$tap_verdict
$escape_break"
}

lint() {
  log "eslint --max-warnings 0"
  # From the root: a flat config does not see files above the directory eslint runs in.
  in_node . app/node_modules/.bin/eslint -c app/eslint.config.js --max-warnings 0 app/src app/vite.config.ts packages
  # Row lint, its negative control (verdict 5): the fixture holding every markup sink, which the
  # run above ignores, must draw exactly the five bans and nothing else.
  log "eslint over tests/ui/raw-html.tsx (expect the five markup-sink bans)"
  # shellcheck disable=SC2016 # expanded by the shell inside the container
  in_node . bash -c 'out=$(app/node_modules/.bin/eslint -c app/eslint.config.js --no-ignore packages/graph-studio/tests/ui/raw-html.tsx); code=$?
[ "$code" -eq 1 ] || { printf "%s\n" "$out"; echo "eslint exit $code, not 1"; exit 1; }
for ban in dangerouslySetInnerHTML innerHTML outerHTML insertAdjacentHTML; do
  printf "%s\n" "$out" | grep -q "No $ban:" || { printf "%s\n" "$out"; echo "no ban on $ban"; exit 1; }
done
found=$(printf "%s\n" "$out" | grep -cE "^ +[0-9]+:[0-9]+ +error")
banned=$(printf "%s\n" "$out" | grep -E "^ +[0-9]+:[0-9]+ +error" | grep -c "no-restricted-syntax$")
[ "$found" -eq 5 ] && [ "$banned" -eq 5 ] || { printf "%s\n" "$out"; exit 1; }
echo "five markup-sink bans, as expected"'
}

build() {
  log "production build into app/dist"
  in_node app node_modules/.bin/vite build
}

case "$command" in
  serve)
    install_deps
    stage_assets
    publish=(-p "127.0.0.1:$port:$port")
    log "dev server on http://127.0.0.1:$port"
    in_node app node_modules/.bin/vite --port "$port"
    ;;
  wasm)
    stage_assets
    ;;
  build)
    install_deps
    stage_assets
    build
    ;;
  test)
    require_refs
    install_deps
    unit_tests
    render_tests
    ;;
  lint)
    install_deps
    lint
    ;;
  check)
    require_refs
    install_deps
    stage_assets
    types
    unit_tests
    render_tests
    lint
    build
    log "ok"
    ;;
  *)
    sed -n '2,12p' "${BASH_SOURCE[0]}" >&2
    exit 2
    ;;
esac
