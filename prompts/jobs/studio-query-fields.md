# Job studio-query-fields (agent build: two generic query fields, `group:` and `version:`)

Why: a user who opens any source with authors or owners (a group) and record versions (the
ingest contract's `updatedAt`, Unix seconds) cannot write:
- "only this group" as a query, so it cannot be a filter, search or colour group;
- "records updated after a date".

The query grammar has `id tag kind db path degree` and nothing else. This adds two fields to the
one grammar that search, filters and groups share. It touches the studio only. No motor, SDK or
wire change: `version` is already a node member of every document the studio opens.

The hub owner (session graph-render-0e) confirmed on 2026-10-05:
- no job of theirs touches `packages/graph-studio/src/console/**` or `src/actions/**`;
- no objection to the names;
- `version:` maps onto the wire's `updatedAt` (u32 seconds).

Facts (develop, 2026-10-05; re-check each on your branch before editing, and stop if one no
longer holds):
- `packages/graph-studio/src/console/queryParse.ts` (247 lines):
  - `QueryField` (`:13`) and `FIELDS` (`:34-41`);
  - `OPERATORS` and `DEGREE_OPS` (`:44-45`);
  - `WHOLE` (`:47`);
  - `operator()` (`:97-110`), where `degree` is the only field that needs an operator;
  - `whole()` (`:113-115`);
  - `readField()` (`:146-155`).
- `packages/graph-studio/src/console/queryMatch.ts` (85 lines):
  - `QueryRow` (`:15-23`);
  - `rowOf()` (`:31-41`);
  - `compares()` (`:52-59`), whose parameter is named `degree`;
  - `matchesField()` (`:66-76`);
  - the header comment (`:1-10`), which states every rule.
- `packages/graph-studio/src/source/meta.ts` (112 lines):
  - `GraphMeta` (`:11-30`) has `groups` (names, first seen first, `:17`) and `group` (a
    `Uint16Array` index per node, `:19`);
  - past `MAX_GROUPS = 4096` names (`:40`), every further name shares `OTHER_GROUPS` (
    `groupIndex` `:63-70`);
  - `metaOf()` (`:88-112`) fills the columns from `IngestNode`s;
  - `IngestNode.version` is a `number`, filled `0` when absent
    (`src/source/ingest.ts:205`; `motor/documents.ts` `columnNodes` sets it from `rows.versions`).
- Tests: `packages/graph-studio/tests/query.test.ts` (parse), `query-match.test.ts` (match and
  masks, built on a hand-made graph) and `query-roundtrip.test.ts` (print → parse).
  `tests/desk.ts` and `tests/reveal-data-path.test.ts` build `GraphMeta`.

Change, exactly:
1. **`meta.ts`.**
   - Add `readonly versions: Float64Array;` to `GraphMeta`, with the doc line "Each node's
     `version` (a source's `updatedAt`); 0 when the document carries none."
   - In `metaOf`, allocate it beside `weight` and fill it in the same loop:
     `versions[i] = node.version;`.
2. **`queryParse.ts`.**
   - `QueryField` gains `"group" | "version"`, and `FIELDS` gains both entries.
   - Replace the `field === "degree"` checks in `operator()` with
     `NUMERIC.has(field)`, where `const NUMERIC: ReadonlySet<QueryField> = new Set(["degree", "version"]);`.
     The two messages name the field: `` `${field}:` needs an operator: one of > < >= <= = ``.
     Rename `DEGREE_OPS` to `COMPARE_OPS`.
   - In `readField`, keep `whole()` for `degree`. For `version`, add `finite(value, op, at)`,
     which refuses with `` `version:` needs a number after `${op}` `` unless the value matches
     `/^-?[0-9]+(\.[0-9]+)?$/`.
   - The file must stay ≤ 300 lines.
3. **`queryMatch.ts`.**
   - `QueryRow` gains `readonly group: string` and `readonly version: number`.
   - `rowOf` sets `group: meta.groups[meta.group[node] ?? 0] ?? ""` and
     `version: meta.versions[node] ?? 0`.
   - Rename `compares`'s first parameter to `given`.
   - In `matchesField`, add `if (field === "version") return compares(row.version, query.op, query.value);`
     and `case "group": return same(row.group, query.value);`.
   - Extend the header's rule list: "`group` is case-insensitive equality on the group's name;
     `version` is numeric, like `degree`."
   - Add above `rowOf` a `Caveat:` line: past 4096 distinct groups, every further name reads
     as `(other groups)`, so `group:` cannot tell those apart. That is under-matching. The escape
     hatch is a `tag:` or `id:` query.
4. **Every other place** that builds a `QueryRow` or a `GraphMeta` gets the new members. Find
   them with `scripts/studio.sh check`, which runs tsc, and fix only what it names.
5. **`queryPrint.ts` and `complete.ts`.** If either lists field names or special-cases `degree`,
   give `version` the same treatment and `group` the string-field treatment. Otherwise leave
   them.

Tests, in the files' own style. Each must fail before the change:
- `query.test.ts`:
  - `version:>=1700000000` parses to `{kind:"field", field:"version", op:">=", value:"1700000000"}`;
  - `group:Ana` parses with `op: ""`;
  - `version:abc` is refused naming column 1 + the length of `version:`;
  - `version:5` (no operator) is refused with the "needs an operator" message;
  - `version:>1.5` is accepted.
- `query-match.test.ts`, on the existing hand-made graph:
  - give its nodes distinct versions and at least two groups (plus one ungrouped);
  - `version:>=N` and `version:<N` split them as expected;
  - `group:ana` matches case-insensitively;
  - `group:"(no group)"` matches the ungrouped node;
  - `NOT group:ana AND version:>N` combines.
- `query-roundtrip.test.ts`: `version:>=1700000000`, `group:"Ana B"` and
  `group:ana OR version:<5` survive print → parse.

Rules (beyond `scripts/orch/common.md`):
- Studio code: no type assertions (`as`), no `any`, no `eslint-disable`.
- `scripts/studio.sh` is the only way to type-check, lint, test and build.
- Paths you may touch: `packages/graph-studio/src/console/**`, `packages/graph-studio/src/source/meta.ts`,
  `packages/graph-studio/tests/**`, and the files tsc names under step 4. Nothing else.
  `scripts/orch/rows/studio-query-fields.rows` is already on develop: do not edit it.

Done when:
- `scripts/orch/gate.sh target/rows-studio-query-fields scripts/orch/rows/studio-query-fields.rows`
  writes a `summary.txt` with every row PASS.

Return: the branch tip, the files changed with their line counts, every test added (name → the
assertion that failed before the change), each row's result, and every deviation.
