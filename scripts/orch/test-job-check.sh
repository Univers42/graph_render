#!/usr/bin/env bash
# test-job-check.sh — behaviour tests for scripts/orch/job-check.sh.
# Plain bash, no bats: a temp repo with a bare temp origin, a temp lock directory, and rows made of
# `true`/`false`, so no cargo and no real remote are touched. Exits 0 when every case passes, 1 otherwise.
set -uo pipefail
here=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
JC=$here/job-check.sh
tmp=$(mktemp -d "${TMPDIR:-/tmp}/jobcheck.XXXXXX")
trap 'rm -rf "$tmp"' EXIT
export JC_LOCKS=$tmp/locks JC_SLOTS=2
pass=0
fail=0

ok() { pass=$((pass + 1)); printf 'ok    %s\n' "$1"; }
no() { fail=$((fail + 1)); printf 'FAIL  %s: %s\n' "$1" "$2"; }

# has <name> <want-rc> <want-regex> <cmd> [args...]: rc equal and stdout matching the regex
has() {
  local name=$1 wrc=$2 want=$3 out rc
  shift 3
  out=$("$@" 2>&1)
  rc=$?
  if [[ $rc == "$wrc" && $out =~ $want ]]; then ok "$name"
  else no "$name" "rc=$rc want=$wrc out=$(head -c 300 <<<"$out" | tr '\n' '|')"; fi
}

g() { git -c user.name=t -c user.email=t@t -c init.defaultBranch=develop "$@"; }

setup() {
  git init -q --bare "$tmp/origin.git"
  g init -q "$tmp/wt" && cd "$tmp/wt" || exit 2
  printf '/target\n' >.gitignore
  mkdir -p crates/graph-core/src && printf '[dependencies]\nlibm = "0.2"\n' >crates/graph-core/Cargo.toml
  printf 'ok|0|true\n' >"$tmp/pass.rows"
  printf 'ok|0|true\nbad|0|echo boom-failed; false\n' >"$tmp/fail.rows"
  printf 'slow|0|sleep 3\n' >"$tmp/slow.rows"
  g add -A && g commit -qm base && g remote add origin "$tmp/origin.git" && g push -q origin develop
  g checkout -q -b feat
}

gate() { "$JC" start "$1" >/dev/null && "$JC" wait 60; }

case_gate() {
  has "status before start" 2 '^NONE' "$JC" status
  has "start refuses a missing rows file" 2 'rows file not found' "$JC" start "$tmp/none.rows"
  has "passing row gives PASS" 0 'GATE PASS' gate "$tmp/pass.rows"
  has "failing row gives FAIL with its tail" 1 'GATE FAIL.*--- bad.*boom-failed' gate "$tmp/fail.rows"
  "$JC" start "$tmp/slow.rows" >/dev/null
  has "a second start is refused while running" 2 'already running' "$JC" start "$tmp/pass.rows"
  has "status while running" 3 'GATE RUNNING' "$JC" status
  has "wait returns the final state" 0 'GATE PASS' "$JC" wait 60
  gate "$tmp/pass.rows" >/dev/null
  printf 'x\n' >edited.txt
  has "an edit after start is STALE" 1 'GATE STALE \(was PASS\)' "$JC" status
  rm edited.txt
  has "undoing the edit is fresh again" 0 'GATE PASS' "$JC" status
}

case_lint() {
  local vendor=open
  vendor+=ai
  has "clean tree lints clean" 0 'LINT 0 ERROR, 0 WARN' "$JC" lint
  printf 'use std::collections::HashMap;\nlet y = x.sin();\n' >crates/graph-core/src/a.rs
  has "HashMap and std sin in the motor" 1 'determinism +crates/graph-core/src/a.rs:1.*determinism +crates/graph-core/src/a.rs:2' "$JC" lint
  rm crates/graph-core/src/a.rs
  printf 'calls %s here\n' "$vendor" >notes.md
  has "vendor name" 1 'vendor-name +notes.md:1' "$JC" lint
  rm notes.md
  seq 301 >big.sh
  has "file over 300 lines" 1 'ERROR size +big.sh: 301 lines' "$JC" lint
  rm big.sh
  : >run.log
  has "junk file" 1 'ERROR junk +run.log' "$JC" lint
  rm run.log
  mkdir -p deploy/__pycache__ && : >deploy/__pycache__/m.cpython-313.pyc
  has "python bytecode is junk" 1 'ERROR junk +deploy/__pycache__/m.cpython-313.pyc' "$JC" lint
  rm -r deploy
  printf '// TODO later\n' >t.ts
  has "TODO without an issue" 1 'ERROR todo' "$JC" lint
  printf '// TODO later, see #12\n' >t.ts
  has "TODO with an issue" 0 'LINT 0 ERROR' "$JC" lint
  rm t.ts
  printf 'rand = "0.8"\n' >>crates/graph-core/Cargo.toml
  has "new graph-core dependency" 1 "ERROR dependency.*new 'rand'" "$JC" lint
  g checkout -q -- crates/graph-core/Cargo.toml
}

case_commit() {
  local sha
  printf 'fine\n' >feature.txt
  "$JC" start "$tmp/fail.rows" >/dev/null && "$JC" wait 60 >/dev/null
  has "commit refused on FAIL" 2 'not PASS' "$JC" commit
  gate "$tmp/pass.rows" >/dev/null
  printf 'more\n' >>feature.txt
  has "commit refused on STALE" 2 'tree changed' "$JC" commit
  : >junk.log
  gate "$tmp/pass.rows" >/dev/null
  has "commit refused on a lint ERROR" 2 'rule ERRORs' "$JC" commit
  rm junk.log
  gate "$tmp/pass.rows" >/dev/null
  has "commit on PASS pushes" 0 '^COMMITTED [0-9a-f]+ on feat, pushed$' "$JC" commit
  sha=$(git rev-parse HEAD)
  has "pushed to origin" 0 "$sha" git --git-dir="$tmp/origin.git" rev-parse feat
  has "author and message" 0 '^LESdylan <dev.pro.photo@gmail.com> updated$' git log -1 --format='%an <%ae> %s'
  has "state dir not committed" 0 '^$' git ls-files target
  g checkout -q develop
  has "commit refused on develop" 2 "refusing to commit on 'develop'" "$JC" commit
}

setup
case_gate
case_lint
case_commit
printf '%d passed, %d failed\n' "$pass" "$fail"
((fail == 0))
