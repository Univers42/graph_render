#!/usr/bin/env bash
# embed-files.sh — the staged file list of `/embed/<version>/` equals the documented one.
#
#   scripts/embed-files.sh DIR      DIR is a `scripts/studio.sh embed` output directory
#
# The documented list is parsed out of `docs/deploy/service.md`, not copied here: the bullet of the
# build's file list, `- \`embed/<version>/\`: ...`, whose backticked names are the entries. A copy
# in this script would be a second list to keep in step, and the drift it was written to catch is
# exactly a drift between the two. `assets/` is one entry: the bullet says "their chunks under
# `assets/`", and the chunks carry content hashes in their names, so it is checked as "at least
# one `*.js` under `assets/`", never by name.
#
# Three checks, exit 1 naming the file that broke one:
#   - every regular file at DIR's top level is named in that bullet. A file the server will serve
#     under a versioned, `immutable` URL, that no document tells a host to ask for.
#   - every file the bullet names exists in DIR. A host that follows the document and gets a 404.
#   - `assets/` holds at least one `*.js`. Without it the element's worker URL has nothing to
#     resolve to, and only a browser would notice.
#
# Why checking DIR is checking what the image ships: `scripts/service.sh:81` builds that exact
# directory with `scripts/studio.sh embed` and `scripts/service.sh:84` moves it, whole, to
# `target/service/stage/embed/<version>/`, which is the docker build context's only embed path. No
# file is added or dropped between the bundle and the stage, so this runs with no docker build at
# all; `scripts/service-image.sh` then checks the image that stage produces.
#
# Exit: 0 the list matches · 1 drift, named · 2 no DIR, or the document has no such bullet.
set -euo pipefail

readonly DOC=docs/deploy/service.md
readonly LABEL='embed/<version>/'

# The bullet's backticked names, one per line, the label itself dropped: `embed/<version>/` is the
# directory's name, not one of its files.
# The bullet and every indented continuation of it. The list wraps across two lines in the
# document, so a parser that read only the line the label is on would check half the list and
# report the other half as drift, which is a wrong answer in the loud direction.
bullet_text() {
  awk -v label="$LABEL" '
    !inside && index($0, "- `" label "`") == 1 { inside = 1; print; next }
    inside && $0 ~ /^[[:space:]]+[^[:space:]]/ { print; next }
    inside { exit }
  ' "$DOC"
}

documented() {
  local bullet
  bullet=$(bullet_text)
  [[ -n $bullet ]] || return 1
  # shellcheck disable=SC2016  # the backticks delimit the names to read, they are not a quote
  printf '%s\n' "$bullet" | grep -o -e '`[^`]*`' | tr -d '`' | tail -n +2
}

report_drift() {
  printf 'embed-files: %s\n' "$*" >&2
  drift=$((drift + 1))
}

check_named_exist() {
  local name
  while IFS= read -r name; do
    [[ $name == assets/ ]] && continue
    [[ -f $dir/$name ]] || report_drift "documented but missing: $name"
  done < <(documented)
}

check_all_named() {
  local file base
  while IFS= read -r file; do
    base=${file##*/}
    documented | grep -qxF "$base" || report_drift "in $dir but not in $DOC: $base"
  done < <(find "$dir" -maxdepth 1 -type f -printf '%f\n' | LC_ALL=C sort)
}

check_assets() {
  local chunks=0
  [[ -d $dir/assets ]] && chunks=$(find "$dir/assets" -type f -name '*.js' | wc -l)
  ((chunks > 0)) || report_drift "documented \`assets/\` holds no *.js: the worker URL resolves to nothing"
}

main() {
  local dir=$1
  (($# == 1)) || { printf 'usage: scripts/embed-files.sh DIR\n' >&2; exit 2; }
  [[ -d $dir ]] || { printf 'embed-files: no directory: %s\n' "$dir" >&2; exit 2; }
  documented >/dev/null || { printf 'embed-files: %s has no %s bullet\n' "$DOC" "$LABEL" >&2; exit 2; }
  check_all_named
  check_named_exist
  check_assets
  printf 'embed-files: %s matches %s: %s\n' "$dir" "$DOC" "$(documented | tr '\n' ' ')"
  ((drift == 0)) || { printf 'embed-files: %d drift(s) between %s and %s\n' "$drift" "$dir" "$DOC" >&2; exit 1; }
}

drift=0
main "$@"