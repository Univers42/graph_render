#!/usr/bin/env bash
# studio-pack-manifest.sh PACK_DIR SOURCE_REV TREE — writes PACK_DIR/pack.json and verifies the
# pack, for packaging/studio-pack.Dockerfile. A port of scripts/studio-pack.sh's write_manifest,
# verify_files and verify_references (same pack.json line format, same checks, its worker-form.sh
# taken from TREE), plus one check of its own: the pack is exactly the six files of
# docs/contract/packaging.md. SOURCE_REV is the commit the workflow archived, so never `-dirty`.
#
# Exit: 0 written and verified · 1 the pack is bad · 2 usage.
set -uo pipefail

(($# == 3)) || { echo "usage: studio-pack-manifest.sh PACK_DIR SOURCE_REV TREE" >&2; exit 2; }
pack=$1 rev=$2 tree=$3
[[ $rev =~ ^[0-9a-f]{40}$ ]] || { echo "studio-pack: SOURCE_REV is not a full sha: '$rev'" >&2; exit 2; }
readonly EXPECTED_FILES="graph-studio.js graph_wasm.wasm graph_wasm_threads.wasm helper.js pack.json worker.js"
name=graph-studio
version=$(sed -n 's/^  "version": "\([^"]*\)",$/\1/p' "$tree/packages/graph-studio/package.json" | head -1)
abi=$(sed -n 's/^pub const ABI_VERSION: u32 = \([0-9][0-9]*\);$/\1/p' "$tree/crates/graph-wasm/src/lib.rs" | head -1)
problems=0

bad() { problems=$((problems + 1)); printf 'studio-pack: %s\n' "$*" >&2; }

write_manifest() {
  local file bytes digest first=1
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

manifest() {
  sed -n 's/^  "\([^"]*\)": {"bytes": \([0-9][0-9]*\), "sha256": "\([0-9a-f]\{64\}\)"}\(,\)\?$/\1 \2 \3/p' \
    "$pack/pack.json"
}

verify_file_set() {
  local found
  found=$(cd "$pack" && find . -type f | sed 's|^\./||' | LC_ALL=C sort | tr '\n' ' ')
  [[ ${found% } == "$EXPECTED_FILES" ]] || bad "pack files are '${found% }', packaging.md lists '$EXPECTED_FILES'"
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
  local line offenders verdict
  grep -qsF 'graph_wasm_threads.wasm' "$pack"/*.js ||
    bad "no pack .js names graph_wasm_threads.wasm: the worker would fall back to the serial module"
  # Comments dropped: rollup's //#region banner names each module's own path, which is provenance.
  offenders=$(awk '!/^[[:space:]]*(\/\/|\/\*|\*)/ && ($0 ~ /\/src\// || $0 ~ /node_modules/ || $0 ~ /\.\.\//) {
                   print FILENAME ":" FNR ": " $0 }' "$pack"/*.js | head -5)
  while IFS= read -r line; do
    [[ -n $line ]] && bad "the pack reaches outside itself: $line"
  done <<<"$offenders"
  verdict=$(bash "$tree/scripts/worker-form.sh" "$pack" 2>&1 >/dev/null) ||
    bad "worker-form.sh refused the pack: $verdict"
}

[[ -n $version && -n $abi ]] || { echo "studio-pack: no version/abi_version in $tree" >&2; exit 1; }
write_manifest
verify_file_set
verify_files
verify_references
((problems == 0)) || { echo "studio-pack: $problems problem(s): the pack is not shippable" >&2; exit 1; }
echo "studio-pack: ok, $name-$version abi $abi source_rev $rev"
