#!/usr/bin/env bash
# worker-form.sh — the one worker-form predicate, over any number of built bundles.
#
#   scripts/worker-form.sh DIR [DIR...]     each DIR is a built bundle's directory
#
# What it checks, per DIR. Every `*.js` under DIR, recursively (`assets/` included), is grepped
# for the fixed string `new Worker`. A line that also carries `blob:`, `data:` or
# `createObjectURL` is a worker the CSP refuses: `worker-src 'self'`, the policy a host is asked
# for (`docs/contract/host-api.md` condition 13, `docs/contract/packaging.md`), admits neither
# scheme. So:
#
#   - no such line at all -> exit 1. A bundle with no worker cannot pass vacuously: this is the
#     arm that would otherwise let a silently worker-less bundle through every other row.
#   - one such line carrying a refused scheme -> exit 1, naming `file:line` for at most 5.
#   - a DIR that is not there -> exit 2, which wins over 1.
#   - otherwise -> exit 0.
#
# The pattern is `grep -rnF 'new Worker'` piped into `grep -F -e`, never `new Worker([^)]*)`.
# `docs/reviews/review-bundle-unify.md` section 0 measured the `[^)]*` form a false negative on
# this tree's minified output: minification leaves the newline between `new Worker(new URL(` and
# its argument, a character class cannot cross it, and the prescribed grep reported "no worker" for
# a bundle that has one. Fixed-string matching has no such gap.
#
# Caveat: the predicate is line-granular, so both edges are visible. A minified chunk is one long
# line, so any `data:` anywhere on the same line as a `new Worker` (an inline data-URI icon, say) is
# reported as a worker the CSP refuses: over-reporting, the safe direction, and a false positive
# here costs one comment. A worker whose URL is assembled on a line *after* the `new Worker` is not
# seen at all: under-reporting, the direction that lets a real blob worker through. The escape hatch
# is the pack (`scripts/studio-pack.sh`), built unminified (`app/vite.pack.config.ts`), so the
# constructor and its argument are readable on separate lines.
#
# Read-only: it writes nothing and takes no lock.
set -euo pipefail

readonly MAX_REPORTED=5

# Every `new Worker` line under one DIR, as `DIR/file:line:text`. grep exits 1 when there is none,
# which is this script's first arm and not an error here.
worker_lines() {
  grep -rnF 'new Worker' --include='*.js' "$1" || true
}

# The refused schemes out of that, capped at MAX_REPORTED. Three `-e` literals, not a class:
# `blob:` and `data:` appear verbatim in the built output.
refused_lines() {
  grep -F -e 'blob:' -e 'data:' -e 'createObjectURL' | head -n "$MAX_REPORTED" || true
}

# One `grep -rnF` line as `file:line`, the text dropped: a minified chunk is one line of ~600 kB,
# and a gate row's log is not the place for it. Two fields, so the split stops at the first two
# colons and the rest of the line, colons included, is discarded.
at() {
  local rest=${1#*:}
  printf '%s:%s' "${1%%:*}" "${rest%%:*}"
}

check_dir() {
  local dir=$1 workers found=0 refused=0 line
  [[ -d $dir ]] || { printf 'worker-form: no directory: %s\n' "$dir" >&2; return 2; }
  workers=$(worker_lines "$dir")
  if [[ -n $workers ]]; then
    while IFS= read -r line; do
      [[ -n $line ]] || continue
      found=$((found + 1))
      printf 'worker-form: %s: %s: a worker is built here\n' "$dir" "$(at "$line")"
    done <<<"$workers"
  fi
  if ((found == 0)); then
    # shellcheck disable=SC2016  # the backticks name the string `new Worker`, they do not quote
    printf 'worker-form: %s: no `new Worker` in any *.js: the motor worker is gone\n' "$dir" >&2
    return 1
  fi
  while IFS= read -r line; do
    [[ -n $line ]] || continue
    refused=$((refused + 1))
    printf 'worker-form: %s: a worker built from a blob: or data: URL: %s\n' "$dir" "$(at "$line")" >&2
  done < <(printf '%s\n' "$workers" | refused_lines)
  printf 'worker-form: %s: %d worker line(s), %d refused\n' "$dir" "$found" "$refused"
  ((refused == 0))
}

main() {
  local dir code status=0
  (($# > 0)) || { printf 'usage: scripts/worker-form.sh DIR [DIR...]\n' >&2; exit 2; }
  for dir in "$@"; do
    check_dir "$dir" && code=0 || code=$?
    ((code == 0)) || status=$code
  done
  exit "$status"
}

main "$@"