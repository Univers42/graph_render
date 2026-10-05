#!/usr/bin/env bash
# hub-mem-upload.sh — sourced by scripts/orch/hub-mem.sh; its `upload` verb (Decision 4).
#
# Starts a real graph-server as the motor beside the hub, runs the client case that fills one
# workspace to GRAPH_HUB_MAX_DOC_BYTES, and reads the six `layout-upload` lines the relay wrote.
#
# The motor is the graph-motor image (scripts/service.sh), one container named
# gm-hub-motor-<worktree dir>, started detached with SERVICE_PORT=0 so nothing is published to a
# host port the case could reach by accident: the hub reaches it at its **bridge** address, which is
# what makes this loopback between two containers on one host rather than a network measurement.
#
# Credentials: `scripts/service.sh keygen` writes `name <sha256>` into the keyfile at 0640 and the
# plaintext on stdout; the plaintext goes into a 0600 file the hub reads through
# GRAPH_HUB_MOTOR_KEY_FILE (see motor_credentials for who owns it and why). Neither the key nor the
# keyfile's line is ever printed here.
#
# Caveat: the motor's own build (`scripts/service.sh build`) stages the studio bundle as well as the
# binary, so this verb is slow the first time and on any change under studio/. `hub.sh
# upload-measurement` is the same verb.
#
# Caveat: the motor container runs with drun's own 8 g cap (scripts/service.sh run), not the hub's
# 1 g. Only the hub's cap is the row's subject (§6 caps the hub), and raising the motor's would
# change what the motor is.
#
# Sourced, not run: `root`, `out` and `here` are the caller's (scripts/orch/hub-mem-upload-run.sh),
# and this file assigns none of them. shellcheck reads that as SC2154, which is why the names appear
# in the caller's `source` line's contract rather than being re-derived here: deriving `root` twice
# is two `git rev-parse` calls and one more place the worktree name can disagree.

# The motor container of this worktree, and the key material beside the hub's own.
motor_name() { printf 'gm-hub-motor-%s' "$(basename "$root")"; }

# The motor's bridge address, which is how the hub reaches it. The published 127.0.0.1 port is
# deliberately not used: it is a host path, and the measurement is between two containers.
motor_ip() { docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' "$(motor_name)" 2>/dev/null; }

# A keyfile for the motor and its plaintext, both under target/hub-mem/.
#
# The plaintext is minted by `scripts/service.sh keygen` on stdout and then **written by a container
# running as the hub's own uid (10001)**, because the hub container reads it as that uid and a 0600
# file owned by the host user is unreadable to it: measured here, not assumed — the relay's
# `bearer()` turned the unreadable file into `MotorUnavailable`, so every `/layout` answered 502 in
# 6 ms. Mode 0600 is what Decision 6 and `docs/deploy/hub.md` name, and this is the only way to get
# it and have the hub read it, since the host user cannot `chown` to 10001.
#
# The keyfile itself is written by keygen directly at 0640, which is the mode graph-server's own
# `KeySet::load` requires and the mode `scripts/service.sh keygen` installs.
motor_credentials() {
  mkdir -p "$out" || return 1
  [ -s "$out/motor-key" ] && [ -s "$out/motor-keys" ] && return 0
  rm -f "$out/motor-key" "$out/motor-keys"
  local minted
  minted=$("$root/scripts/service.sh" keygen motor "$out/motor-keys") || return 1
  [ -n "$minted" ] || return 1
  write_as_the_hub "$minted" || return 1
  # The staged file's mode and owner, so a caller can see the credential is not world-readable.
  ls -l "$out/motor-key" >&2
}

# `text` into target/hub-mem/motor-key, created by a container as uid 10001 at mode 0600.
#
# WHY a container and not a redirect: the hub reads this file as 10001 (the image's user) and 0600
# grants nothing to anyone but the owner, so the file has to be **owned** by 10001 — and the host user
# cannot `chown` to a uid it does not hold. A container as that uid can create it.
#
# The key arrives on the container's **stdin** and is written from there, so it never reaches a command
# line and `ps` on the host never shows it. `--group-add` plus a group-writable `target/hub-mem` is
# what lets uid 10001 create the file there at all; the directory is this script's own under
# `target/`, and `hub-mem.sh reset` removes it. drun, like every container here.
write_as_the_hub() {
  local image
  image=$(cat target/hub-image/name) || return 1
  chmod g+w "$out" || return 1
  printf '%s\n' "$1" \
    | "$here/drun" --rm --network none -u 10001 --group-add "$(id -g)" --entrypoint /bin/sh \
      -v "$root/$out:/run/keys" "$image" \
      -c 'umask 077; cat > /run/keys/motor-key' >/dev/null 2>&1
}

# Wait for the motor to answer /healthz over its own bridge address, or give up. 120 half-second
# polls is a guess above a cold container start, not a measurement; failing here means the motor
# never came up, and its redacted log tail says why.
await_motor() {
  local i ip=${1-}
  for ((i = 0; i < 120; i++)); do
    healthz "$ip" && return 0
    [ "$(docker inspect -f '{{.State.Running}}' "$(motor_name)" 2>/dev/null)" = true ] || break
    sleep 0.5
  done
  echo "hub-mem: the motor did not become healthy; its log, URLs redacted:" >&2
  docker logs --tail 20 "$(motor_name)" 2>&1 | sed -E 's#[a-z]+://[^ "]*#<url>#g' >&2
  return 1
}

# One `GET /healthz` over the bridge; 0 when the motor answered 200.
#
# `curl` and not a raw `/dev/tcp` socket: `exec 3<>…` is a special builtin, so a refused connection
# kills a non-interactive shell outright and no `||` guard catches it — measured here, not assumed.
# `curl` is already this repo's readiness probe (scripts/service-limits.sh:161).
#
# The address is a parameter rather than read from the container, so the readiness loop probes the
# one it was given. The motor takes no credential on /healthz, so none is sent and none is read.
healthz() {
  local ip=${1-}
  [ -n "$ip" ] || return 1
  [ "$(curl -sS -o /dev/null -m 5 -w '%{http_code}' "http://$ip:8080/healthz" 2>/dev/null)" = 200 ]
}

# The hub's `layout-upload` lines, oldest first: `event`, `ws`, `bytes`, `records`, `upload_ms`.
#
# The log is read through `docker logs` rather than a file because the hub writes JSON to its
# stdout and nothing reads it here; each line is one upload, so six of them are the warm-up and the
# five timed runs.
upload_lines() {
  docker logs "$(hub_name)" 2>/dev/null | grep '"event":"layout-upload"' || true
}

# The hub container `hub-run.sh` started, by the same default it names.
hub_name() { printf 'gm-hub-%s' "$(basename "$root")"; }

# The motor's log lines that mention a body timeout, which the pass condition forbids.
motor_408s() {
  docker logs "$(motor_name)" 2>/dev/null | grep -c '\b408\b' || true
}

# The `upload_ms` values of the five runs after the warm-up, in log order.
#
# `tail -n 5` is the warm-up exclusion: the client case writes the warm-up first, so the first of
# the six lines is it and the last five are what §5.3's condition is over.
timed_ms() {
  upload_lines | tail -n 5 | sed -nE 's/.*"upload_ms":([0-9]+).*/\1/p'
}

# The median of the numbers on stdin, by sorting them. `sort -n` and not a mean: §5.3's condition
# is over the **slowest** run, so the median is only the figure the file quotes beside it, and a
# mean would move when one run is slow, which is the opposite of what a reader wants beside it.
median() {
  sort -n | awk '{v[NR] = $1} END {print (NR % 2) ? v[(NR + 1) / 2] : int((v[NR / 2] + v[NR / 2 + 1]) / 2)}'
}

