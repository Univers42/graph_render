set -u
initdb -D /tmp/pg --encoding=UTF8 --locale=C >/dev/null 2>&1
pg_ctl -D /tmp/pg -o "-k /tmp -c listen_addresses=''" -l /tmp/pg.log -w start >/dev/null
P="psql -h /tmp -X -qAt"
$P -d postgres -c "CREATE ROLE hub LOGIN"; $P -d postgres -c "CREATE DATABASE hubdb OWNER hub"
echo "== as hub (non-superuser)"
$P -d hubdb -U hub -c "SELECT 'sysid', system_identifier FROM pg_control_system();" 2>&1
$P -d hubdb -U hub -c "SELECT 'tli', timeline_id FROM pg_control_checkpoint();" 2>&1
$P -d hubdb -U hub -c "SELECT 'dboid', oid FROM pg_database WHERE datname = current_database();" 2>&1
echo "== dump/restore: are triggers created after data?"
$P -d hubdb -U hub -c "CREATE TABLE t(x int); CREATE FUNCTION f() RETURNS trigger LANGUAGE plpgsql AS \$\$ BEGIN RAISE NOTICE 'fired'; RETURN NULL; END \$\$; CREATE TRIGGER tr AFTER INSERT ON t FOR EACH STATEMENT EXECUTE FUNCTION f(); ALTER TABLE t ENABLE ALWAYS TRIGGER tr; INSERT INTO t VALUES (1);" 2>&1
pg_dump -h /tmp -U hub -Fc hubdb > /tmp/d.dump
pg_restore -l /tmp/d.dump | grep -E 'TABLE DATA|TRIGGER' 
$P -d postgres -c "CREATE DATABASE r OWNER hub;"
pg_restore -h /tmp -U hub -d r /tmp/d.dump 2>&1 | head -n 5
echo "restored rows: $($P -d r -U hub -c 'SELECT count(*) FROM t')"
$P -d r -U hub -c "SELECT 'r dboid', oid FROM pg_database WHERE datname = current_database();"
