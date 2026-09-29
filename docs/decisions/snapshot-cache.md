# ADR — A snapshot cache is not persistence

Status: **proposed** (plan approved by the user 2026-09-29). A `devil` verdict is owed before
phase D2; until it is recorded no snapshot is written to Redis.
Decides the case `docs/contract/binary-layout.md` ("The version rule") leaves open: a minor
bump may break the payload "only while nothing persists snapshots".

## Context

Redis would hold snapshot bytes under `gm:v1:snap:{key}`. Read literally, that is a store
of snapshots, which would force declaring format 1.0 today.

## Decision

Cache entries are **disposable and never migrated**:

- `key = sha256("gm-cache/1" ‖ contract versions ‖ build id ‖ ingest sha256 ‖ stages with
  canonical params)`. A contract bump or a new build changes every key, so an entry written
  by another version is unreachable rather than misread.
- Redis runs `allkeys-lru` with persistence off; entries carry a 24 h TTL. Losing Redis loses
  time, never data: every value is rebuilt from PostgreSQL inputs.
- A reader that finds an entry still checks its sha256 before serving it.
- Nothing else may keep snapshot bytes: no table, no file, no browser storage.

The alternative is declaring 1.0 and freezing the layout. Rejected for now: phases 10 and 11
still change the format.

## Consequences

- The cache is exact, not approximate, because the motor is bit-identical: equal key, equal
  bytes. Row `cache-hit-identical` compares a miss and a hit with `cmp`.
- Negative control: a build with the params dropped from the key must turn that row red.
- The build id does not exist yet (phase M5); until it does the key cannot be built and D2
  is blocked on it.
