#!/usr/bin/env bash
#
# guard-osionos.sh — rule 0.1 of prompt.md, enforced: osionos is READ ONLY.
#
# It lives in graph-engine, not in osionos: a guard written into the tree it protects
# would break the rule on its first commit.
#
#   guard-osionos.sh --snapshot          capture the baseline (run once, commit the file)
#   guard-osionos.sh --check             compare osionos against the baseline
#   guard-osionos.sh -- <cmd...>         snapshot to a temp file, run cmd, re-check:
#                                        exit 90 if the invariant broke even when cmd succeeded
#   guard-osionos.sh --self-test         prove the exit codes against a throwaway dirty repo
#   --include-ignored                    (with --snapshot/--check) also hash gitignored paths
#
#   OSIONOS_DIR       the repo to guard   (default /home/dlesieur/Documents/osionos)
#   OSIONOS_BASELINE  the baseline file   (default scripts/osionos-baseline.txt beside this script)
#
# Exit codes: 0 the invariant holds · 1 the guard itself could not run · 90 the invariant broke.
#
# Why contents and not `git status --porcelain`: osionos is already dirty, and a file
# listed ` M` or `??` keeps those two characters however often it is edited again, so
# status is blind to exactly the files most at risk. Every tracked and untracked file
# is hashed instead, plus HEAD and the index (a stage or commit is a write too), and
# submodules are walked recursively.
#
# Client-owned paths (CLIENT_OWNED below) are rewritten by the Claude Code client itself
# — a JSON round-trip of .claude/settings.json was observed dropping six allow rules
# mid-session with no agent involved. Changes there are REPORTED, never fatal. The list
# is declared here so the tolerance is visible, not a silent blind spot.
#
# Ponytail: the default hashed set is what git does not ignore. A write under a path
# osionos's .gitignore excludes (node_modules/, build/, .env) is invisible to --check,
# so the guard UNDER-REPORTS there — the dangerous direction. Escape hatch:
# --include-ignored hashes ignored paths too; slow on node_modules, so not the default.

set -euo pipefail

readonly CLIENT_OWNED='^\.claude/settings[^/]*\.json$'
readonly UNCAPTURED='# UNCAPTURED'
SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
readonly SCRIPT_DIR
DIR=${OSIONOS_DIR:-/home/dlesieur/Documents/osionos}
BASELINE=${OSIONOS_BASELINE:-$SCRIPT_DIR/osionos-baseline.txt}
IGNORED=0
TEMP=''

die() { printf 'guard-osionos: %s\n' "$*" >&2; exit 1; }

# fingerprint <repo> <prefix>: one "path<TAB>digest" line per file, recursing into submodules.
fingerprint() {
  local repo=$1 prefix=$2 path files=() others=(--exclude-standard)
  (( IGNORED )) && others=()
  printf '/HEAD\t%s\n' "$(git -C "$repo" rev-parse -q --verify HEAD || echo none)" | sed "s#^/#/$prefix#"
  printf '/INDEX\t%s\n' "$(git -C "$repo" ls-files -s -z | sha256sum | cut -d' ' -f1)" | sed "s#^/#/$prefix#"
  while IFS= read -r -d '' path; do
    if [[ -L $repo/$path ]]; then printf '%s\tlink:%s\n' "$prefix$path" "$(readlink "$repo/$path")"
    elif [[ -d $repo/$path && -e $repo/$path/.git ]]; then fingerprint "$repo/${path%/}" "$prefix${path%/}/"
    elif [[ -f $repo/$path ]]; then files+=("$path")
    elif [[ ! -e $repo/$path ]]; then printf '%s\tMISSING\n' "$prefix$path"
    fi
  done < <(git -C "$repo" ls-files -z --cached --others "${others[@]}" | sort -zu)
  if (( ${#files[@]} )); then
    (cd "$repo" && printf '%s\0' "${files[@]}" | xargs -0 sha256sum --) |
      awk -v p="$prefix" '{ h = $1; sub(/^[^ ]+  /, ""); printf "%s%s\t%s\n", p, $0, h }'
  fi
}

snapshot_to() {
  local out=$1
  git -C "$DIR" rev-parse --git-dir >/dev/null 2>&1 || die "not a git repo: $DIR"
  { printf '# guard-osionos baseline v1\n# include-ignored %s\n' "$IGNORED"
    fingerprint "$DIR" '' | LC_ALL=C sort -u; } > "$out.tmp"
  mv "$out.tmp" "$out"
}

# check_against <baseline>: print every change, return 90 if any is agent-owned.
check_against() {
  local base=$1
  [[ -f $base ]] || die "no baseline at $base; run --snapshot first"
  grep -qx "$UNCAPTURED" "$base" && die "baseline $base was never captured; run --snapshot on the host that has osionos"
  grep -qx "# include-ignored $IGNORED" "$base" || die "baseline mode differs from --include-ignored=$IGNORED; re-snapshot"
  git -C "$DIR" rev-parse --git-dir >/dev/null 2>&1 || die "not a git repo: $DIR"
  local status=0
  fingerprint "$DIR" '' | LC_ALL=C sort -u | awk -F'\t' -v client="$CLIENT_OWNED" '
    FNR == NR { if ($0 !~ /^#/) old[$1] = $2; next }
    { new[$1] = $2 }
    END {
      for (p in old) if (!(p in new)) report("REMOVED", p); else if (old[p] != new[p]) report("CHANGED", p)
      for (p in new) if (!(p in old)) report("ADDED", p)
      printf "guard-osionos: %d change(s), %d fatal\n", total, fatal
      exit fatal ? 90 : 0
    }
    function report(kind, p) {
      total++
      if (p ~ client) { printf "  %-8s %s  [client-owned: reported]\n", kind, p } else { fatal++; printf "  %-8s %s  [agent-owned: FATAL]\n", kind, p }
    }' "$base" - || status=$?
  return "$status"
}

run_guarded() {
  local status=0 verdict=0
  TEMP=$(mktemp)
  trap 'rm -f "$TEMP"' EXIT
  snapshot_to "$TEMP"
  "$@" || status=$?
  check_against "$TEMP" || verdict=$?
  (( verdict == 90 )) && { printf 'guard-osionos: INVARIANT BROKEN by: %s\n' "$*" >&2; return 90; }
  (( verdict != 0 )) && return 1
  return "$status"
}

main() {
  local mode=${1:-}
  [[ ${2:-} == --include-ignored ]] && IGNORED=1
  case $mode in
    --snapshot) snapshot_to "$BASELINE" && printf 'guard-osionos: baseline written to %s (%s entries)\n' "$BASELINE" "$(grep -vc '^#' "$BASELINE")" ;;
    --check) check_against "$BASELINE" ;;
    --self-test) exec bash "$SCRIPT_DIR/guard-osionos.sh" --self-test-run ;;
    --self-test-run) self_test ;;
    --) shift; (( $# )) || die "nothing to run after --"; run_guarded "$@" ;;
    *) die "usage: guard-osionos.sh --snapshot|--check [--include-ignored] | --self-test | -- <cmd...>" ;;
  esac
}

# self_test: every exit code the guard promises, against a repo that starts dirty.
self_test() {
  local root repo fails=0
  TEMP=$(mktemp -d)
  trap 'rm -rf "$TEMP"' EXIT
  root=$TEMP repo=$TEMP/osionos
  git init -q "$repo" && git -C "$repo" config user.email t@t && git -C "$repo" config user.name t
  printf 'tracked\n' > "$repo/tracked.txt" && printf 'node_modules/\n' > "$repo/.gitignore"
  git -C "$repo" add -A && git -C "$repo" commit -qm base
  printf 'dirty\n' >> "$repo/tracked.txt"
  mkdir -p "$repo/.claude" "$repo/node_modules" && printf '{}' > "$repo/.claude/settings.json" && printf 'n\n' > "$repo/notes.md"
  export OSIONOS_DIR=$repo OSIONOS_BASELINE=$root/baseline.txt
  local self=$SCRIPT_DIR/guard-osionos.sh
  expect() { local want=$1 got=0; shift; "$@" >/dev/null 2>&1 || got=$?; printf '  %-58s want %-3s got %-3s %s\n' "$*" "$want" "$got" "$([[ $want == "$got" ]] && echo ok || echo FAIL)"; [[ $want == "$got" ]] || fails=$((fails + 1)); }
  expect 0 bash "$self" --snapshot
  expect 0 bash "$self" --check
  local before; before=$(git -C "$repo" status --porcelain)
  printf 'x' >> "$repo/tracked.txt"
  [[ $(git -C "$repo" status --porcelain) == "$before" ]] && printf '  git status --porcelain unchanged by the extra byte (the blind spot)\n'
  expect 90 bash "$self" --check
  truncate -s -1 "$repo/tracked.txt"
  expect 0 bash "$self" --check
  printf '{"x":1}' > "$repo/.claude/settings.json"
  expect 0 bash "$self" --check
  printf '{}' > "$repo/.claude/settings.json"
  printf 'x' > "$repo/node_modules/ignored.js"
  expect 0 bash "$self" --check
  expect 0 bash "$self" --snapshot --include-ignored
  printf 'y' > "$repo/node_modules/ignored.js"
  expect 90 bash "$self" --check --include-ignored
  expect 0 bash "$self" --snapshot
  expect 90 bash "$self" -- sh -c "printf z >> '$repo/notes.md'"
  expect 0 bash "$self" --snapshot
  expect 90 bash "$self" -- git -C "$repo" add notes.md
  expect 3 bash "$self" -- sh -c 'exit 3'
  expect 1 bash "$self" --bogus
  printf 'guard-osionos self-test: %s\n' "$([[ $fails == 0 ]] && echo PASS || echo "FAIL ($fails)")"
  (( fails == 0 ))
}

main "$@"
