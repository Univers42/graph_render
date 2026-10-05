\set VERBOSITY terse
SHOW server_encoding; SELECT datcollate, datctype FROM pg_database WHERE datname = current_database();
SELECT string_agg(x, ',' ORDER BY x COLLATE "C") AS c_order FROM unnest(ARRAY['é','z','Z','a']) x;
CREATE TABLE workspaces(id text PRIMARY KEY, epoch bigint NOT NULL, head_seq bigint NOT NULL DEFAULT 0);
CREATE TABLE records(ws text NOT NULL REFERENCES workspaces ON DELETE CASCADE, id text, body text, PRIMARY KEY(ws,id));
CREATE TABLE epoch_clock(one boolean PRIMARY KEY DEFAULT true CHECK(one), last bigint NOT NULL);
INSERT INTO epoch_clock VALUES (true, 0);
CREATE FUNCTION hub_next_epoch() RETURNS bigint LANGUAGE sql AS $$
  UPDATE epoch_clock SET last = greatest(last + 1, (extract(epoch FROM clock_timestamp()) * 1000)::bigint) RETURNING last $$;
CREATE FUNCTION hub_bump() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  IF current_setting('hub.writer', true) = '1' OR pg_trigger_depth() > 1 THEN RETURN NULL; END IF;
  IF TG_OP = 'TRUNCATE' THEN
    UPDATE workspaces SET epoch = hub_next_epoch();
  ELSIF TG_OP = 'INSERT' THEN
    EXECUTE format('UPDATE workspaces SET epoch = hub_next_epoch() WHERE id IN (SELECT %I FROM new_rows)', TG_ARGV[0]);
  ELSIF TG_OP = 'DELETE' THEN
    EXECUTE format('UPDATE workspaces SET epoch = hub_next_epoch() WHERE id IN (SELECT %I FROM old_rows)', TG_ARGV[0]);
  ELSE
    EXECUTE format('UPDATE workspaces SET epoch = hub_next_epoch() WHERE id IN (SELECT %I FROM new_rows UNION SELECT %I FROM old_rows)', TG_ARGV[0], TG_ARGV[0]);
  END IF;
  RETURN NULL;
END $$;
\echo == 1 multi-event with transition tables
CREATE TRIGGER t_multi AFTER INSERT OR UPDATE OR DELETE ON records REFERENCING NEW TABLE AS new_rows FOR EACH STATEMENT EXECUTE FUNCTION hub_bump('ws');
\echo == 2 per-event triggers
CREATE TRIGGER rec_ins AFTER INSERT ON records REFERENCING NEW TABLE AS new_rows FOR EACH STATEMENT EXECUTE FUNCTION hub_bump('ws');
CREATE TRIGGER rec_upd AFTER UPDATE ON records REFERENCING OLD TABLE AS old_rows NEW TABLE AS new_rows FOR EACH STATEMENT EXECUTE FUNCTION hub_bump('ws');
CREATE TRIGGER rec_del AFTER DELETE ON records REFERENCING OLD TABLE AS old_rows FOR EACH STATEMENT EXECUTE FUNCTION hub_bump('ws');
CREATE TRIGGER rec_trunc AFTER TRUNCATE ON records FOR EACH STATEMENT EXECUTE FUNCTION hub_bump('ws');
CREATE TRIGGER ws_ins AFTER INSERT ON workspaces REFERENCING NEW TABLE AS new_rows FOR EACH STATEMENT EXECUTE FUNCTION hub_bump('id');
CREATE TRIGGER ws_upd AFTER UPDATE ON workspaces REFERENCING OLD TABLE AS old_rows NEW TABLE AS new_rows FOR EACH STATEMENT EXECUTE FUNCTION hub_bump('id');
CREATE TABLE ep(step text, epoch bigint);
BEGIN; SET LOCAL hub.writer = '1'; INSERT INTO workspaces VALUES ('w1', hub_next_epoch()); INSERT INTO workspaces VALUES ('w2', hub_next_epoch()); COMMIT;
INSERT INTO ep SELECT '0 created by hub', epoch FROM workspaces WHERE id='w1';
BEGIN; SET LOCAL hub.writer = '1'; INSERT INTO records VALUES ('w1','a','x'),('w1','b','y'); UPDATE workspaces SET head_seq = head_seq + 1 WHERE id='w1'; COMMIT;
INSERT INTO ep SELECT '1 hub write (expect same)', epoch FROM workspaces WHERE id='w1';
INSERT INTO records VALUES ('w1','c','z');
INSERT INTO ep SELECT '2 manual insert (expect new)', epoch FROM workspaces WHERE id='w1';
UPDATE records SET body='q' WHERE id='a';
INSERT INTO ep SELECT '3 manual update (expect new)', epoch FROM workspaces WHERE id='w1';
DELETE FROM records WHERE id='c';
INSERT INTO ep SELECT '4 manual delete (expect new)', epoch FROM workspaces WHERE id='w1';
BEGIN; SET LOCAL hub.writer = '1'; DELETE FROM records WHERE id='b'; COMMIT;
INSERT INTO ep SELECT '5 hub sweeper-like delete (expect same)', epoch FROM workspaces WHERE id='w1';
UPDATE workspaces SET head_seq = head_seq + 5 WHERE id='w1';
INSERT INTO ep SELECT '6 manual head_seq edit (expect new, no recursion)', epoch FROM workspaces WHERE id='w1';
TRUNCATE records;
INSERT INTO ep SELECT '7 truncate (expect new)', epoch FROM workspaces WHERE id='w1';
INSERT INTO records VALUES ('w1','d','1');
INSERT INTO ep SELECT '8 manual insert before replica (expect new)', epoch FROM workspaces WHERE id='w1';
SET session_replication_role = replica; UPDATE records SET body='2'; RESET session_replication_role;
INSERT INTO ep SELECT '9 replica role, ENABLE (expect same: bypass)', epoch FROM workspaces WHERE id='w1';
ALTER TABLE records ENABLE ALWAYS TRIGGER rec_upd;
SET session_replication_role = replica; UPDATE records SET body='3'; RESET session_replication_role;
INSERT INTO ep SELECT '10 replica role, ENABLE ALWAYS (expect new)', epoch FROM workspaces WHERE id='w1';
DELETE FROM workspaces WHERE id='w1'; INSERT INTO workspaces VALUES ('w1', 1);
INSERT INTO ep SELECT '11 manual delete+recreate with epoch 1 (expect new, > all)', epoch FROM workspaces WHERE id='w1';
\copy records FROM stdin
w1	cp	v
\.
INSERT INTO ep SELECT '12 COPY FROM (expect new)', epoch FROM workspaces WHERE id='w1';
UPDATE epoch_clock SET last = 0;
INSERT INTO workspaces VALUES ('w3', hub_next_epoch());
INSERT INTO ep SELECT '13 clock rewound to 0 (PITR), new epoch (expect > all)', epoch FROM workspaces WHERE id='w3';
SELECT step, epoch, epoch - lag(epoch) OVER (ORDER BY split_part(step,' ',1)::int) AS delta FROM ep ORDER BY split_part(step,' ',1)::int;
