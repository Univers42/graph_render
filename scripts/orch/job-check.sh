#!/usr/bin/env bash
# job-check.sh — the deterministic half of an agent job: gate, freshness, rule lint, commit.
# An agent reads this ≤40-line output instead of spending tokens re-deriving any of it.
#   job-check.sh start <rows> [base]  run gate.sh <rows> detached; cwd = the worktree top level
#   job-check.sh wait [seconds]       block until the gate ends, at most <seconds> (default 540)
#   job-check.sh status               PASS | FAIL | RUNNING | STALE | DIED, summary, failing tails, lint
#   job-check.sh lint [base]          rule findings on everything since <base>: commits, edits, new files
#   job-check.sh commit               commit + push, only on a PASS gate over the same tree, no ERROR
# Exit: 0 PASS/clean/committed · 1 FAIL/STALE/DIED/ERROR · 2 usage or state error · 3 still running.
# <base> defaults to the merge-base with origin/develop. State lives in target/job-check/.
# Freshness is exact: a hash of HEAD, the diff against it and every untracked file.
# Ponytail: the lint is regex over added lines. It misses what hides behind a macro, an alias or a
# re-export, and flags matches in comments, strings and test oracles; a finding is evidence for the
# reviewer, not a verdict. House limits it cannot see (40-line functions, missing Ponytail markers)
# stay with the reviewer.
set -uo pipefail
here=$(dirname "$(readlink -f "$0")")
st=target/job-check
locks=${JC_LOCKS:-/goinfre/dlesieur/orch/locks}
slots=${JC_SLOTS:-3}

die() { echo "job-check: $*" >&2; exit 2; }
cap() { awk -v max="$1" 'NR <= max { print } END { if (NR > max) print "... +" NR - max " more lines" }'; }
is_running() { [[ -f $st/pid && ! -f $st/exit ]] && kill -0 "$(cat "$st/pid")" 2>/dev/null; }

untracked() { git ls-files -o --exclude-standard "$@" -- . ":(exclude)$st"; }

tree_sum() {
  { git rev-parse HEAD; git diff HEAD --binary; untracked -z | xargs -0r sha256sum; } |
    sha256sum | cut -c1-16
}

cmd_start() {
  local rows=${1:-} base=${2:-} t=0
  [[ -f $rows ]] || die "rows file not found: '$rows'"
  [[ $(git rev-parse --show-toplevel 2>/dev/null) == "$PWD" ]] || die "run from a worktree top level"
  is_running && die "a gate is already running here (pid $(cat "$st/pid"))"
  { rm -rf "$st" && mkdir -p "$st"; } || die "cannot create $st"
  readlink -f "$rows" >"$st/rows"
  [[ -n $base ]] && echo "$base" >"$st/base"
  tree_sum >"$st/tree"
  setsid nohup "$here/job-check.sh" _run </dev/null >"$st/runner.log" 2>&1 &
  while [[ ! -f $st/pid ]] && ((t < 50)); do sleep 0.1; ((t += 1)); done
  [[ -f $st/pid ]] || die "the runner did not start: $st/runner.log"
  echo "STARTED pid=$(cat "$st/pid") rows=$(cat "$st/rows") slots=$slots"
}

# Ponytail: a slot cap is not a CPU cap; one row may use every core (cargo, chromium). A slot
# frees when its holder exits, however it dies, so a killed runner never leaks one.
acquire_slot() {
  local i
  mkdir -p "$locks"
  while :; do
    for ((i = 1; i <= slots; i++)); do
      exec 9>"$locks/job-check.$i"
      flock -n 9 && { echo "$i" >"$st/slot"; return; }
    done
    echo waiting >"$st/slot"
    sleep 15
  done
}

cmd_run() {
  echo $$ >"$st/pid"
  acquire_slot
  "$here/gate.sh" "$st/gate" "$(cat "$st/rows")" 9>&-
  echo $? >"$st/exit"
}

cmd_wait() {
  local limit=${1:-540} t=0
  [[ -f $st/rows ]] || die "no gate started here"
  while is_running && ((t < limit)); do sleep 10; ((t += 10)); done
  cmd_status
}

count_lines() { if [[ -f $1 ]]; then wc -l <"$1"; else echo 0; fi; }

gate_state() {
  local s
  if [[ -f $st/exit ]]; then
    s=FAIL
    [[ $(cat "$st/exit") == 0 ]] && s=PASS
    [[ $(tree_sum) == "$(cat "$st/tree")" ]] || s="STALE (was $s): the tree changed after the gate started"
    echo "$s"
  elif is_running; then
    echo "RUNNING slot=$(cat "$st/slot" 2>/dev/null || echo -)" \
      "rows=$(count_lines "$st/gate/summary.txt")/$(grep -cvE '^(#|$)' "$(cat "$st/rows")")"
  else
    echo "DIED: the runner is gone and left no exit code ($st/runner.log)"
  fi
}

fail_tails() {
  local name log lines
  awk '$1 == "FAIL" { print $2 }' "$st/gate/summary.txt" 2>/dev/null | head -n 4 | while read -r name; do
    log=$st/gate/$name.log
    echo "--- $name: $log"
    lines=$(grep -E 'error|FAIL|panicked|failed|assert|expected|NOT-RUN' "$log" | tail -n 5)
    [[ -n $lines ]] || lines=$(tail -n 5 "$log")
    printf '%s\n' "$lines" | cut -c1-160
  done
}

cmd_status() {
  [[ -f $st/rows ]] || { echo "NONE: no gate started here"; return 2; }
  local state
  state=$(gate_state)
  echo "GATE $state"
  [[ -f $st/gate/summary.txt ]] && cat "$st/gate/summary.txt"
  fail_tails
  cmd_lint "" | cap 14
  case $state in PASS) return 0 ;; RUNNING*) return 3 ;; *) return 1 ;; esac
}

base_rev() {
  local b=${1:-}
  [[ -n $b ]] || b=$(cat "$st/base" 2>/dev/null) || true
  b=${b:-origin/develop}
  # Mid-merge HEAD has not moved yet: what MERGE_HEAD brings in is the base's work, not this branch's.
  if git rev-parse -q --verify MERGE_HEAD >/dev/null && git merge-base --is-ancestor "$b" MERGE_HEAD 2>/dev/null; then
    git rev-parse "$b"
    return
  fi
  git merge-base HEAD "$b" 2>/dev/null || die "no merge-base with '$b'"
}

changed() { { git diff --name-only "$1" --; untracked; } | sort -u; }

# file:line:text for every line added since <base>, untracked files whole.
added_lines() {
  git diff -U0 --no-color --no-ext-diff "$1" -- | awk '
    /^\+\+\+ / { f = substr($0, 7); next }
    /^@@/ { split($3, a, ","); n = substr(a[1], 2) + 0; next }
    /^\+/ { print f ":" n ":" substr($0, 2); n++ }'
  untracked -z | xargs -0r grep -InH '' -- 2>/dev/null
}

rule_paths() {
  changed "$1" | awk '
    $0 == ".claude" || /^\.claude\// { print "ERROR submodule   " $0 ": the .claude submodule is read-only" }
    /^\.opencode\// { print "WARN  opencode    " $0 ": changed; allowed only when the task names it" }
    /\.(log|orig|rej|tmp|swp|bak|pyc)$|(^|\/)\.nfs|(^|\/)(\.playwright-mcp|__pycache__)\/|(^|\/)core$|\.DS_Store$/ {
      print "ERROR junk        " $0 ": delete it" }'
}

rule_size() {
  local f now was
  changed "$1" | grep -E '\.(rs|ts|tsx|js|mjs|cjs|py|sh|css)$' | grep -v '\.d\.ts$' | while read -r f; do
    [[ -f $f ]] || continue
    now=$(wc -l <"$f")
    was=$(git show "$1:$f" 2>/dev/null | wc -l)
    if ((now > 300 && now > was)); then echo "ERROR size        $f: $now lines (limit 300, was $was)"; fi
  done
}

rule_patterns() {
  awk -v q="'" '
    function hit(level, rule, msg) { printf "%-5s %-11s %s:%s: %s\n", level, rule, f, n, msg }
    BEGIN {
      motor = "^crates/graph-(core|wasm|contract)/src/"
      vendor_ok = "(^|/)CLAUDE\\.md$|^\\.opencode/|^opencode\\.json$|^scripts/orch/(oc-|job-check|test-job-check)"
      secret = "(api[_-]?key|secret|passw(or)?d|token)[\"" q " ]*[:=] *[\"" q "][^\"" q " ]{12,}"
      trig = "\\.(mul_add|powi|powf|sin|cos|tan|exp|exp2|ln|log|log2|log10|atan2|atan|asin|acos|sinh|cosh|tanh|hypot|cbrt|exp_m1|ln_1p)\\("
    }
    {
      i = index($0, ":"); f = substr($0, 1, i - 1); r = substr($0, i + 1)
      j = index(r, ":"); n = substr(r, 1, j - 1); t = substr(r, j + 1)
      s = substr(t, 1, 70); l = tolower(t); det = (f ~ /(^|\/)tests?(\.rs|\/)/) ? "WARN" : "ERROR"
    }
    l ~ secret { hit("ERROR", "secret", "possible secret literal, value not shown"); next }
    f !~ vendor_ok && l ~ /claude-[a-z0-9]|anthropic|openai|chatgpt|gpt-[0-9o]|gemini|(^|[^a-z])(opus|sonnet|haiku)([^a-z]|$)|space-bunny|big-pickle|nemotron|longcat/ {
      hit("ERROR", "vendor-name", s) }
    t ~ /TODO|FIXME|XXX/ && t !~ /https?:\/\/|#[0-9]+/ { hit("ERROR", "todo", "no linked issue: " s) }
    t ~ /eslint-disable|@ts-ignore|@ts-nocheck|#\[allow\(|# noqa|\/\/ *nolint/ && t !~ /https?:\/\/|#[0-9]+/ {
      hit("ERROR", "suppression", "no linked issue: " s) }
    f ~ motor && t ~ /(^|[^A-Za-z])(HashMap|HashSet)([^A-Za-z]|$)/ { hit(det, "determinism", "use IndexMap/BTreeMap: " s) }
    f ~ motor && t ~ trig { hit(det, "determinism", "std float math, use libm: " s) }
    f ~ motor && t ~ /std::time|Instant::|SystemTime|thread_rng|rand::/ { hit(det, "determinism", "clock or unseeded rng: " s) }
    f ~ /\.tsx?$/ && f !~ /\.d\.ts$/ && t ~ /(:|<|as) *any([^A-Za-z0-9_]|$)/ { hit("WARN", "ts-any", s) }
    f ~ /\.tsx?$/ && t ~ /[A-Za-z0-9_)\]]![.;,)\]]/ { hit("WARN", "ts-non-null", s) }'
}

cargo_deps() { awk '/^\[/ { s = ($0 ~ /dependencies\]$/) } s && /^[A-Za-z0-9_-]+ *=/ { print $1 }' | sort -u; }
npm_deps() { jq -r '((.dependencies // {}) + (.devDependencies // {}) + (.peerDependencies // {})) | keys[]' 2>/dev/null | sort -u; }

rule_deps() {
  local f new level d
  changed "$1" | grep -E '(^|/)(Cargo\.toml|package\.json)$' | while read -r f; do
    [[ -f $f ]] || continue
    case $f in
      *.toml) new=$(comm -13 <(git show "$1:$f" 2>/dev/null | cargo_deps) <(cargo_deps <"$f")) ;;
      *) new=$(comm -13 <(git show "$1:$f" 2>/dev/null | npm_deps) <(npm_deps <"$f")) ;;
    esac
    level=WARN
    [[ $f == crates/graph-core/Cargo.toml ]] && level=ERROR
    for d in $new; do echo "$level dependency  $f: new '$d' (graph-core: closed list; elsewhere write the rung-6 accounting)"; done
  done
}

lint_findings() {
  local base lines
  base=$(base_rev "$1") || return 2
  lines=$(mktemp)
  added_lines "$base" >"$lines"
  { rule_paths "$base"; rule_size "$base"; rule_patterns <"$lines"; rule_deps "$base"; } | sort -u
  rm -f "$lines"
}

cmd_lint() {
  local out e w base
  base=$(base_rev "${1:-}") || return 2
  out=$(lint_findings "$base") || return 2
  e=$(grep -c '^ERROR' <<<"$out")
  w=$(grep -c '^WARN' <<<"$out")
  echo "LINT $e ERROR, $w WARN (since ${base:0:8})"
  [[ -n $out ]] && cap 16 <<<"$out"
  ((e == 0))
}

cmd_commit() {
  local br rc word=COMMITTED
  br=$(git rev-parse --abbrev-ref HEAD)
  case $br in develop | main | HEAD) die "refusing to commit on '$br'" ;; esac
  [[ $(cat "$st/exit" 2>/dev/null) == 0 ]] || die "the gate is not PASS here: start + wait first"
  [[ $(tree_sum) == "$(cat "$st/tree")" ]] || die "the tree changed after the gate started: start it again"
  cmd_lint >/dev/null || die "rule ERRORs remain: run job-check.sh lint"
  { git add -A && git reset -q -- "$st"; } || die "git add failed"
  # An empty diff says UNCHANGED, so a caller can tell a cycle that made no progress from one that did.
  if git diff --cached --quiet; then word=UNCHANGED
  else git -c user.name=LESdylan -c user.email=dev.pro.photo@gmail.com commit -q -m updated || die "commit failed"; fi
  git push -q origin HEAD 2>&1 | tail -n 2
  rc=${PIPESTATUS[0]}
  ((rc == 0)) || { echo "$word $(git rev-parse --short HEAD) on $br, PUSH FAILED (exit $rc)"; return 1; }
  echo "$word $(git rev-parse --short HEAD) on $br, pushed"
}

cmd=${1:-}
shift || true
case $cmd in
  start) cmd_start "$@" ;;
  wait) cmd_wait "$@" | cap 40 ;;
  status) cmd_status | cap 40 ;;
  lint) cmd_lint "$@" ;;
  commit) cmd_commit ;;
  _run) cmd_run ;;
  *) sed -n '2,9p' "$here/job-check.sh" >&2; exit 2 ;;
esac
