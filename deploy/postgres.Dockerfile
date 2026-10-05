FROM postgres:17
# The hub's settings, appended to $PGDATA/postgresql.conf by the entrypoint on first init, so a
# fresh volume and a rebuilt image agree. `archive_mode` is what makes hub-epoch's promotion and
# point-in-time-recovery cases possible at all; /archive is a named volume hub-pg.sh mounts.
#
# Caveat: `postgres:17` moves within the line, so two runs can differ by patch level. Row
# `hub-pg-durability` reads `server_version` and records it in
# docs/measurements/hub-pg-epoch-probe/store-image.md; nothing in this slice depends on a patch.
#
# N12: `C` collation and UTF8, so `(qcoll COLLATE "C", id COLLATE "C")` really is byte order and
# the store's §6 start check can be a hard refusal instead of a hope.
ENV POSTGRES_INITDB_ARGS="--encoding=UTF8 --locale=C"
#
# The Dockerfile writes the settings to 10-hub.conf.sh, then rewrites that same file into a
# script whose body appends those lines to $PGDATA/postgresql.conf. One file, no COPY: the
# build context is `deploy/` and nothing else there is ours to add.
RUN set -eu; conf=/docker-entrypoint-initdb.d/10-hub.conf.sh; printf '%s\n' \
  'fsync = on' \
  'synchronous_commit = on' \
  'full_page_writes = on' \
  "listen_addresses = '*'" \
  'wal_level = replica' \
  'max_wal_senders = 10' \
  'hot_standby = on' \
  'archive_mode = on' \
  "archive_command = 'test ! -f /archive/%f && cp %p /archive/%f'" \
  > $conf.body \
  && printf '#!/bin/sh\ncat <<EOF >> "$PGDATA/postgresql.conf"\n%s\nEOF\n' "$(cat $conf.body)" > $conf \
  && rm -f $conf.body \
  && chmod +x $conf