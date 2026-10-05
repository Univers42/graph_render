-- The hub's schema (spec §4, §5.3). Every workspace-keyed table carries a trigger; the other
-- four do not and must not, because they are not workspace state.
--
-- Values are stored as the qualified canonical text graph-contract's writer produces, never as
-- `jsonb`: `-0`, 2^53 - 1 and key order come back byte-identical. The primary keys that order a
-- scan name `COLLATE "C"` explicitly, so the order is byte order whatever the database's own
-- collation is.

CREATE TABLE workspaces (
  id text PRIMARY KEY CHECK (id ~ '^[a-z0-9][a-z0-9-]{0,62}$'),
  epoch bigint NOT NULL CHECK (epoch >= 0),
  head_seq bigint NOT NULL DEFAULT 0,
  doc_bytes bigint NOT NULL DEFAULT 0,
  created_at timestamptz NOT NULL DEFAULT clock_timestamp()
);

CREATE TABLE manifests (
  ws text NOT NULL REFERENCES workspaces(id),
  plugin text NOT NULL,
  version int NOT NULL,
  text text NOT NULL,
  text_bytes bigint NOT NULL,
  decl_bytes bigint NOT NULL,
  plugin_bytes bigint NOT NULL DEFAULT 0,
  plugin_seq bigint NOT NULL DEFAULT 0,
  PRIMARY KEY (ws, plugin)
);

CREATE TABLE records (
  ws text NOT NULL REFERENCES workspaces(id),
  plugin text NOT NULL,
  -- The collation is declared on the column, not in the PRIMARY KEY's column list: PostgreSQL
  -- rejects `COLLATE` there, and declaring it here makes the primary-key index byte-ordered too,
  -- which is what `records_scan_index_is_in_byte_order` reads out of `pg_get_indexdef`.
  qcoll text COLLATE "C" NOT NULL,
  id text COLLATE "C" NOT NULL,
  rev bigint NOT NULL CHECK (rev >= 1),
  updated_at int NOT NULL,
  text text NOT NULL,
  text_sha256 bytea NOT NULL,
  text_bytes bigint NOT NULL,
  PRIMARY KEY (ws, qcoll, id)
);

CREATE TABLE links (
  ws text NOT NULL,
  src_qcoll text NOT NULL,
  src_id text NOT NULL,
  field text NOT NULL,
  target_qcoll text NOT NULL,
  target_id text NOT NULL,
  PRIMARY KEY (ws, src_qcoll, src_id, field, target_qcoll, target_id)
);

CREATE TABLE change_headers (
  ws text NOT NULL,
  seq bigint NOT NULL,
  plugin text NOT NULL,
  at timestamptz NOT NULL,
  kind text NOT NULL CHECK (kind IN ('batch','manifest')),
  bytes bigint NOT NULL,
  ops int NOT NULL,
  PRIMARY KEY (ws, seq)
);

CREATE TABLE change_ops (
  ws text NOT NULL,
  seq bigint NOT NULL,
  ord int NOT NULL,
  op text NOT NULL CHECK (op IN ('upsert','delete')),
  qcoll text NOT NULL,
  id text NOT NULL,
  rev bigint NOT NULL,
  text text NOT NULL,
  PRIMARY KEY (ws, seq, ord)
);

CREATE TABLE idempotency (
  ws text NOT NULL,
  plugin text NOT NULL,
  key text NOT NULL,
  body_sha256 bytea NOT NULL,
  response text NOT NULL,
  seq bigint NOT NULL,
  created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (ws, plugin, key)
);

-- `hub_migrations` is deliberately NOT created here: the runner creates it before any file can
-- be recorded in it, so it has exactly one producer. `schema_has_exactly_the_named_tables` sees
-- all ten tables because the runner has made it by the time it reads them.

-- The anti-join reads `links` by source; the reverse index is what a target-side prune reads.
CREATE INDEX links_source ON links (ws, src_qcoll, src_id);
CREATE INDEX links_target ON links (ws, target_qcoll, target_id);
CREATE INDEX records_plugin_scan ON records (ws, plugin, qcoll, id);
CREATE INDEX change_ops_record ON change_ops (ws, qcoll, id);
CREATE INDEX idempotency_age ON idempotency (created_at);