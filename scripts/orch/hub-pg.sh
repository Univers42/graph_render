#!/usr/bin/env bash
# hub-pg.sh — the PostgreSQL 17 container every graph-store database row runs against.
#
#   hub-pg.sh reset | start | stop | kill | url | ip | wait [secs] | sql "SQL"
#   hub-pg.sh copy-data | restore-data | switch-wal | replica | replica-promote | pitr "NAME"
#   hub-pg.sh run [cargo test args...]
#
# Every container starts through scripts/orch/drun, never a bare `docker run`
# (scripts/orch/drun-check.sh). The image is built from deploy/postgres.Dockerfile; `docker build`
# is not a run, so drun-check.sh stays green.
#
# Volumes: gm-hub-pg-data (the data directory), gm-hub-pg-archive (WAL archive) and
# gm-hub-pg-snapshot (a tar of the data volume, used only by copy-data/restore-data).
# Container name: gm-hub-pg. Role: postgres owns it; the store connects as `hub`/`hub` on database `hub`.
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
# A third volume, used only by copy-data/restore-data: a volume snapshot belongs in its own
# volume, and putting the tar in the archive volume would leave a multi-gigabyte file beside WAL.
snap_vol=gm-hub-pg-snapshot

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
  local u="postgres://hub:hub@$addr:5432/hub"
  # WHY the URL is also written to a file: every container-level case (promotion, PITR, volume
  # snapshot, kill/restart) replaces the container, and a replacement gets a NEW bridge IP. A test
  # that read GM_HUB_PG_URL once would keep dialling the old address. `target/` is the same
  # directory on the host and in the test container, so re-reading this file is how a test follows
  # the server across a restore.
  mkdir -p target
  printf '%s' "$u" >target/hub-pg-url || return 1
  printf '%s' "$u"
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

prepare_volumes() {
  # A named volume created by `docker run -v name:/path` is owned by root, and PostgreSQL runs as
  # uid 999. Without this, `archive_command` cannot write the WAL archive — it fails every second
  # with "Permission denied" — and a standby or a PITR has nothing to replay, so it comes up as a
  # fresh primary. That failure is silent from the outside: the server is up and the data looks
  # right, which is the worst shape for a detector test.
  docker volume create "$data_vol" >/dev/null || return 1
  docker volume create "$archive_vol" >/dev/null || return 1
  docker volume create "$snap_vol" >/dev/null || return 1
  "$drun" --rm --user 0 -v "$data_vol":/data -v "$archive_vol":/archive "$image" \
    sh -c 'chown -R 999:999 /archive; chmod 1777 /archive'
}

start() {
  image || return 1
  prepare_volumes || return 1
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
  # WHY the removals are NOT silenced: `docker volume rm` fails while anything still holds the
  # volume, and a swallowed failure means the next `start` silently reuses the OLD volume. That is
  # not a nuisance: a volume left on timeline 2 by an earlier promotion makes every later PITR
  # look for a `00000002.history` that does not exist, recovery never reaches its consistent
  # point, and the server refuses to start — which reads as "PITR is broken" rather than
  # "the reset did not happen".
  docker rm -f "$name" >/dev/null 2>&1
  local vol rc=0
  for vol in "$data_vol" "$archive_vol" "$snap_vol"; do
    docker volume rm -f "$vol" >/dev/null 2>&1 || rc=1
  done
  [ "$rc" -eq 0 ] || { echo "hub-pg: reset: a volume is still in use; not pretending it was removed" >&2; return 1; }
  rm -f target/hub-pg-url
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

# copy-data / restore-data: a tar through drun between the data volume and a third named volume.
#
# Caveat: the copy is taken from a STOPPED container's volume, so it is crash-consistent at best
# and at worst a little behind the last commit. That is exactly the case
# `detector_bumps_on_a_small_gap_crash_consistent_copy` needs, and it is why `hub-pg.sh
# switch-wal` has to run BEFORE the copy: a segment still open is lost with the volume, while a
# completed segment is in the archive.
copy_data() {
  # WHY a checkpoint and a segment switch BEFORE the stop: the base copy has to be a consistent
  # starting point whose own WAL is in the archive. Without them the base's control file can name
  # a checkpoint whose REDO range was never archived, and recovery from that base then starts PAST
  # the writes a PITR is supposed to replay — which is exactly the "requested recovery stop point
  # is before consistent recovery point" failure, wearing a different hat.
  sql "CHECKPOINT" >/dev/null 2>&1
  switch_wal >/dev/null 2>&1
  sleep 2
  docker stop "$name" >/dev/null 2>&1
  "$drun" --rm -v "$data_vol":/from:ro -v "$snap_vol":/to "$image" \
    sh -c 'cd /from && tar cf /to/hub-data.tar .'
}
restore_data() {
  docker rm -f "$name" >/dev/null 2>&1
  "$drun" --rm -v "$data_vol":/to -v "$snap_vol":/from "$image" \
    sh -c 'cd /to && find . -mindepth 1 -maxdepth 1 -exec rm -rf {} + && tar xf /from/hub-data.tar'
}

# replica / replica-promote / pitr.
#
# WHY the recovery settings go into postgresql.conf and a bare `recovery.signal` is created:
# PostgreSQL 12 removed `recovery.conf` entirely, so a file named that is silently ignored and the
# server comes up as a fresh primary with no standby.signal — which would make a promotion test
# pass for the wrong reason. `standby.signal` is what keeps it a standby.
recovery_settings() {
  printf '%s\n' \
    "restore_command = 'cp /archive/%f %p'" \
    "hot_standby = on"
}

# Write the recovery settings and the signal files into the data volume, then start.
# $1 is 'standby' or 'target'; $2, when 'target', is the recovery target time.
start_recovery() {
  local mode=$1 target=${2-}
  {
    recovery_settings
    # WHY an `if` and not `[ -n "$target" ] && printf ...`: the `&&` form returns 1 when the
    # condition is false, which is this block's exit status, so the `|| return 1` below fired and
    # `replica` returned before it had replaced the container — leaving the ORIGINAL primary
    # running and the test dialling a server that was never a standby.
    # WHY `current` and not `latest`: `latest` makes recovery look for a HIGHER timeline's
    # history file. After a promotion on this cluster that file does not exist, the lookup fails,
    # and recovery never reaches its consistent point. A PITR on the cluster that wrote the
    # archive wants `current`.
    #
    # WHY a NAME and not a wall-clock time: the host clock, the container clock and the archived
    # commit timestamps are three clocks, and a target expressed in one of them is a target the
    # others may disagree with. A named restore point is a position in the WAL itself.
    if [ -n "$target" ]; then
      printf "recovery_target_timeline = 'current'\n"
      printf "recovery_target_name = '%s'\n" "$target"
      # WHY `promote` and not the default: with `hot_standby = on`, the default action for a named
      # target is `pause`, which leaves the server read-only and IN RECOVERY forever. The detector
      # refuses a database in recovery, so a paused PITR would fail the refusal instead of the
      # bump — the wrong reason, and a test that passes for it proves nothing.
      printf "recovery_target_action = 'promote'\n"
    else
      printf "recovery_target_timeline = 'latest'\n"
    fi
  } >target/hub-recovery.conf || return 1
  docker rm -f "$name" >/dev/null 2>&1
  image || return 1
  # The settings are copied in before the server starts, and the signal file decides whether it
  # comes up as a standby (waiting) or replays to the target and promotes itself.
  "$drun" --rm -v "$data_vol":/data -v "$archive_vol":/archive -v "$PWD/target":/w \
    "$image" sh -c "set -eu
      cat /w/hub-recovery.conf >> /data/postgresql.conf
      rm -f /data/standby.signal /data/recovery.signal /data/recovery.conf
      [ '$mode' = standby ] && touch /data/standby.signal || touch /data/recovery.signal" || return 1
  "$drun" --name "$name" -d -e POSTGRES_PASSWORD=hub -e POSTGRES_DB=hub \
    -v "$data_vol":/var/lib/postgresql/data -v "$archive_vol":/archive "$image" >/dev/null || return 1
  wait_ready 90
}

# A standby on a copy of the data volume, following the archive.
replica() {
  start_recovery standby
}

# Promote the running standby, in place.
#
# `pg_promote` over SQL rather than `pg_ctl promote`: it reaches the server through the network
# namespace, so it does not need the PGDATA path (which differs between PostgreSQL majors — 18
# moved it to a `pgdata` subdirectory) and it cannot get that path wrong.
replica_promote() {
  "$drun" --rm --network "container:$name" "$image" \
    psql -v ON_ERROR_STOP=1 -h 127.0.0.1 -U postgres -d hub \
    -c "SELECT pg_promote(true, 60)" >/dev/null
  wait_ready 90
}

# Replay the archive to a wall-clock time, then promote.
pitr() {
  [ $# -eq 1 ] || { echo "hub-pg: pitr needs a restore point name" >&2; exit 2; }
  start_recovery target "$1"
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