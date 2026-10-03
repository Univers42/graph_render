# Job fix-gates-hashgate-limits (agent build: house-limit debt left by fix-gates-hashgate)

Read `prompts/jobs/fix-common.md` first. Start only once `fix-gates-hashgate` is on develop (merge
`origin/develop` first and check `git log origin/develop` names it; if it does not, stop).
Source: the `findings:` lines of the fix-gates-hashgate return block, copied here.

| File | Debt |
|---|---|
| `crates/graph-cli/src/hashgate/knob/setting.rs` | 327 lines (> 300); `apply` is 76 lines (> 40) |
| `crates/graph-cli/src/hashgate/tests/knob/controls.rs` | 325 lines (> 300) |
| `crates/graph-cli/src/hashgate/report.rs` | `body`/`conclude`/`record` take 5/5/6 parameters (> 4); `conclude` is 42 lines |
| `crates/graph-cli/src/runner/resolve.rs:21` | doc says `None` is "this process's PATH", the code passes `""` |
| `runner.rs:139`, `hashgate/stages/checks.rs:66,81`, `hashgate/report.rs:77` | fallbacks with no `Caveat:`/`Ponytail:` marker |

A refactor: behaviour does not change. Split by concern into child modules (no `utils`), group
the report parameters into one struct, make the `resolve.rs` doc say what the code does (or the
code what the doc says, if a caller relies on it: check every caller and say which), and give
each fallback a marker naming what it gets wrong.

Paths: the files above and new child modules beside them.

Done when: every file ≤ 300 lines and every function ≤ 40 lines / ≤ 4 parameters in the table,
fmt/clippy/test green, `hashgate --seeds 8` 0, its `GM_MUTATE_REFERENCE_DEGREE=9` control non-zero,
and `git diff --stat` touches no test assertion (moved tests keep their names).
