# Two bundles of `<graph-studio>`, and the four rows that keep them honest

Status: **accepted** 2026-10-04. Follows `docs/reviews/review-bundle-unify.md` (verdict
PROCEED-WITH-CONDITIONS, option (c); branch `review-bundle-unify`, commit `30d456b9`). The review's
sections 0 and 5 are the facts this rests on.

## The decision

Keep both bundles. One is the service's (`app/vite.embed.config.ts`, entry
`app/src/embed-bundle.ts`), served under `/embed/<version>/`; the other is the pack's
(`app/vite.pack.config.ts`). Neither config nor entry is deleted, and no host's HTML, tag or
`<version>` changes.

The review's option (a) — the service stages the pack — is BLOCK, and not on cost. The service
bundle's entry auto-defines the element on import, which is what
`docs/deploy/service.md:98-99` and `deploy/nav/serviceproxy.py:34-35` both rely on, and the pack's
entry does not. Unifying them either breaks every existing host page silently (the element is
never defined, `whenDefined` never resolves, no `graph-error`) or reintroduces the hazard below.

## What each is for

- **The service bundle** — what a host page imports from its own origin, so the motor worker is
  same-origin (`docs/contract/host-api.md` condition 13). Minified, content-hashed as
  `<version>`, and the thing `scripts/service.sh` stages into the image.
- **The pack** — one self-contained, versioned tarball a host ships inside its own product: six
  flat, unhashed, auditable files, unminified, with a `pack.json` to check the download against
  (`docs/contract/packaging.md`).

They differ in audience, so they differ in shape. What they must agree on is the worker form and
the published file list, and that is what the rows assert.

## The four rows

| row | over | what it fails on |
|---|---|---|
| `embed-bundle` | the service bundle | a worker built from `blob:`/`data:`, or none at all |
| `negctl-worker-form` | `scripts/worker-form.sh` | the multi-line `new Worker` the `[^)]*` grep cannot see must PASS |
| `negctl-embed-files` | `scripts/embed-files.sh` | a file the document does not name, or a named one that is gone |
| `embed-sdk` (+ `negctl-embed-sdk`) | the built `graph-sdk.js` | a `graph_wasm.wasm` that will not load, named by error class |

The first two read the same predicate the pack already had; the review's finding was that it was
asserted over one of the two artifacts and silently unasserted over the one a host imports.

## The auto-define difference, and why (a) is still blocked

`app/src/embed-bundle.ts` calls `defineGraphStudio()` at import, so a bare `<script type="module">`
is enough. The pack's entry only exports it, and a host calls it itself
(`app/src/embed.ts:111`). These are not the same requirement, and auto-define is **not** a safe
superset: `packages/graph-studio/src/element.ts:159` returns early on the first definition, so a
pack entry that auto-defined would swallow the options a host passed to its own
`defineGraphStudio({...})` with no error at all.

Unifying them (option (a)) must first answer the review's condition 1, which offers two choices:
either (i) the pack's entry keeps not auto-defining, and `docs/deploy/service.md:98-99` plus
`deploy/nav/serviceproxy.py:34-35` change to import-and-call — a breaking change to every host's
HTML, released as one — or (ii) the pack's entry auto-defines, and then
`docs/contract/host-api.md:158` must gain the warning that a host's own options are discarded,
and `packages/graph-studio/tests/` must gain a case that fails when a second definition with
options is ignored. No row today can tell (i) from (ii): `app/src/embed.ts:111` passes no options.

Reopen when the pack gains a host that needs auto-define, or when `graph-sdk.js` is removed from
the service bundle — which changes the file set, therefore `<version>`, therefore every host's tag.