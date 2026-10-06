# Hub API v1: graph-hub, the HTTP edge in front of the store and the motor

Status: **IN FLIGHT, 2026-10-06**. Slice 3 of
`docs/superpowers/specs/2026-10-05-graph-service-plugins-design.md` (revision 5.1) is landing as a
sequence of tasks on one branch, so no route set below is final until the orchestrator's gate run is
green. Every status in the Routes and Errors tables is copied from the test that asserts it and cited
as `tests/<file>.rs` `<test_name>`; a status with no asserting test says so in the last column rather
than borrowing the spec's word. The rows in `scripts/orch/rows/hub.rows` are the gate.

Sources in this tree (`hub-relay`, which carries tasks 1–5, 8, 9, 10) and on the local branch
`hub-api` (tasks 6 and 7: the reads, `/changes` and the SSE stream), which this document also
covers. Where the two branches disagree about a route's status, the branch that owns the handler is
named in the last column.

## What it is

One container image (`deploy/hub.Dockerfile`, `debian:trixie-slim` pinned by digest) holding one
binary, `graph-hub`, from the `server/` workspace. It is three things and never a fourth:

1. **The write path.** `PUT /v1/workspaces/{ws}`, `PUT …/plugins/{plugin}` and `POST …/batches` go
   through one transaction that also advances `doc_bytes`, prunes to `GRAPH_HUB_RETAIN`, and wakes
   the watchers. The answer is `{seq, applied}` plus a `Graph-Seq` header.
2. **The read path.** `/graph` streams the materialized document out of an ordered scan a page at a
   time; `/changes` pages the change log; `/records` pages and fetches by id. Nothing holds a whole
   document in memory (H14, `docs/superpowers/specs/2026-10-05-graph-service-plugins-design.md`).
3. **The relay.** `POST …/layout` hands the materialized document to `graph-server` and streams the
   snapshot back, byte for byte. The hub runs no motor math of its own; `relay/map.rs` is the only
   place graph-server's statuses are translated.

There is a fourth surface, `/healthz`, which is liveness only. The hub is **not** the motor: it
never imports `graph-wasm`, never links `probe` or `threads`, and `hub-breaks-off` proves it with
`cargo tree -i graph-server`. There is no embed tree, no CORS and no TLS here; the relay is the only
path from a document to a snapshot.

A browser never holds an API key. Every `/v1/` route needs one, and the hub serves no page.

## Routes

Thirteen paths, one row each, from `server/graph-hub/src/lib.rs` `router` (identical on both
branches). The last column is the test that asserts each status.

| Method, path | Grant | Request | Response | Statuses, and the test that asserts each |
|---|---|---|---|---|
| `GET /healthz` | none | — | `200` text `ok` | `200` — `tests/health.rs` `healthz_needs_no_key` (hub-api) |
| `GET /v1/meta` | any key | — | `200` `{api:1, version, limits:{…}}` | `200` — `tests/read/meta.rs` `meta_reports_every_limit_of_section_6` (hub-api) |
| `GET /v1/workspaces` | any key | — | `200` `{workspaces:[{id, epoch, head_seq}]}`, filtered to the granted ids | `200` — `tests/read/meta.rs` `the_workspaces_list_holds_only_what_the_key_may_read` (hub-api); `401`/`403`/`400` — `tests/authz.rs` `no_authorization_header_is_the_same_401_as_an_unknown_key`, `a_second_authorization_header_is_400` |
| `PUT /v1/workspaces/{ws}` | `admin` on `{ws}` | the create body | `201` on create, `200` when it exists | `201` then `200` — `tests/write.rs` `put_workspaces_is_201_then_200_and_takes_no_seq`; `403` — `tests/authz.rs` `a_read_only_key_is_403_on_every_write_route`; `503` + `Retry-After: 1` — `tests/write.rs` `every_write_route_takes_its_permit` |
| `PUT /v1/workspaces/{ws}/plugins/{plugin}` | `write:<plugin>` | the manifest, JSON | `201` on store, `200` on an identical re-PUT; the stored manifest back | `201`/`200` — `tests/write.rs` `put_manifest_is_201_then_200_and_the_same_content_takes_no_seq`; `409` `conflict` — `put_manifest_refuses_a_removed_field_with_409`, `put_manifest_refuses_the_same_version_with_other_content_with_409`; `413` `too_large` — `put_manifest_refuses_the_sixty_fifth_plugin_with_413`; `403` — `tests/authz.rs` `a_read_only_key_is_403_on_every_write_route`; `503` — `tests/write.rs` `every_write_route_takes_its_permit` |
| `GET /v1/workspaces/{ws}/plugins` | `read` | — | `200` `{manifests:[{plugin, …}]}` in plugin order | `200` — `tests/write.rs` `get_plugins_returns_every_manifest` |
| `POST /v1/workspaces/{ws}/plugins/{plugin}/batches` | `write:<plugin>` | the batch, JSON; `Idempotency-Key`, `If-Match` | `200` `{seq, applied}` + `Graph-Seq: <epoch>.<seq>` | `200` with `applied:1` — `tests/write.rs` `post_batch_answers_seq_and_applied_and_a_graph_seq_header`; `200` with `applied:0` — `an_identical_upsert_answers_applied_zero`; `412` — `if_match_gives_412_on_a_stale_plugin_seq`; `422` — `a_batch_with_one_bad_record_changes_nothing`, `the_same_key_with_another_body_is_422`, `an_idempotency_key_over_128_bytes_is_422`, `a_qualified_collection_in_a_body_is_422`, `a_nul_anywhere_in_a_body_is_422`; idempotent replay byte-identical — `idempotency_replay_returns_the_same_response_and_the_same_head_seq`; `403` — `tests/authz.rs` `a_read_only_key_is_403_on_every_write_route`; `503` — `tests/write.rs` `every_write_route_takes_its_permit` |
| `GET /v1/workspaces/{ws}/plugins/{plugin}/records?cursor=&limit=` | `write:<plugin>` or `read` | `limit` default 1 000, max 10 000, never 0; `cursor` opaque | `200` `{plugin_seq, records:[{collection, id, rev}], next}` in byte order, `next` `null` on the last page | `200` — `tests/write.rs` `a_record_page_is_in_byte_order_and_carries_plugin_seq`, `a_records_page_next_cursor_terminates`; `403` (not 404) for another plugin — `one_record_of_another_plugin_is_403_not_404`; `403` byte-equal to a missing plugin's — `tests/authz.rs` `a_key_learns_nothing_from_another_plugins_records` |
| `GET /v1/workspaces/{ws}/records/{plugin}/{collection}/{id}` | `read` | — | `200` `{id, collection, rev, values}` | `200` — `tests/write.rs` `one_record_is_200_with_its_rev`; `403` byte-equal to an absent record's — `one_record_of_another_plugin_is_403_not_404` |
| `GET /v1/workspaces/{ws}/graph` | `read` | `If-None-Match` | `200` the canonical ingest document, streamed, `ETag: "<epoch>.<seq>"`; `304` and no body on a match | `200` streamed — `tests/read/graph.rs` `graph_streams_the_document_with_an_e_tag`, `the_same_cursor_gives_the_same_bytes`, `graph_never_buffers_a_whole_document`, `graph_stops_at_the_stream_deadline`; `304` — `graph_is_304_on_a_matching_if_none_match`; `403` — `tests/authz.rs` `refusal_bytes_are_identical_with_and_without_the_workspace` |
| `GET /v1/workspaces/{ws}/changes?since=&limit=` | `read` | `since=<epoch>.<seq>`, `limit` | `200` `{changes:[…], next, epoch, bytes}`, ascending `seq`, never empty, at most `GRAPH_HUB_CHANGES_BYTES` | `200` — `tests/read/changes.rs` `changes_after_a_cursor_are_in_seq_order`, `a_changes_page_never_exceeds_the_byte_cap_but_holds_one_change`; `410` `cursor` — `changes_returns_410_for_a_cursor_from_another_epoch`, `changes_returns_410_for_a_cursor_below_what_is_kept`, `changes_returns_410_for_a_seq_above_head` (all hub-api) |
| `GET /v1/workspaces/{ws}/events` | `read` | `since=<epoch>.<seq>` or `Last-Event-ID`, `Accept: text/event-stream` | `200` an SSE stream: `event: change` with `id: <epoch>.<seq>`, `event: busy` with **no** `id:`, `event: resync`, `:` heartbeats | `429` on both subscriber caps — `tests/events/stream.rs` `the_per_key_subscriber_cap_is_429`, `the_total_subscriber_cap_is_429`; `429` absent after a `busy` close — `tests/events/ends.rs` `the_busy_slot_is_free_before_the_close` (all hub-api). The `200` status line itself is **not asserted by a test**: these cases read wire lines from the open stream |
| `POST /v1/workspaces/{ws}/layout?layout=&post=` | `read` | the plugin's `read` grant; `Accept` passed through | `200` graph-server's snapshot, streamed unchanged, + `Graph-Seq: <epoch>.<seq>`; never retried | `200` byte-equal to graph-server over three fixtures — `tests/relay/roundtrip.rs` `layout_bytes_equal_motor_bytes_at_the_same_cursor`; `Graph-Seq` == `/graph`'s `ETag` — `tests/relay/shape.rs` `the_graph_seq_header_equals_the_graph_etag`; streamed, never a whole document — `layout_never_holds_a_whole_document`; the snapshot closes before the answer is awaited — `the_snapshot_closes_before_the_motor_answer_is_awaited`; `422` relayed — `a_layout_failure_is_relayed_with_the_motors_error`; `503` relayed with no `Retry-After` — `a_layout_is_never_retried` |
| anything else, or a wrong method on a registered path | — | — | `404` `NotFound`, JSON shape | `404` — `tests/health.rs` `an_unknown_route_is_the_json_404_shape` (hub-api). On `hub-relay` the same fallback is only constructed, not routed: `tests/limits/refusals.rs` `the_refusal_headers_come_from_the_status_and_not_the_variant` |
| `POST /v1/workspaces` | — | — | `501` `NotImplemented` — registered and authorized, handler unwritten | **not asserted by a test**. It is outside §5.2's table; `src/lib.rs` `not_ready` answers for it while its own task is open |

**Order of refusals.** `src/auth.rs` `authorize_middleware` runs before every handler, so no route
learns anything from a gate being full or a workspace existing: no or unknown key is `401`, a key
with no grant on the path's workspace and plugin is `403`, and only then may a `404` be reached.
`tests/authz.rs` `no_grant_is_403_before_any_404` and
`refusal_bytes_are_identical_with_and_without_the_workspace` assert both, and compare the two
refusal bodies byte for byte.

## Errors

Every error body is graph-server's shape: `{"error": "<code>", "message": "<one line>"}`. The `message`
is one-lined and capped at 256 B. The codes below are the hub's own; graph-server's
`server/graph-server/src/error.rs` is untouched, and a relayed body keeps the motor's own `error`
string.

| Status | `error` | When | Test |
|---|---|---|---|
| 400 | `BadRequest` | a bare seq cursor, a `%00` in a path id, a second `Authorization` header, a malformed `If-Match` or `limit` | `tests/authz.rs` `a_second_authorization_header_is_400`, `a_percent_encoded_nul_in_a_path_id_is_400`; `tests/limits/reader.rs` `a_body_that_cannot_be_read_is_400` |
| 401 | `Unauthorized` | no key, or a key matching no stored hash; `WWW-Authenticate: Bearer`. The body never says which | `tests/authz.rs` `no_authorization_header_is_the_same_401_as_an_unknown_key`, `a_non_bearer_credential_is_401`; `tests/limits/refusals.rs` `a_401_carries_www_authenticate_and_a_503_does_not` |
| 403 | `Forbidden` | no grant covering the workspace and plugin; `"the key has no grant for this workspace and plugin"`. A key with no grant at all is denied by default | `tests/authz.rs` `no_grant_is_403_before_any_404`, `a_read_only_key_is_403_on_every_write_route`; `tests/write.rs` `one_record_of_another_plugin_is_403_not_404` |
| 404 | `NotFound` | no such route, a wrong method, or a missing store object **after** authorization said yes | `tests/health.rs` `an_unknown_route_is_the_json_404_shape` (hub-api) |
| 406 | `NotAcceptable` | graph-server's own `Accept` refusal, relayed with its message | `tests/motor_map/rows.rs` `motor_406_is_relayed_as_406` |
| 408 | `Timeout` | the request body did not arrive within `GRAPH_HUB_BODY_TIMEOUT_MS` | `tests/limits/reader.rs` `a_body_that_stalls_past_the_body_timeout_is_408` |
| 409 | `conflict` | a manifest that would remove a field, or the same `manifestVersion` with other content | `tests/write.rs` `put_manifest_refuses_a_removed_field_with_409`, `put_manifest_refuses_the_same_version_with_other_content_with_409` |
| 410 | `cursor` | `since` is from another epoch, below what is kept, or above `head_seq` | `tests/read/changes.rs` `changes_returns_410_for_a_cursor_from_another_epoch`, `changes_returns_410_for_a_cursor_below_what_is_kept`, `changes_returns_410_for_a_seq_above_head` (hub-api) |
| 412 | `PreconditionFailed` | `If-Match` is not the plugin's `plugin_seq` | `tests/write.rs` `if_match_gives_412_on_a_stale_plugin_seq` |
| 413 | `too_large` | over any §6 cap: `over the {what} limit of {limit}` | `tests/limits/reader.rs` `a_body_over_the_cap_is_413_without_reading_a_byte`, `a_chunked_body_over_the_cap_is_413_mid_stream`, `a_413_names_its_limit`; `tests/write.rs` `put_manifest_refuses_the_sixty_fifth_plugin_with_413` |
| 422 | `invalid` | a wire fault: one undeclared collection, a qualified collection, a NUL, an idempotency key over 128 B, a malformed path id | `tests/write.rs` `a_batch_with_one_bad_record_changes_nothing`, `a_qualified_collection_in_a_body_is_422`, `a_nul_anywhere_in_a_body_is_422`, `an_idempotency_key_over_128_bytes_is_422`; `tests/authz.rs` `a_malformed_path_id_is_422_with_a_contract_class` |
| 429 | `Busy` | a subscriber cap, per key or in total; `Retry-After: 1` | `tests/limits/refusals.rs` `the_subscriber_cap_per_key_is_429`, `the_subscriber_cap_in_total_is_429`; `tests/events/stream.rs` `the_per_key_subscriber_cap_is_429`, `the_total_subscriber_cap_is_429` (hub-api) |
| 431 | — | hyper's own answer for a head over `GRAPH_HUB_MAX_HEADER_BYTES`, with no JSON body | `tests/limits/reader.rs` `a_header_over_the_cap_is_431` (raw status line) |
| 500 | `internal` | a hub defect: a `StoreError::Db`/`Eof`, or a panic | **not asserted by a test** |
| 501 | `NotImplemented` | a registered, authorized route whose handler its own task has not written | **not asserted by a test** (see Routes) |
| 502 | `MotorAuth` | the motor refused the hub's own key. A hub defect, logged `motor-fault` | `tests/motor_map/rows.rs` `motor_401_is_502_motor_auth_and_logged` |
| 502 | `MotorBodyTimeout` | the upload missed graph-server's 10 s body timeout. No `Retry-After`: the same request would miss it again | `tests/motor_map/rows.rs` `motor_408_is_502_motor_body_timeout_with_no_retry_after` |
| 502 | `MaterializeInvalid` | graph-server answered 422 with `IngestInvalid` or `ContractInvalid`. A hub defect, logged | `tests/motor_map/rows.rs` `motor_422_ingest_invalid_is_502_materialize_invalid`, `motor_422_contract_invalid_is_502_materialize_invalid` |
| 502 | `MotorError` | graph-server answered 500, or any status §5.2 does not map (404, 405, …). Logged | `tests/motor_map/rows.rs` `motor_500_is_502_motor_error`, `motor_404_is_502_motor_error_and_logged` |
| 502 | `MotorUnavailable` | the motor was unreachable, or silent past `GRAPH_HUB_MOTOR_TIMEOUT_MS` | `tests/motor_map/rows.rs` `a_motor_that_never_answers_is_502_motor_unavailable` |
| 503 | `Busy` | a permit or a pool connection waited out `GRAPH_HUB_TIMEOUT_MS`; `Retry-After: 1` | `tests/limits/refusals.rs` `a_pool_wait_past_the_timeout_is_503_with_retry_after`, `a_permit_wait_past_the_timeout_is_503_not_500`, `a_503_carries_retry_after_and_the_json_shape` |
| 503 | `Timeout` | graph-server's own 503, relayed with its `error` and **no** `Retry-After` | `tests/motor_map/rows.rs` `motor_503_is_relayed_as_503_with_no_retry_after` |

graph-server's `400`, `406`, and `422 LayoutFailed`/`PostFailed` are relayed as themselves with
graph-server's `error` string and body (`tests/motor_map/rows.rs` `motor_400_is_relayed_as_400`,
`motor_422_layout_failed_is_relayed_as_422`, `motor_422_post_failed_is_relayed_as_422`,
`a_relayed_body_keeps_the_motors_error_string`), and its `413` becomes `413 GraphTooLarge` with the
motor's message kept (`motor_413_is_413_graph_too_large_with_the_motors_message`). Its `429` becomes
the hub's own `503` with `Retry-After: 1`, and a wait past the hub's own timeout is `503 Busy`
before the motor is called at all (`motor_429_is_503_with_retry_after`,
`a_pool_wait_past_the_timeout_is_503_before_the_motor_is_called`).

## API keys

The hub has **its own** key set, separate from graph-server's, and it never stores a key.

- `GRAPH_HUB_KEYS_FILE` holds one line per key, `<name> <sha256-hex>`, `#` starts a comment, blank
  lines are skipped. A name is 1–64 of `A-Z a-z 0-9 . _ -`. A duplicate name or hash is a load
  failure. The file must be `0640` or stricter, must be a regular file, at most 1 MiB, and UTF-8.
- `GRAPH_HUB_GRANTS_FILE` holds `<key-name> <ws|*> <read|write:<plugin>|admin>`. `admin` implies
  `read` and every `write`, and is scoped to one workspace. Same mode rule, same 1 MiB ceiling. A
  malformed line fails the load; it is never skipped. An empty grants file is itself a refusal
  (`holds no grant`).
- A key with **no grant is denied**: `Grants::of` returns nothing and every route answers `403`. The
  hub has no auth-off mode; an empty or missing file refuses to start (exit 2).
- A key is sent as `Authorization: Bearer <key>`. The scheme is case-insensitive; the parsing is
  `graph_server::auth::bearer`, made `pub` for this and nothing else. `scripts/hub.sh keygen NAME
  [KEYFILE]` mints one through graph-server's own generator, appends `<name> <sha256-hex>` at mode
  `0640`, and prints the key on stdout once.
- On `SIGHUP` the hub reads **both** files and swaps the pair as one `RwLock<Arc<(KeySet, Grants)>>`.
  If either fails, the old pair stays. A reload whose bytes are identical is not a swap.
  `tests/reload.rs` `a_sighup_with_a_bad_grants_file_keeps_the_old_pair`,
  `a_sighup_with_a_bad_keys_file_keeps_the_old_pair`, `a_good_pair_swaps_both`,
  `an_unchanged_pair_is_not_a_swap`, `an_empty_keys_file_is_refused_and_keeps_the_old_pair`.
- Logs carry the key's **name**, never the key or its hash.

## Limits and scheduling

The 32 names in `src/config/env.rs` `NAMES` are the five unbounded ones, §6's twenty-six and one more, and
`tests/start/settings.rs` `every_hub_env_name_is_read` asserts the count and that a nonsense value is
refused by name. An empty value counts as unset; a non-UTF-8, malformed or out-of-range value refuses
the start (exit 2) naming the variable and never its value. The start line prints each of those 32
`set` or `unset`.

`GRAPH_HUB_TIMEOUT_MS` is the last of the 32: §6 names it inside the `GRAPH_HUB_DB_POOL` row rather
than in a row of its own, and `every_hub_env_name_is_read` pins the count.

| Variable | Default | Range | What it does | Over the limit |
|---|---|---|---|---|
| `GRAPH_HUB_BIND` | `127.0.0.1` (`0.0.0.0` in the image) | an `IpAddr` | the listen address | refuses to bind |
| `GRAPH_HUB_PORT` | `8080` | 0–65535 | the listen port; `0` asks the kernel | — |
| `GRAPH_HUB_DB_URL` | unset | a URL | the store | a start check refuses it |
| `GRAPH_HUB_DB_POOL` | `8` | 1–1024 | store connections | a start check refuses a pool at or below `WRITERS + READS + LAYOUTS`; past it, 503 after `GRAPH_HUB_TIMEOUT_MS` |
| `GRAPH_HUB_KEYS_FILE` | unset | a path | the hub's own keys | unset refuses the start |
| `GRAPH_HUB_GRANTS_FILE` | unset | a path | the grants | unset refuses the start |
| `GRAPH_HUB_MOTOR_URL` | `http://127.0.0.1:8080` | a URL | graph-server, for `/layout` | 502 `MotorUnavailable` |
| `GRAPH_HUB_MOTOR_KEY_FILE` | unset | a path | the plaintext motor key, one line | a refused motor key is 502 `MotorAuth` |
| `GRAPH_HUB_MAX_BODY` | `4194304` (4 MiB) | 1 B–16 GiB | the largest request body, chunked included | 413 `too_large`, before a byte is read |
| `GRAPH_HUB_MAX_BATCH` | `10000` operations | 1–1 000 000 | operations in one batch | 413 |
| `GRAPH_HUB_MAX_RECORD_BYTES` | `1048576` | 1 B–16 GiB | one record's canonical text | 413 |
| `GRAPH_HUB_MAX_PLUGIN_BYTES` | `16777216` (16 MiB) | 1 B–16 GiB | one plugin's stored records | 413 on the batch that would cross it |
| `GRAPH_HUB_MAX_DOC_BYTES` | `67108864` (64 MiB) | 1 B–16 GiB | one workspace's whole document | 413 on the batch that would cross it |
| `GRAPH_HUB_RETAIN` | `100000` changes | 1–1 000 000 000 | how many changes are kept per workspace | older changes pruned in the commit's transaction |
| `GRAPH_HUB_RETAIN_BYTES` | `536870912` (512 MiB) | 1 B–16 GiB | how many bytes of changes are kept | below one max change, the start refuses |
| `GRAPH_HUB_CHANGES_BYTES` | `8388608` (8 MiB) | 1 B–16 GiB | one `/changes` page | the page ends; never less than one change |
| `GRAPH_HUB_FETCH_ROWS` | `4096` | 1–65 536 | rows per document page; the page is also cut at `GRAPH_HUB_CHANGES_BYTES` of row cost | — |
| `GRAPH_HUB_LAST_SEEN` | `65536` entries | 1–16 777 216 | the last-seen map | the least recently used entry is evicted |
| `GRAPH_HUB_WRITERS` | `2` | 1–1024 | batches, manifest PUTs and creates in flight | waits, then 503 with `Retry-After: 1` |
| `GRAPH_HUB_WRITERS_PER_KEY` | `1` | 1–1024 | `WRITERS` permits one key holds at once | waits, then 503 |
| `GRAPH_HUB_READS` | `2` | 1–1024 | `/graph`, `/changes`, records pages, one record, `GET /plugins`, `/workspaces` at once | waits, then 503 |
| `GRAPH_HUB_LAYOUTS` | `1` | 1–1024 | `/layout` at once | waits, then 503 |
| `GRAPH_HUB_MAX_SUBSCRIBERS` | `64` | 1–65 536 | SSE streams in total | 429 with `Retry-After: 1` |
| `GRAPH_HUB_MAX_SUBSCRIBERS_PER_KEY` | `8` | 1–65 536 | SSE streams one key holds | 429 |
| `GRAPH_HUB_SSE_PAGE` | `256` headers | 1–65 536 | change headers per store read on a stream | the page ends; the next read continues from the last seq sent |
| `GRAPH_HUB_MAX_CONNECTIONS` | `256` | 1–65 536 | open connections | the next waits in the backlog, never refused |
| `GRAPH_HUB_HEADER_TIMEOUT_MS` | `5000` | 1–600 000 | receiving the request head | the connection is closed |
| `GRAPH_HUB_MAX_HEADER_BYTES` | `16384` | 8192–1 MiB | hyper's read buffer, so the largest head | 431 from hyper, no JSON body |
| `GRAPH_HUB_BODY_TIMEOUT_MS` | `10000` | 1–600 000 | reading the whole body | 408 |
| `GRAPH_HUB_TIMEOUT_MS` | `30000` | 1–600 000 | a permit or pool wait, and the shutdown drain | 503 with `Retry-After: 1` |
| `GRAPH_HUB_STREAM_DEADLINE_MS` | `120000` | 1–600 000 | a `/graph` or `/layout` body | the connection is cut mid-body |
| `GRAPH_HUB_MOTOR_TIMEOUT_MS` | `45000` | 1–600 000 | the `/layout` relay | at or below 40 000 the start refuses; past it, 502 `MotorUnavailable` |

**Scheduling.** Every route takes a permit before it reads anything: the three write routes and
`PUT /v1/workspaces` under `WRITERS`, `/graph` `/changes` `/records` `/plugins` `GET /workspaces`
under `READS`, `/layout` under `LAYOUTS`, and `/events` under a subscriber counter rather than a
semaphore. A wait that runs past `GRAPH_HUB_TIMEOUT_MS` is `503` with `Retry-After: 1`, never `429`
and never `500` (`tests/limits/gates.rs` `writers_two_and_a_third_waits_then_gets_503_with_retry_after`,
`readers_two_and_a_third_waits_then_503`, `layouts_one_and_a_second_waits_then_503`,
`a_second_batch_from_one_key_waits_on_writers_per_key_while_another_keys_batch_proceeds`).

**Start checks** (`src/config/check.rs`, `tests/start/refusals.rs`) and the operator's remedy for
each are in `docs/deploy/hub.md`.

**Memory.** The budget is §6's formula with `F_w` = 52 (measured worst 50.92) and the last-seen entry
at 272 B (measured 263.8 B), which is about 797 MiB at the defaults; the container peak is
404 580 KiB under `--memory 1g`. Caveat: that is arithmetic over a measured `F_w`, not a measurement
of the whole process, and the reads, layouts and subscribers terms are arithmetic only. The ledger
and the commands are `docs/measurements/hub-memory.md`.

## Headers

- `ETag` on `/graph`, and the answer `If-None-Match` is compared against: `"<epoch>.<seq>"` — a
  **position**, not a byte range and not a content hash. `src/etag.rs` refuses a weak comparison,
  so `W/"…"` never gets a 304.
- `Graph-Seq: <epoch>.<seq>` on `POST …/batches` and on `POST …/layout`, the same position the
  matching `/graph` `ETag` would carry.
- `If-Match` on `POST …/batches`, compared to the plugin's `plugin_seq`; a stale one is `412`. A
  malformed one is `400`, not an absent header. It is passed through, never rewritten.
- `Idempotency-Key` on `POST …/batches`: at most 128 B, the same key with another body is `422`, and
  a replay returns the byte-identical response and the same head seq. The hub never hashes it away.
- `Last-Event-ID` on `GET …/events` beats `?since=`, and both are a cursor `<epoch>.<seq>`.
- `Retry-After: 1` on `429` (a subscriber cap) and on `503` from `busy_wait`. **Not** on a relayed
  graph-server `503` or `408`, and not on a 502: those are not retried by this hub's caller either.
  `tests/limits/refusals.rs` `the_refusal_headers_come_from_the_status_and_not_the_variant` is the
  table.
- `WWW-Authenticate: Bearer` on `401`, and on nothing else
  (`a_401_carries_www_authenticate_and_a_503_does_not`).
- A `%00` in any path id is `400` before authorization
  (`tests/authz.rs` `a_percent_encoded_nul_in_a_path_id_is_400`).

**Statuses §5.2 gives a route that no test asserts.** `GET /v1/meta` 401/403, `PUT
/v1/workspaces/{ws}` 401 and 404, `GET …/plugins` 401/403/404, `POST …/batches` 401/404, `GET
…/records` (page and one record) 401, `GET /graph` 401/404, `GET …/changes` 401/403/404, `GET
…/events` 401/403/404 and its own `200` status line, `POST …/layout` 401/403/404, `500 internal`,
and `501 NotImplemented`. The refusals these would use are the same
`HubApiError` variants the asserted routes use, and the middleware is one layer over the whole
`/v1` prefix, so a reader should treat them as covered by construction rather than by evidence.