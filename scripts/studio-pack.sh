#!/usr/bin/env bash
# studio-pack.sh — <graph-studio> as one self-contained, versioned pack: an ESM bundle, both wasm
# artifacts, and a manifest a host can check its download against.
#
#   scripts/studio-pack.sh [outdir]              the pack, into <outdir>/graph-studio-<version>/
#   STUDIO_PACK_BREAK=1 scripts/studio-pack.sh [outdir]
#                                               its negative control: the threads artifact is
#                                               deleted after the build, and the verify must exit 1
#                                               naming it
#
# Steps, in order: `scripts/studio.sh wasm` (both artifacts, built fresh, staged into app/public),
# the vite pack build (app/vite.pack.config.ts, PACK_MODE=pack), both wasm copied beside the
# bundle, pack.json, then the deterministic tarball `<outdir>/graph-studio-<version>.tgz`
# (`tar --sort=name --mtime=@0 --owner=0 --group=0 --numeric-owner --format=gnu --mode=...`,
# piped through `gzip -n`). Then the verify. The outdir must be inside the worktree: the container
# sees the repository at /w and nothing else. Default: target/pack.
#
# pack.json: {name, version, abi_version, source_rev, files: {<path>: {bytes, sha256}}}, the file
# keys in byte order, over every file of the pack but pack.json itself. `version` is
# packages/graph-studio/package.json, `abi_version` is graph-wasm's ABI_VERSION, `source_rev` is
# `git rev-parse HEAD` with `-dirty` appended on an unclean tree.
#
# The verify, exit 0 good / 1 bad / 2 could not build: every file of pack.json present with its
# sha256; both wasm artifacts present; the threads artifact named by a pack `.js` (that string is
# how the motor worker finds its sibling, packages/graph-studio/src/motor/worker.ts:59); nothing in
# the pack's code reaching for this repository (`/src/`, `node_modules`, `../`) — rollup stamps each
# module's own path in a `//#region` banner, which is provenance and not a reference, so the check
# reads the code with its comments dropped; and no worker built from a `blob:` or `data:` URL, which
# the CSP of verdict 13 refuses.
#
# Finally one line per file with its size, then the tarball's sha256.
#
# Exit: 0 built and verified · 1 the pack is bad · 2 could not build.
# Never takes the host gate lock. The wasm it stages land in app/public (target/ holds the pack).
set -uo pipefail

here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
root=$(git -C "$here" rev-parse --show-toplevel)
# shellcheck source=scripts/orch/image.sh
source "$here/orch/image.sh"
refs=${STUDIO_REFS:-${REFS:-$GM_SCRATCH/refs}}

name=graph-studio
version=$(sed -n 's/^  "version": "\([^"]*\)",$/\1/p' "$root/packages/graph-studio/package.json" | head -1)
abi=$(sed -n 's/^pub const ABI_VERSION: u32 = \([0-9][0-9]*\);$/\1/p' "$root/crates/graph-wasm/src/lib.rs" | head -1)
out=${1:-target/pack}
[[ $out == /* ]] || out=$root/$out
pack=$out/$name-$version
# The container sees the repository at /w and nothing else, so the pack is addressed there.
pack_rel=${pack#"$root"/}
problems=0

log() { printf '\033[1m[studio-pack]\033[0m %s\n' "$*"; }
bad() { problems=$((problems + 1)); printf 'studio-pack: %s\n' "$*" >&2; }

# in_node <command...> — node in the image, over the same path scripts/studio.sh uses.
in_node() {
  "$root/scripts/orch/drun" --rm -e NPM_CONFIG_UPDATE_NOTIFIER=false \
    -e PACK_MODE=pack -e PACK_OUT_DIR="/w/$pack_rel" -e PACK_UID="$(id -u)" -e PACK_GID="$(id -g)" \
    -v "$root:/w" -w /w/app -v "$refs:/refs:ro" "$GM_NODE_IMAGE" "$@"
}

build_pack() {
  [[ -n $version && -n $abi ]] || { log "no version/abi_version in the tree"; exit 2; }
  log "staging both wasm artifacts (scripts/studio.sh wasm)"
  "$root/scripts/studio.sh" wasm || exit 2
  [[ -x $root/app/node_modules/.bin/vite ]] || in_node npm ci --ignore-scripts --no-audit --no-fund || exit 2
  log "vite pack build into $pack_rel"
  mkdir -p "$out"
  # The two wasm copied in and the directory handed back inside the container: it runs as root,
  # and a root-owned pack is one the next run of this script cannot rewrite.
  in_node sh -c "node_modules/.bin/vite build --config vite.pack.config.ts &&
    cp /w/app/public/graph_wasm.wasm /w/app/public/graph_wasm_threads.wasm /w/$pack_rel &&
    chown -R \$PACK_UID:\$PACK_GID /w/$pack_rel" || exit 2
}

# The manifest, one file per line, in byte order. Ponytail: the verify reads back exactly the shape
# written here, line by line; a manifest written by anything else (nested keys, escapes, a
# different spacing) reads as "pack.json lists no file" rather than as a pack it cannot check.
write_manifest() {
  local file bytes digest first=1 rev
  rev=$(git -C "$root" rev-parse HEAD)
  [[ -z $(git -C "$root" status --porcelain) ]] || rev=$rev-dirty
  printf '{\n "name": "%s",\n "version": "%s",\n "abi_version": %s,\n "source_rev": "%s",\n "files": {\n' \
    "$name" "$version" "$abi" "$rev" >"$pack/pack.json"
  while IFS= read -r file; do
    bytes=$(wc -c <"$pack/$file")
    digest=$(sha256sum "$pack/$file" | cut -d' ' -f1)
    [[ $first == 1 ]] || printf ',\n' >>"$pack/pack.json"
    first=0
    printf '  "%s": {"bytes": %s, "sha256": "%s"}' "$file" "$bytes" "$digest" >>"$pack/pack.json"
  done < <(cd "$pack" && find . -type f ! -name pack.json | sed 's|^\./||' | LC_ALL=C sort)
  printf '\n }\n}\n' >>"$pack/pack.json"
}

write_tarball() {
  # --mode normalises what the file system hands over: a pack directory that already existed with
  # a group-writable mode would otherwise tar differently from a fresh one, and the tarball's
  # sha256 is a claim about the tree, not about the umask that built it.
  tar --sort=name --mtime=@0 --owner=0 --group=0 --numeric-owner --format=gnu \
    --mode=u+rw,go=rX,go-w -cf - -C "$out" "$name-$version" |
    gzip -n -9 >"$out/$name-$version.tgz" || exit 2
}

# Every file of pack.json, as `<path> <bytes> <sha256>`, in the order it was written. Only the
# last entry of the map carries no comma, so the pattern takes one or none.
manifest() {
  sed -n 's/^  "\([^"]*\)": {"bytes": \([0-9][0-9]*\), "sha256": "\([0-9a-f]\{64\}\)"}\(,\)\?$/\1 \2 \3/p' \
    "$pack/pack.json"
}

verify_files() {
  local listed=0 file bytes digest got
  while read -r file bytes digest; do
    listed=$((listed + 1))
    if [[ ! -f $pack/$file ]]; then bad "missing $file (pack.json lists it)"; continue; fi
    got=$(sha256sum "$pack/$file" | cut -d' ' -f1)
    [[ $got == "$digest" ]] || bad "$file: sha256 $got, pack.json says $digest"
    [[ $(wc -c <"$pack/$file") == "$bytes" ]] || bad "$file: not $bytes bytes, as pack.json says"
  done < <(manifest)
  ((listed > 0)) || bad "pack.json lists no file in the shape the verify reads"
}

verify_references() {
  local wasm line offenders verdict
  for wasm in graph_wasm.wasm graph_wasm_threads.wasm; do
    [[ -f $pack/$wasm ]] || bad "$wasm is not in the pack"
  done
  grep -qsF 'graph_wasm_threads.wasm' "$pack"/*.js ||
    bad "no pack .js names graph_wasm_threads.wasm: the worker would fall back to the serial module"
  # Comments out of it: the //#region banner names each module's own path, which is provenance.
  offenders=$(awk '!/^[[:space:]]*(\/\/|\/\*|\*)/ && ($0 ~ /\/src\// || $0 ~ /node_modules/ || $0 ~ /\.\.\//) {
                   print FILENAME ":" FNR ": " $0 }' "$pack"/*.js | head -5)
  while IFS= read -r line; do
    [[ -n $line ]] && bad "the pack reaches outside itself: $line"
  done <<<"$offenders"
  # The worker form, by the one predicate that now watches both bundles: the row embed-bundle runs
  # scripts/worker-form.sh over the service bundle, and the pack is the artifact it was extracted
  # from. A worker is one statement, so a blob: or data: URL on its line is a worker the CSP
  # refuses; the file:line comes back stripped of worker-form.sh's own `worker-form: DIR:` prefix,
  # so this script's message reads as it always did.
  verdict=$("$here/worker-form.sh" "$pack" 2>&1 >/dev/null || true)
  while IFS= read -r line; do
    [[ -n $line ]] && bad "a worker built from a blob: or data: URL: $line"
  done < <(sed -n 's|^worker-form: .*: a worker built from a blob: or data: URL: ||p' <<<"$verdict")
  if grep -qF 'the motor worker is gone' <<<"$verdict"; then
    bad "no worker in the pack: $(grep -F 'the motor worker is gone' <<<"$verdict")"
  fi
}

report() {
  local file
  while IFS= read -r file; do
    printf '%10d  %s\n' "$(wc -c <"$pack/$file")" "$file"
  done < <(cd "$pack" && find . -type f | sed 's|^\./||' | LC_ALL=C sort)
  printf '%s  %s\n' "$(sha256sum "$out/$name-$version.tgz" | cut -d' ' -f1)" "$name-$version.tgz"
}

build_pack
write_manifest
write_tarball
if [[ ${STUDIO_PACK_BREAK:-} == 1 ]]; then
  rm -f "$pack/graph_wasm_threads.wasm"
  log "negative control: graph_wasm_threads.wasm deleted from the pack"
fi
verify_files
verify_references
report
((problems == 0)) || { log "$problems problem(s): the pack is not shippable"; exit 1; }
log "ok: $name-$version, $name-$version.tgz"