-- The epoch clock, the database's identity, the one function that draws an epoch, and the
-- triggers that draw one for writes the hub did not make (spec H15, §5.3).
--
-- WHY microseconds: `last = greatest(last + 1, clock_timestamp() * 1e6)` puts an epoch near
-- 1.79e15 in 2026, below `2^53 - 1` and so exactly representable on the wire. A millisecond
-- reading would put it near 1.8e12 and, worse, would let two hubs draw the same epoch after a
-- restore. The run-ahead is microseconds too: N epochs drawn inside one microsecond leave `last`
-- N microseconds ahead. Pinned by `epoch_is_microseconds` and `epoch_run_ahead_is_microseconds`.

CREATE TABLE epoch_clock (
  one bool PRIMARY KEY CHECK (one),
  last bigint NOT NULL
);

INSERT INTO epoch_clock(one, last) VALUES (true, 0);

CREATE TABLE hub_meta (
  one bool PRIMARY KEY CHECK (one),
  system_identifier bigint NOT NULL,
  timeline text NOT NULL,
  datoid oid NOT NULL
);

-- One function, not four: the trigger functions call this once per distinct workspace.
CREATE OR REPLACE FUNCTION hub_next_epoch() RETURNS bigint LANGUAGE sql AS $$
  UPDATE epoch_clock SET last = greatest(last + 1,
    (extract(epoch FROM clock_timestamp()) * 1000000)::bigint)
  RETURNING last
$$;

-- ---------------------------------------------------------------------------
-- The four trigger functions (spec §5.3).
--
-- WHY one function per event: PostgreSQL 17 refuses a transition table on a trigger that names
-- more than one event, and a row-level trigger would fire once per row instead of once per
-- statement — which would draw one epoch per record for a single COPY.
--
-- WHY the guard is on every one of them: `hub.writer = '1'` is how the hub's own write paths say
-- "I know what I am doing, do not bump"; `pg_trigger_depth() > 1` covers the operator's own SQL,
-- where a trigger's own write to `workspaces` would otherwise draw a second epoch for the same
-- event (spec §5.3 runbook: two epochs per workspace at depth 0, harmless but pointless).
--
-- WHY the workspace column arrives as TG_ARGV[0] rather than being hard-coded: the six trigger
-- tables name it differently (`workspaces` uses `id`), and one function serving all six is what
-- keeps the guard identical across them.
-- ---------------------------------------------------------------------------

CREATE OR REPLACE FUNCTION hub_bump_insert() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
  ws_col text := TG_ARGV[0];
BEGIN
  IF current_setting('hub.writer', true) = '1' OR pg_trigger_depth() > 1 THEN
    RETURN NULL;
  END IF;
  -- One epoch per DISTINCT workspace, not per row: a batch of 10 000 records for one workspace
  -- is one epoch, not ten thousand.
  PERFORM hub_next_epoch() FROM (SELECT DISTINCT to_jsonb(n) ->> ws_col AS id FROM n) s;
  RETURN NULL;
END
$$;

CREATE OR REPLACE FUNCTION hub_bump_update() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
  ws_col text := TG_ARGV[0];
BEGIN
  IF current_setting('hub.writer', true) = '1' OR pg_trigger_depth() > 1 THEN
    RETURN NULL;
  END IF;
  -- Both tables: a row moved from one workspace to another is an event for both of them.
  PERFORM hub_next_epoch() FROM (
    SELECT DISTINCT id FROM (
      SELECT to_jsonb(n) ->> ws_col AS id FROM n
      UNION
      SELECT to_jsonb(o) ->> ws_col AS id FROM o
    ) u
  ) s;
  RETURN NULL;
END
$$;

CREATE OR REPLACE FUNCTION hub_bump_delete() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
  ws_col text := TG_ARGV[0];
BEGIN
  IF current_setting('hub.writer', true) = '1' OR pg_trigger_depth() > 1 THEN
    RETURN NULL;
  END IF;
  PERFORM hub_next_epoch() FROM (SELECT DISTINCT to_jsonb(o) ->> ws_col AS id FROM o) s;
  RETURN NULL;
END
$$;

-- WHY every workspace, not none: TRUNCATE takes no transition table, and a truncated table has
-- no rows to name one. Every workspace loses state, so every workspace draws a fresh epoch.
CREATE OR REPLACE FUNCTION hub_bump_truncate() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  IF current_setting('hub.writer', true) = '1' OR pg_trigger_depth() > 1 THEN
    RETURN NULL;
  END IF;
  PERFORM hub_next_epoch() FROM workspaces;
  RETURN NULL;
END
$$;

-- ---------------------------------------------------------------------------
-- The 24 triggers: four per trigger table, one per event.
--
-- `workspaces` is keyed by `id`; the other five by `ws`. No trigger on `epoch_clock`,
-- `hub_meta`, `idempotency` or `hub_migrations`: none of those is workspace state, and a bump
-- from the sweeper's own idempotency delete would move an epoch for no reason.
--
-- WHY `ENABLE ALWAYS` is a separate `ALTER TABLE`: `CREATE TRIGGER` has no such clause. Plain
-- `ENABLE` is skipped when `session_replication_role = replica`, which would let every
-- replication-slave write move no epoch — `replica_role_write_moves_the_epoch_for_every_event`
-- is the test that catches that, and `negctl-one-trigger-origin` the control that breaks it.
-- ---------------------------------------------------------------------------

CREATE TRIGGER hub_workspaces_ins AFTER INSERT ON workspaces
  REFERENCING NEW TABLE AS n FOR EACH STATEMENT EXECUTE FUNCTION hub_bump_insert('id');
CREATE TRIGGER hub_workspaces_upd AFTER UPDATE ON workspaces
  REFERENCING OLD TABLE o NEW TABLE AS n FOR EACH STATEMENT EXECUTE FUNCTION hub_bump_update('id');
CREATE TRIGGER hub_workspaces_del AFTER DELETE ON workspaces
  REFERENCING OLD TABLE AS o FOR EACH STATEMENT EXECUTE FUNCTION hub_bump_delete('id');
CREATE TRIGGER hub_workspaces_trunc AFTER TRUNCATE ON workspaces
  FOR EACH STATEMENT EXECUTE FUNCTION hub_bump_truncate('id');

CREATE TRIGGER hub_manifests_ins AFTER INSERT ON manifests
  REFERENCING NEW TABLE AS n FOR EACH STATEMENT EXECUTE FUNCTION hub_bump_insert('ws');
CREATE TRIGGER hub_manifests_upd AFTER UPDATE ON manifests
  REFERENCING OLD TABLE o NEW TABLE AS n FOR EACH STATEMENT EXECUTE FUNCTION hub_bump_update('ws');
CREATE TRIGGER hub_manifests_del AFTER DELETE ON manifests
  REFERENCING OLD TABLE AS o FOR EACH STATEMENT EXECUTE FUNCTION hub_bump_delete('ws');
CREATE TRIGGER hub_manifests_trunc AFTER TRUNCATE ON manifests
  FOR EACH STATEMENT EXECUTE FUNCTION hub_bump_truncate('ws');

CREATE TRIGGER hub_records_ins AFTER INSERT ON records
  REFERENCING NEW TABLE AS n FOR EACH STATEMENT EXECUTE FUNCTION hub_bump_insert('ws');
CREATE TRIGGER hub_records_upd AFTER UPDATE ON records
  REFERENCING OLD TABLE o NEW TABLE AS n FOR EACH STATEMENT EXECUTE FUNCTION hub_bump_update('ws');
CREATE TRIGGER hub_records_del AFTER DELETE ON records
  REFERENCING OLD TABLE AS o FOR EACH STATEMENT EXECUTE FUNCTION hub_bump_delete('ws');
CREATE TRIGGER hub_records_trunc AFTER TRUNCATE ON records
  FOR EACH STATEMENT EXECUTE FUNCTION hub_bump_truncate('ws');

CREATE TRIGGER hub_links_ins AFTER INSERT ON links
  REFERENCING NEW TABLE AS n FOR EACH STATEMENT EXECUTE FUNCTION hub_bump_insert('ws');
CREATE TRIGGER hub_links_upd AFTER UPDATE ON links
  REFERENCING OLD TABLE o NEW TABLE AS n FOR EACH STATEMENT EXECUTE FUNCTION hub_bump_update('ws');
CREATE TRIGGER hub_links_del AFTER DELETE ON links
  REFERENCING OLD TABLE AS o FOR EACH STATEMENT EXECUTE FUNCTION hub_bump_delete('ws');
CREATE TRIGGER hub_links_trunc AFTER TRUNCATE ON links
  FOR EACH STATEMENT EXECUTE FUNCTION hub_bump_truncate('ws');

CREATE TRIGGER hub_change_headers_ins AFTER INSERT ON change_headers
  REFERENCING NEW TABLE AS n FOR EACH STATEMENT EXECUTE FUNCTION hub_bump_insert('ws');
CREATE TRIGGER hub_change_headers_upd AFTER UPDATE ON change_headers
  REFERENCING OLD TABLE o NEW TABLE AS n FOR EACH STATEMENT EXECUTE FUNCTION hub_bump_update('ws');
CREATE TRIGGER hub_change_headers_del AFTER DELETE ON change_headers
  REFERENCING OLD TABLE AS o FOR EACH STATEMENT EXECUTE FUNCTION hub_bump_delete('ws');
CREATE TRIGGER hub_change_headers_trunc AFTER TRUNCATE ON change_headers
  FOR EACH STATEMENT EXECUTE FUNCTION hub_bump_truncate('ws');

CREATE TRIGGER hub_change_ops_ins AFTER INSERT ON change_ops
  REFERENCING NEW TABLE AS n FOR EACH STATEMENT EXECUTE FUNCTION hub_bump_insert('ws');
CREATE TRIGGER hub_change_ops_upd AFTER UPDATE ON change_ops
  REFERENCING OLD TABLE o NEW TABLE AS n FOR EACH STATEMENT EXECUTE FUNCTION hub_bump_update('ws');
CREATE TRIGGER hub_change_ops_del AFTER DELETE ON change_ops
  REFERENCING OLD TABLE AS o FOR EACH STATEMENT EXECUTE FUNCTION hub_bump_delete('ws');
CREATE TRIGGER hub_change_ops_trunc AFTER TRUNCATE ON change_ops
  FOR EACH STATEMENT EXECUTE FUNCTION hub_bump_truncate('ws');

ALTER TABLE workspaces ENABLE ALWAYS TRIGGER hub_workspaces_ins;
ALTER TABLE workspaces ENABLE ALWAYS TRIGGER hub_workspaces_upd;
ALTER TABLE workspaces ENABLE ALWAYS TRIGGER hub_workspaces_del;
ALTER TABLE workspaces ENABLE ALWAYS TRIGGER hub_workspaces_trunc;
ALTER TABLE manifests ENABLE ALWAYS TRIGGER hub_manifests_ins;
ALTER TABLE manifests ENABLE ALWAYS TRIGGER hub_manifests_upd;
ALTER TABLE manifests ENABLE ALWAYS TRIGGER hub_manifests_del;
ALTER TABLE manifests ENABLE ALWAYS TRIGGER hub_manifests_trunc;
ALTER TABLE records ENABLE ALWAYS TRIGGER hub_records_ins;
ALTER TABLE records ENABLE ALWAYS TRIGGER hub_records_upd;
ALTER TABLE records ENABLE ALWAYS TRIGGER hub_records_del;
ALTER TABLE records ENABLE ALWAYS TRIGGER hub_records_trunc;
ALTER TABLE links ENABLE ALWAYS TRIGGER hub_links_ins;
ALTER TABLE links ENABLE ALWAYS TRIGGER hub_links_upd;
ALTER TABLE links ENABLE ALWAYS TRIGGER hub_links_del;
ALTER TABLE links ENABLE ALWAYS TRIGGER hub_links_trunc;
ALTER TABLE change_headers ENABLE ALWAYS TRIGGER hub_change_headers_ins;
ALTER TABLE change_headers ENABLE ALWAYS TRIGGER hub_change_headers_upd;
ALTER TABLE change_headers ENABLE ALWAYS TRIGGER hub_change_headers_del;
ALTER TABLE change_headers ENABLE ALWAYS TRIGGER hub_change_headers_trunc;
ALTER TABLE change_ops ENABLE ALWAYS TRIGGER hub_change_ops_ins;
ALTER TABLE change_ops ENABLE ALWAYS TRIGGER hub_change_ops_upd;
ALTER TABLE change_ops ENABLE ALWAYS TRIGGER hub_change_ops_del;
ALTER TABLE change_ops ENABLE ALWAYS TRIGGER hub_change_ops_trunc;