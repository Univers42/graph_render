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
# status is blind to exactly the files most at risk. What is fingerprinted instead:
#   - every tracked and untracked file, by content and mode (a chmod is a write);
#   - every untracked directory, so an empty `mkdir` is a write too;
#   - HEAD (symbolic ref and commit), every ref, and the index as git records it;
#   - every file in the git dir outside the object store — config, hooks, info/, logs,
#     refs, packed-refs — by content and mode, plus the object store's file names and
#     sizes (objects are content-addressed: the name is the digest) and the whole of
#     .git/modules, so `git switch -c`, `git config`, `git tag` and a new hook all count;
#   - submodules, recursively, the same way.
# Paths are %q-quoted in the records, so a tab or newline in a file name cannot split one.
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
# Also unseen, by design: timestamps (a bare `touch`), ownership, the mode of a
# non-empty directory, the raw bytes of .git/index (read-only `git status` rewrites its
# stat cache; what the index records is covered) and *.lock files. A repack or gc
# renames object files, so it reads as a change to /.git/objects: over-reported, safe.

set -Eeuo pipefail

declare -rx CLIENT_OWNED='^\.claude/settings[^/]*\.json$'
readonly UNCAPTURED='# UNCAPTURED'
readonly FORMAT='# guard-osionos baseline v2'
SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
readonly SCRIPT_DIR
DIR=${OSIONOS_DIR:-/home/dlesieur/Documents/osionos}
BASELINE=${OSIONOS_BASELINE:-$SCRIPT_DIR/osionos-baseline.txt}
IGNORED=0
TEMP=''
VERDICT=0

die() { printf 'guard-osionos: %s\n' "$*" >&2; exit 1; }

# Any unexpected failure is "the guard could not run" (exit 1), never another code. Every
# fingerprinting call below is a plain command — none sits under && / || / if, where bash
# silently switches errexit off for the whole call tree and a half-written fingerprint
# would pass for a whole one.
trap 'die "failed (line $LINENO): $BASH_COMMAND"' ERR

# record <path> <value>: one "key<TAB>value" line, the key %q-quoted.
record() { local key; printf -v key '%q' "$1"; printf '%s\t%s\n' "$key" "$2"; }

digest() { sha256sum | cut -c1-64; }

# hash_files <root> <prefix> <relative path...>: one record per file, "sha256:mode".
hash_files() {
  local root=$1 prefix=$2 i
  shift 2
  (( $# )) || return 0
  local -a names=("$@") sums modes
  mapfile -d '' sums < <(cd "$root" && printf '%s\0' "${names[@]}" | xargs -0 sha256sum -z --)
  mapfile -d '' modes < <(cd "$root" && printf '%s\0' "${names[@]}" | xargs -0 stat --printf '%a\0' --)
  (( ${#sums[@]} == $# && ${#modes[@]} == $# )) || die "could not hash every file under $root"
  for i in "${!names[@]}"; do record "$prefix${names[i]}" "${sums[i]:0:64}:${modes[i]}"; done
}

# git_state <repo> <prefix>: HEAD (symbolic ref and commit), every ref, the index's entries.
git_state() {
  local repo=$1 prefix=$2 head
  head="$(git -C "$repo" symbolic-ref -q HEAD || echo detached) $(git -C "$repo" rev-parse -q --verify HEAD || echo none)"
  record "/${prefix}HEAD" "$head"
  record "/${prefix}REFS" "$(git -C "$repo" for-each-ref --format='%(refname) %(objectname) %(symref)' | digest)"
  record "/${prefix}INDEX" "$(git -C "$repo" ls-files -s -z | digest)"
}

# git_dir <gitdir> <prefix>: the git dir's own files, the object store, and .git/modules.
git_dir() {
  local gitdir=$1 prefix=$2 path files=()
  while IFS= read -r -d '' path; do files+=("$path"); done < <(cd "$gitdir" &&
    find . \( -path ./objects -o -path ./modules \) -prune -o \
      -type f ! -path ./index ! -name '*.lock' -printf '%P\0' | LC_ALL=C sort -z)
  hash_files "$gitdir" "/${prefix}.git/" "${files[@]}"
  record "/${prefix}.git/objects" "$(cd "$gitdir" && find objects -type f -printf '%P %s\n' | LC_ALL=C sort | digest)"
  [[ -d $gitdir/modules ]] || return 0
  record "/${prefix}.git/modules" "$(cd "$gitdir" && {
    find modules -type f ! -name index ! -name '*.lock' -printf '%m %P\n' | LC_ALL=C sort
    find modules -type f ! -name index ! -name '*.lock' -print0 | LC_ALL=C sort -z | xargs -0r sha256sum -z --
  } | digest)"
}

# worktree <repo> <prefix>: every file git does not ignore, and every untracked directory.
worktree() {
  local repo=$1 prefix=$2 path files=() others=(--exclude-standard)
  (( IGNORED )) && others=()
  while IFS= read -r -d '' path; do
    if [[ -L $repo/$path ]]; then record "$prefix$path" "link:$(readlink "$repo/$path")"
    elif [[ -d $repo/$path && -e $repo/$path/.git ]]; then repository "$repo/${path%/}" "$prefix${path%/}/"
    elif [[ -f $repo/$path ]]; then files+=("$path")
    elif [[ ! -e $repo/$path ]]; then record "$prefix$path" MISSING
    fi
  done < <(git -C "$repo" ls-files -z --cached --others "${others[@]}" | LC_ALL=C sort -zu)
  hash_files "$repo" "$prefix" "${files[@]}"
  while IFS= read -r -d '' path; do
    if [[ $path == */ ]]; then record "$prefix$path" "dir:$(stat --printf '%a' -- "$repo/$path")"; fi
  done < <(git -C "$repo" ls-files -z --others --directory "${others[@]}" | LC_ALL=C sort -z)
}

# repository <repo> <prefix>: git state, work tree, and the git dir unless a parent's
# .git/modules already holds it.
repository() {
  local repo=$1 prefix=$2 gitdir common
  gitdir=$(git -C "$repo" rev-parse --absolute-git-dir)
  common=$(git -C "$repo" rev-parse --path-format=absolute --git-common-dir)
  git_state "$repo" "$prefix"
  worktree "$repo" "$prefix"
  if [[ -z $prefix || -d $repo/.git ]]; then git_dir "$gitdir" "$prefix"; fi
  if [[ $common != "$gitdir" ]]; then git_dir "$common" "${prefix}common:"; fi
}

# snapshot_to <file>: the header, then the sorted fingerprint of osionos.
snapshot_to() {
  local out=$1
  git -C "$DIR" rev-parse --git-dir >/dev/null 2>&1 || die "not a git repo: $DIR"
  { printf '%s\n# include-ignored %s\n' "$FORMAT" "$IGNORED"
    repository "$DIR" '' | LC_ALL=C sort -u; } > "$out.tmp"
  mv "$out.tmp" "$out"
}

# check_against <baseline>: print every change; VERDICT becomes 90 if any is agent-owned.
# It reports through VERDICT, not its status, so no caller ever runs it under ||.
check_against() {
  local base=$1 now
  [[ -f $base ]] || die "no baseline at $base; run --snapshot first"
  grep -qx "$UNCAPTURED" "$base" && die "baseline $base was never captured; run --snapshot on the host that has osionos"
  grep -qx "$FORMAT" "$base" || die "baseline $base is not in the current format; re-snapshot"
  grep -qx "# include-ignored $IGNORED" "$base" || die "baseline mode differs from --include-ignored=$IGNORED; re-snapshot"
  now=$TEMP/now
  snapshot_to "$now"
  VERDICT=0
  awk -F'\t' '
    /^#/ { next }
    FNR == NR { old[$1] = $2; next }
    { new[$1] = $2 }
    END {
      for (p in old) if (!(p in new)) report("REMOVED", p); else if (old[p] != new[p]) report("CHANGED", p)
      for (p in new) if (!(p in old)) report("ADDED", p)
      printf "guard-osionos: %d change(s), %d fatal\n", total, fatal
      exit fatal ? 90 : 0
    }
    function report(kind, p) {
      total++
      if (p ~ ENVIRON["CLIENT_OWNED"]) { printf "  %-8s %s  [client-owned: reported]\n", kind, p }
      else { fatal++; printf "  %-8s %s  [agent-owned: FATAL]\n", kind, p }
    }' "$base" "$now" || VERDICT=$?
  (( VERDICT == 0 || VERDICT == 90 )) || die "could not compare against $base"
}

# run_guarded <cmd...>: snapshot, run, re-check, exit; 90 beats the command's own status.
# It exits rather than returns: a non-zero return would trip the ERR trap.
run_guarded() {
  local status=0
  snapshot_to "$TEMP/before"
  "$@" || status=$?
  check_against "$TEMP/before"
  if (( VERDICT == 90 )); then
    printf 'guard-osionos: INVARIANT BROKEN by: %s\n' "$*" >&2
    exit 90
  fi
  exit "$status"
}

main() {
  local mode=${1:-}
  if [[ ${2:-} == --include-ignored ]]; then IGNORED=1; fi
  if [[ $mode != --self-test* ]]; then
    TEMP=$(mktemp -d)
    trap 'rm -rf "$TEMP"' EXIT
  fi
  case $mode in
    --snapshot)
      snapshot_to "$BASELINE"
      printf 'guard-osionos: baseline written to %s (%s entries)\n' "$BASELINE" "$(grep -vc '^#' "$BASELINE")" ;;
    --check) check_against "$BASELINE"; exit "$VERDICT" ;;
    --self-test) exec bash "$SCRIPT_DIR/guard-osionos.sh" --self-test-run ;;
    --self-test-run) self_test ;;
    --) shift; (( $# )) || die "nothing to run after --"; run_guarded "$@" ;;
    *) die "usage: guard-osionos.sh --snapshot|--check [--include-ignored] | --self-test | -- <cmd...>" ;;
  esac
}

# expect <exit> <guard args...>: run the guard, compare its exit code, count a miss.
expect() {
  local want=$1 got=0
  shift
  bash "$SCRIPT_DIR/guard-osionos.sh" "$@" >/dev/null 2>&1 || got=$?
  local shown=${*//$TEMP/T}
  shown=${shown//$'\t'/\\t}
  printf '  %-62s want %-3s got %-3s %s\n' "${shown//$'\n'/\\n}" "$want" "$got" "$([[ $want == "$got" ]] && echo ok || echo FAIL)"
  if [[ $want != "$got" ]]; then fails=$((fails + 1)); fi
}

# self_test_repo <root>: a repo that starts dirty, with a submodule and awkward names.
self_test_repo() {
  local root=$1 repo=$1/osionos
  git init -q "$root/subsrc" && printf 's\n' > "$root/subsrc/s.txt"
  git init -q "$repo" && printf 'tracked\n' > "$repo/tracked.txt" && printf 'node_modules/\n' > "$repo/.gitignore"
  printf 't\n' > "$repo/"$'tab\tname.txt' && printf 'n\n' > "$repo/"$'new\nline.txt'
  local g=(-c user.email=t@t -c user.name=t -c commit.gpgsign=false -c protocol.file.allow=always)
  git "${g[@]}" -C "$root/subsrc" add -A && git "${g[@]}" -C "$root/subsrc" commit -qm sub
  git "${g[@]}" -C "$repo" submodule --quiet add "$root/subsrc" sub
  git "${g[@]}" -C "$repo" add -A && git "${g[@]}" -C "$repo" commit -qm base
  printf 'dirty\n' >> "$repo/tracked.txt"
  mkdir -p "$repo/.claude" "$repo/node_modules"
  printf '{}' > "$repo/.claude/settings.json" && printf 'n\n' > "$repo/notes.md"
}

# self_test_contents <repo>: file contents, modes, names, directories, exit codes.
# shellcheck disable=SC2016  # "$1" is expanded by the child sh, on purpose
self_test_contents() {
  local repo=$1 before
  expect 0 --snapshot
  expect 0 --check
  before=$(git -C "$repo" status --porcelain)
  printf 'x' >> "$repo/tracked.txt"
  if [[ $(git -C "$repo" status --porcelain) == "$before" ]]; then
    printf '  git status --porcelain unchanged by the extra byte (the blind spot)\n'
  fi
  expect 90 --check
  truncate -s -1 "$repo/tracked.txt"
  expect 0 --check
  printf '{"x":1}' > "$repo/.claude/settings.json"
  expect 0 --check
  printf '{}' > "$repo/.claude/settings.json" && printf 'x' > "$repo/node_modules/ignored.js"
  expect 0 --check
  expect 0 --snapshot --include-ignored
  printf 'y' > "$repo/node_modules/ignored.js"
  expect 90 --check --include-ignored
  expect 0 --snapshot
  expect 90 -- sh -c 'printf z >> "$1"' sh "$repo/notes.md"
  expect 90 -- sh -c 'printf z >> "$1"' sh "$repo/"$'tab\tname.txt'
  expect 90 -- sh -c 'printf z >> "$1"' sh "$repo/"$'new\nline.txt'
  expect 90 -- chmod +x "$repo/tracked.txt"
  expect 90 -- mkdir "$repo/empty"
  expect 90 -- sh -c 'printf z >> "$1"' sh "$repo/sub/s.txt"
  expect 3 -- sh -c 'exit 3'
  expect 1 --bogus
  mkdir "$TEMP/broken" && printf '#!/bin/sh\nexit 1\n' > "$TEMP/broken/stat" && chmod +x "$TEMP/broken/stat"
  PATH="$TEMP/broken:$PATH" expect 1 --snapshot
  PATH="$TEMP/broken:$PATH" expect 1 --check
}

# self_test_git <repo>: writes that touch only the git dir, and reads that must not count.
# shellcheck disable=SC2016  # "$1" is expanded by the child sh, on purpose
self_test_git() {
  local repo=$1
  expect 0 -- git -C "$repo" status
  expect 0 -- git -C "$repo" log --oneline
  expect 0 -- git -C "$repo/sub" status
  expect 90 -- git -C "$repo" add notes.md
  expect 90 -- git -C "$repo" switch -q -c evil
  expect 90 -- git -C "$repo" branch -q other
  expect 90 -- git -C "$repo" -c tag.gpgSign=false tag t1
  expect 90 -- git -C "$repo" config guard.test yes
  expect 90 -- sh -c 'printf "#!/bin/sh\n" > "$1/.git/hooks/pre-commit"' sh "$repo"
  expect 90 -- chmod +x "$repo/.git/hooks/pre-commit"
  expect 90 -- git -C "$repo/sub" switch -q -c evil
  expect 90 -- git -C "$repo/sub" config guard.test yes
}

# self_test: every exit code the guard promises, and every write class it claims to see.
self_test() {
  local fails=0
  TEMP=$(mktemp -d)
  trap 'rm -rf "$TEMP"' EXIT
  self_test_repo "$TEMP" >/dev/null
  export OSIONOS_DIR=$TEMP/osionos OSIONOS_BASELINE=$TEMP/baseline.txt
  self_test_contents "$OSIONOS_DIR"
  self_test_git "$OSIONOS_DIR"
  printf 'guard-osionos self-test: %s\n' "$([[ $fails == 0 ]] && echo PASS || echo "FAIL ($fails)")"
  exit $(( fails == 0 ? 0 : 1 ))
}

main "$@"
