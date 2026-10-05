set -u
# Physical restores: which ones change the restore detector's keys, and which ones the LSN
# high-water sees. Every instance is a copy of one primary, on its own socket directory.
mkdir -p /tmp/arch
start() { pg_ctl -D "$1" -o "-k $2 -c listen_addresses='' -c shared_buffers=32MB" -l "$1.log" -w start >/dev/null; }
stop() { pg_ctl -D "$1" -m fast -w stop >/dev/null; }
sql() { psql -h "$1" -d hubdb -X -qAt -F' ' -c "$2"; }
# sysid, checkpoint timeline, current WAL file's timeline, database oid, LSN; read as role hub.
keys() {
  psql -h "$1" -d hubdb -U hub -X -qAt -F' ' -c "SELECT '$2', (SELECT system_identifier FROM pg_control_system()),
    'ckpt_tli=' || (SELECT timeline_id FROM pg_control_checkpoint()),
    'wal_tli=' || substr(pg_walfile_name(pg_current_wal_lsn()), 1, 8),
    'oid=' || (SELECT oid FROM pg_database WHERE datname = current_database()),
    'lsn=' || pg_current_wal_lsn(), 'below_high=' || (pg_wal_lsn_diff(pg_current_wal_lsn(), '${3:-0/0}') < 0)" 2>&1
}
writes() { sql "$1" "INSERT INTO t SELECT g, repeat('x', 200) FROM generate_series(1, $2) g"; }

initdb -D /tmp/p --encoding=UTF8 --locale=C >/dev/null
cat >>/tmp/p/postgresql.conf <<'EOF'
archive_mode = on
archive_command = 'test ! -f /tmp/arch/%f && cp %p /tmp/arch/%f'
EOF
mkdir -p /tmp/sp /tmp/ss /tmp/sb /tmp/sr /tmp/sv; start /tmp/p /tmp/sp
psql -h /tmp/sp -d postgres -X -q -c "CREATE ROLE hub LOGIN" -c "CREATE DATABASE hubdb OWNER hub"
sql /tmp/sp "CREATE TABLE t(x int, pad text)"; writes /tmp/sp 1000; sql /tmp/sp "CHECKPOINT"
keys /tmp/sp "0 primary"

echo "== volume snapshot taken cold"
stop /tmp/p; cp -a /tmp/p /tmp/vol; start /tmp/p /tmp/sp

echo "== base backup, then the standby"
pg_basebackup -h /tmp/sp -D /tmp/bb -X stream -c fast
pg_basebackup -h /tmp/sp -D /tmp/stb -X stream -c fast -R
start /tmp/stb /tmp/ss
writes /tmp/sp 20000; T=$(sql /tmp/sp "SELECT pg_current_wal_lsn()")
writes /tmp/sp 20000; sql /tmp/sp "SELECT pg_switch_wal()" >/dev/null; sleep 3
HIGH=$(sql /tmp/sp "SELECT pg_current_wal_lsn()")
echo "target=$T high=$HIGH archived=$(sql /tmp/sp "SELECT last_archived_wal FROM pg_stat_archiver")"
keys /tmp/sp "1 primary after writes" "$HIGH"

echo "== promotion of a caught-up standby"
sleep 2; psql -h /tmp/ss -d postgres -X -qAt -c "SELECT 'promoted', pg_promote(true, 60)"
keys /tmp/ss "2 promoted, at once" "$HIGH"
psql -h /tmp/ss -d postgres -X -qAt -c "CHECKPOINT"
keys /tmp/ss "2 promoted, after CHECKPOINT" "$HIGH"
stop /tmp/stb; stop /tmp/p

echo "== volume snapshot restored on the primary's path"
start /tmp/vol /tmp/sv; keys /tmp/sv "3 volume snapshot" "$HIGH"; stop /tmp/vol

echo "== base backup started without recovery.signal"
cp -a /tmp/bb /tmp/bb1; start /tmp/bb1 /tmp/sb; keys /tmp/sb "4 base backup, no signal" "$HIGH"; stop /tmp/bb1

echo "== point-in-time recovery to target, then promote"
cp -a /tmp/bb /tmp/pitr; touch /tmp/pitr/recovery.signal
cat >>/tmp/pitr/postgresql.conf <<EOF
archive_mode = off
restore_command = 'cp /tmp/arch/%f %p'
recovery_target_lsn = '$T'
recovery_target_action = 'promote'
EOF
start /tmp/pitr /tmp/sr
for _ in $(seq 1 30); do [ "$(psql -h /tmp/sr -d postgres -X -qAt -c 'SELECT pg_is_in_recovery()')" = f ] && break; sleep 1; done
keys /tmp/sr "5 PITR, at once" "$HIGH"
psql -h /tmp/sr -d postgres -X -qAt -c "CHECKPOINT"
keys /tmp/sr "5 PITR, after CHECKPOINT" "$HIGH"
stop /tmp/pitr
