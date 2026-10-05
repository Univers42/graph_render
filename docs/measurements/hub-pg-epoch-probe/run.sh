set -u
initdb -D /tmp/pg --encoding=UTF8 --locale=C >/dev/null
pg_ctl -D /tmp/pg -o "-k /tmp -c listen_addresses=''" -l /tmp/pg.log -w start >/dev/null
psql -h /tmp -d postgres -X -q -f /probe/probe.sql 2>&1
echo "== 12 REPEATABLE READ vs concurrent prune"
psql -h /tmp -d postgres -X -q -c "CREATE TABLE ch(seq int); INSERT INTO ch SELECT generate_series(1,10);"
( psql -h /tmp -d postgres -X -qAt -c "BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY; SELECT 'headers', count(*) FROM ch; SELECT pg_sleep(2); SELECT 'ops', count(*) FROM ch; COMMIT;" ) &
sleep 1; psql -h /tmp -d postgres -X -qAt -c "DELETE FROM ch WHERE seq <= 5; SELECT 'pruned', count(*) FROM ch;"
wait
( psql -h /tmp -d postgres -X -qAt -c "BEGIN ISOLATION LEVEL READ COMMITTED; SELECT 'rc headers', count(*) FROM ch; SELECT pg_sleep(2); SELECT 'rc ops', count(*) FROM ch; COMMIT;" ) &
sleep 1; psql -h /tmp -d postgres -X -qAt -c "DELETE FROM ch WHERE seq <= 8;"
wait
