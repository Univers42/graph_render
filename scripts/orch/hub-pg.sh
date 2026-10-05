#!/usr/bin/env bash
# hub-pg.sh — the PostgreSQL 17 container every graph-store database row runs against.
#
#   hub-pg.sh reset | start | stop | kill | url | ip | wait [secs] | sql "SQL"
#   hub-pg.sh copy-data | restore-data | switch-wal | replica | replica-promote | pitr "TIME"
#   hub-pg.sh run [cargo test args...]
#
# Every container starts through scripts/orch/drun, never a bare `docker run`
# (scripts/orch/drun-check.sh). The image is built from deploy/postgres.Dockerfile; `docker build`
# is not a run, so drun-check.sh stays green.
#
# Volumes: gm-hub-pg-data (the data directory) and gm-hub-pg-archive (WAL archive). Container
# name: gm-hub-pg. Role: postgres owns it; the store connects as `hub`/`hub` on database `hub`.
#
# Exit: 0 the verb did what it says · 1 it could not (docker or psql failed) · 2 usage
#
# Caveat: `run` needs the repository bind-mounted, because GM_HUB_STEP_DIR is a path under
# `target/` that scripts/orch/gr mounts read-write at /w — that is the whole file handshake the
# container-level cases (promotion, PITR, snapshot, kill) use, and it works only because both
# sides name the same directory.

set -uo pipefail
here=$(dirname "$(readlink -f "$0")")
root=$(git -C "$here" rev-parse --show-toplevel)
cd "$root" || exit 2
drun=$here/drun
name=gm-hub-pg
image=gm-hub-pg:17
data_vol=gm-hub-pg-data
archive_vol=gm-hub-pg-archive

image() {
  docker build -q -f deploy/postgres.Dockerfile -t "$image" deploy >/dev/null || return 1
}

# The container's bridge IP, which is how a `gr` test container reaches it: `gr` has no network
# option and its containers sit on Docker's default bridge, so 127.0.0.1 would be the test
# container's own loopback and a published host port would not exist.
ip() {
  docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' "$name" 2>/dev/null
}

url() {
  local addr
  addr=$(ip) || return 1
  [ -n "$addr" ] || return 1
  printf 'postgres://hub:hub@%s:5432/hub' "$addr"
}

# Caveat: 90 polls of one second is a guess above a cold container on a loaded host. It is only
# a bound on how long THIS waits; `wait` failing means the container never became ready, which a
# caller must not read as "the store is broken".
wait_ready() {
  local limit=${1:-90}
  local i
  for ((i = 0; i < limit; i++)); do
    docker exec "$name" pg_isready -U postgres >/dev/null 2>&1 && return 0
    sleep 1
  done
  echo "hub-pg: $name did not become ready in ${limit}s" >&2
  return 1
}

start() {
  image || return 1
  if docker inspect "$name" >/dev/null 2>&1; then
    docker start "$name" >/dev/null || return 1
  else
    "$drun" --name "$name" -d -e POSTGRES_PASSWORD=hub -e POSTGRES_DB=hub \
      -v "$data_vol":/var/lib/postgresql/data \
      -v "$archive_vol":/archive "$image" >/dev/null || return 1
  fi
  wait_ready 90 || return 1
  "$drun" --rm --network "container:$name" "$image" psql -v ON_ERROR_STOP=1 -h 127.0.0.1 -U postgres -d hub \
    -c "DO \$\$ BEGIN IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname='hub') THEN \
      CREATE ROLE hub LOGIN PASSWORD 'hub' CREATEDB; ELSE ALTER ROLE hub PASSWORD 'hub'; END IF; END \$\$;" \
    >/dev/null || return 1
  # PostgreSQL 15 and later grant CREATE on schema `public` to `pg_database_owner`, not to
  # PUBLIC, so the store's own role cannot create its tables in a database it does not own.
  "$drun" --rm --network "container:$name" "$image" psql -v ON_ERROR_STOP=1 -h 127.0.0.1 -U postgres -d hub \
    -c "GRANT ALL ON SCHEMA public TO hub;" >/dev/null || return 1
  # `detector_refuses_a_hub_writer_default` sets that GUC as a role default, which needs this.
  "$drun" --rm --network "container:$name" "$image" psql -v ON_ERROR_STOP=1 -h 127.0.0.1 -U postgres -d hub \
    -c "GRANT SET ON PARAMETER hub.writer TO hub;" >/dev/null || return 1
  # The restore detector reads `pg_control_system()`, `pg_current_wal_flush_lsn()` and
  # `pg_walfile_name()`, all of which are superuser-only by default. `hub` is deliberately NOT a
  # superuser, so it is granted `pg_monitor`, which is the least-privilege role that carries
  # `pg_read_all_settings` and EXECUTE on the WAL-position readers.
  # Caveat: `pg_control_system()` is not among them. `detector_refuses_fsync_off` proves which of
  # the three the role actually reaches; if it cannot, `pg_control_checks()` (the row-level
  # view, superuser-only too) or an explicit per-function GRANT is the alternative, and the test
  # that proved it is named in docs/measurements/hub-pg-epoch-probe/store-image.md.
  "$drun" --rm --network "container:$name" "$image" psql -v ON_ERROR_STOP=1 -h 127.0.0.1 -U postgres -d hub \
    -c "GRANT pg_monitor TO hub;" >/dev/null || return 1
  url || return 1
  echo
}

stop() { docker stop "$name" >/dev/null 2>&1; return 0; }
kill9() { docker kill "$name" >/dev/null 2>&1; return 0; }
reset() {
  docker rm -f "$name" >/dev/null 2>&1
  docker volume rm -f "$data_vol" "$archive_vol" >/dev/null 2>&1
  return 0
}

sql() {
  [ $# -eq 1 ] || { echo "hub-pg: sql needs one argument" >&2; exit 2; }
  "$drun" --rm --network "container:$name" "$image" psql -v ON_ERROR_STOP=1 -h 127.0.0.1 -U postgres -d hub -c "$1"
}

switch_wal() {
  "$drun" --rm --network "container:$name" "$image" psql -v ON_ERROR_STOP=1 -h 127.0.0.1 -U postgres -d hub \
    -c "SELECT pg_switch_wal()" >/dev/null
}

# copy-data / restore-data: a tar through drun with both volumes mounted. Caveat: the copy is of a
# STOPPED container's volume, so it is crash-consistent at best — that is exactly the case
# `detector_bumps_on_a_small_gap_crash_consistent_copy` needs.
copy_data() {
  docker stop "$name" >/dev/null 2>&1
  "$drun" --rm -v "$data_vol":/from:ro -v "$archive_vol":/to "$image" \
    sh -c 'cd /from && tar cf /to/hub-data.tar .'
}
restore_data() {
  docker rm -f "$name" >/dev/null 2>&1
  "$drun" --rm -v "$data_vol":/to -v "$archive_vol":/from "$image" \
    sh -c 'cd /to && rm -rf ./* && tar xf /from/hub-data.tar'
}

# replica / replica-promote / pitr: recovery.signal and standby.signal written into the data
# volume, then started (as a standby) or promoted. The recovery lines name the archive volume.
recovery_lines() {
  printf '%s\n' "restore_command = 'cp /archive/%f %p'" "recovery_target_timeline = 'latest'"
}
replica() {
  recovery_lines >target/hub-recovery.conf || return 1
  docker rm -f "$name" >/dev/null 2>&1
  "$drun" --rm -v "$data_vol":/data -v "$archive_vol":/archive "$image" \
    sh -c 'cp /w/target/hub-recovery.conf /data/recovery.conf && touch /data/standby.signal' || return 1
  image || return 1
  "$drun" --name "$name" -d -e POSTGRES_PASSWORD=hub -e POSTGRES_DB=hub -v "$data_vol":/var/lib/postgresql/data \
    -v "$archive_vol":/archive "$image" >/dev/null || return 1
  wait_ready 90
}
replica_promote() {
  "$drun" --rm --network "container:$name" "$image" \
    pg_ctl -D /var/lib/postgresql/data/pgdata promote -w
}
pitr() {
  [ $# -eq 1 ] || { echo "hub-pg: pitr needs a target time" >&2; exit 2; }
  recovery_lines >target/hub-recovery.conf || return 1
  printf "recovery_target_time = '%s'\n" "$1" >>target/hub-recovery.conf || return 1
  docker rm -f "$name" >/dev/null 2>&1
  "$drun" --rm -v "$data_vol":/data -v "$archive_vol":/archive "$image" \
    sh -c 'cp /w/target/hub-recovery.conf /data/recovery.conf && touch /data/standby.signal' || return 1
  image || return 1
  "$drun" --name "$name" -d -e POSTGRES_PASSWORD=hub -e POSTGRES_DB=hub -v "$data_vol":/var/lib/postgresql/data \
    -v "$archive_vol":/archive "$image" >/dev/null || return 1
  wait_ready 90
}

run() {
  local u
  u=$(url) || { echo "hub-pg: no $name; run hub-pg.sh start first" >&2; return 1; }
  "$here/gr" -e GM_HUB_PG_URL="$u" -e GM_HUB_BREAK="${GM_HUB_BREAK-}" \
    -e GM_HUB_STEP_DIR=target/hub-steps cargo test --manifest-path server/Cargo.toml \
    -p graph-store --features db-tests,negctl "$@"
}

case "${1-}" in
reset) reset ;;
start) start ;;
stop) stop ;;
kill) kill9 ;;
url) url ;;
ip) ip ;;
wait) wait_ready "${2:-90}" ;;
sql) shift; sql "$@" ;;
copy-data) copy_data ;;
restore-data) restore_data ;;
switch-wal) shift; switch_wal "$@" ;;
replica) shift; replica "$@" ;;
replica-promote) shift; replica_promote "$@" ;;
pitr) shift; pitr "$@" ;;
run) shift; run "$@" ;;
--help | -h) sed -n '2,/^$/p' "$0" | sed -e 's/^# \{0,1\}//' -e '/^$/d'; exit 0 ;;
*) echo "hub-pg: unknown verb: ${1-}" >&2; exit 2 ;;
esac