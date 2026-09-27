#!/usr/bin/env node
// opencode-deny-test.mjs — proves what the opencode.json bash deny-list does and does not stop.
//
//   node scripts/opencode-deny-test.mjs      exit 0 every case behaves as listed · 1 one does not
//
// The matcher below is copied from opencode (sst/opencode, branch dev, fetched 2026-09-27):
// `match` from packages/core/src/util/wildcard.ts, and the rule choice from `evaluate` in
// packages/opencode/src/permission/index.ts — rules in config order, the LAST match wins.
// opencode parses a command with tree-sitter-bash and checks every `command` node on its own
// (packages/opencode/src/tool/shell.ts: commands(), source()), so each case below is the text
// of one such node. A newer opencode may match differently; re-fetch and re-run on upgrade.
//
// Ponytail: a deny-list is a speed bump, not a boundary. BYPASSES lists commands it is known
// not to stop; the enforcement for osionos is scripts/guard-osionos.sh, which fingerprints the
// tree and does not care how a write was spelled.

import { readFileSync } from "node:fs";

function match(input, pattern) {
  const normalized = input.replaceAll("\\", "/");
  let escaped = pattern
    .replaceAll("\\", "/")
    .replace(/[.+^${}()|[\]\\]/g, "\\$&")
    .replace(/\*/g, ".*")
    .replace(/\?/g, ".");
  if (escaped.endsWith(" .*")) escaped = escaped.slice(0, -3) + "( .*)?";
  return new RegExp("^" + escaped + "$", "s").test(normalized);
}

const rules = Object.entries(JSON.parse(readFileSync(new URL("../opencode.json", import.meta.url))).permission.bash);
const action = (command) => rules.findLast(([pattern]) => match(command, pattern))?.[1] ?? "ask";

const DENIED = [
  "git push", "git -C ../osionos push origin main", "git commit -m x", "git -C /x commit -am y",
  "git switch -c evil", "git -C /x switch main", "git checkout -b evil", "git checkout -- a.ts",
  "git merge dev", "git -C /x merge dev", "git reset HEAD~1", "git -C /x reset --hard",
  "git restore a.ts", "git clean -fdx", "git -C /x clean -fd", "git stash", "git -C /x stash pop",
  "git submodule update", "git -C /x submodule add u p", "git config user.name x",
  "git -C /x config core.hooksPath h", "git -c alias.c=commit c", "git -C /x -c k=v status",
  "git --git-dir=/x/.git commit", "git --work-tree=/x add .", "git -C /x add a", "git tag v1",
  "git branch -D main", "git -C /x update-ref refs/heads/x HEAD", "git rebase -i HEAD~2",
  "git cherry-pick abc", "git revert abc", "git remote add o u", "git fetch", "git pull",
  "git -C /x worktree add w", "git apply p.diff", "git am p.mbox", "git gc", "git init",
  "GIT_DIR=/x/.git git commit", "env git commit", "/usr/bin/git commit", "command git push",
  "bash -c 'git commit -m x'", "git --no-pager -C /x commit", "rm -rf x", "sudo ls", "gh repo create x",
];
const ALLOWED = [
  "git status", "git -C /x status", "git log --oneline", "git -C /x log -3", "git diff HEAD",
  "git show HEAD:a.ts", "git ls-files -z", "git rev-parse HEAD", "git -C /x diff --stat",
  "cargo test --workspace", "npm run check", "ls -la", "cat README.md",
];
// Known holes: each still reads "allow". Listed so the gap is declared, not discovered.
const BYPASSES = [
  "git \"commit\" -m x", "$g commit", "sh -c \"gi\"\"t commit\"",
  "git co", "xargs -a verbs.txt -n1 git",
];

let failures = 0;
const check = (list, want) => {
  for (const command of list) {
    const got = action(command);
    const ok = got === want;
    if (!ok) failures += 1;
    console.log(`  ${ok ? "ok  " : "FAIL"} ${want.padEnd(5)} ${got.padEnd(5)} ${command}`);
  }
};
check(DENIED, "deny");
check(ALLOWED, "allow");
check(BYPASSES, "allow");
console.log(`opencode-deny-test: ${failures === 0 ? "PASS" : `FAIL (${failures})`}`);
process.exit(failures === 0 ? 0 : 1);
