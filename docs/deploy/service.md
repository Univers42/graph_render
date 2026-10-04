# graph-motor as a service

One image holds two things. The first is `graph-server`, the HTTP layout service: `server/`,
contract `docs/contract/service-api.md`. The second is the embed bundle, `<graph-studio>` with both
wasm builds of the motor, served under `/embed/<version>/`. The scripts' headers are the manual. This
page explains how the parts fit together.

| file | what it is |
|---|---|
| `scripts/service.sh` | `build`, `run`, `keygen`, `version`, `image` |
| `deploy/service.Dockerfile` | trixie-slim pinned by digest. It copies the staged artifacts in and builds nothing |
| `app/vite.embed.config.ts`, `app/src/embed.ts` | the bundle (`scripts/studio.sh embed DIR`) |
| `scripts/service-image.sh`, `scripts/orch/rows/service-image.rows` | the gate and the SDK row, each with its negative controls |

## Build

```sh
scripts/service.sh build      # prints graph-motor:<tag>; logs the size and the embed version
scripts/service.sh version    # <version>, the path segment hosts put in their tags
```

`build` stages everything into `target/service/stage`, which is the whole build context:

- `bin/graph-server`, built in release inside `ge-rust`, which is itself trixie, so it links against the
  same glibc as the base.
- `embed/<version>/`: `graph-studio.js`, `graph-sdk.js`, their chunks under `assets/`,
  `graph_wasm.wasm` and `graph_wasm_threads.wasm`.
- `embed/VERSION`: `<version>` followed by a newline.

Those three are the only paths staged, and staging them one by one is what excludes everything else:
the build context *is* `target/service/stage`, so no key file, `.env`, `.git` or scratch path can
reach the build, and there is no separate ignore file to keep in step with it. The `svc-no-leak` scan
in `scripts/service-image.sh` checks the image that context produces, so a path that slipped into the
stage is caught after the fact rather than assumed away.

`<version>` is a content hash: the first 16 hex characters of the sha256 over `sha256sum` of every
bundle file, sorted by path. Two different bundles therefore never share a URL, and that is what makes
the year-long `immutable` cache safe. Identical bytes keep their URL across commits. `<tag>` is the same
hash taken over the whole stage. The image labels carry `org.opencontainers.image.version`
(`<version>`) and `org.opencontainers.image.revision` (the commit).

## Run

```sh
scripts/service.sh keygen ops keys     # the key on stdout, once; `ops <sha256>` appended to ./keys
scripts/service.sh run keys            # 127.0.0.1:8080, read-only root, no capabilities
```

In this repository `run` starts the container through `scripts/orch/drun`, which adds the memory cap
(`DRUN_MEM`, default 8g for `run`) and the `gm.slice` ceiling. 8 GiB is the minimum: one worker
slot is 4.32 GiB (`docs/measurements/service-caps.md`, "Memory per slot"), so a 4 GiB container
holds no slot and the server exits 2 at start. On a host without this repository, the same
container is:

```sh
docker run -d --name graph-motor --read-only --cap-drop ALL --security-opt no-new-privileges \
  --memory 8g --memory-swap 8g \
  --group-add "$(stat -c %g keys)" -v "$PWD/keys:/run/graph/keys:ro" -e GRAPH_API_KEYS_FILE=/run/graph/keys \
  -p 127.0.0.1:8080:8080 graph-motor:<tag>
```

Publish on `127.0.0.1` only. The server has no TLS, so a remote client reaches it through the
host's proxy (contract, "Headers"). The image's `HEALTHCHECK` runs `graph-server healthcheck`,
which exits 0 only on a 200 from `/healthz` and gives up after 2 s.

The key file holds `<name> <sha256-hex>` lines and never a key, and it is mounted read-only at
run time. The process runs as uid 10001 and reads the file through the file's group
(`--group-add`), so the file mode is `0640`. Anything but 0640 or stricter — group write or exec, or
any bit for others, read included, because the file holds hashes — is refused at start
(contract C9).

### Rotate a key

1. Run `scripts/service.sh keygen ops-2 keys`. It appends the new line and the old one keeps working.
2. Run `docker kill -s HUP <container>`. The server re-reads the whole file and swaps the set in one
   step. A malformed or empty file keeps the old set (C9).
3. Move the clients to the new key, delete the `ops` line, then send SIGHUP again.

## Embed

The motor runs in a module Worker created from the bundle's own URL. Browsers refuse a Worker from
another origin, so **the host page and `/embed/` must share one origin**. The supported v1 embed is
the host reverse-proxying `/embed/` to the container (contract C10, `docs/contract/host-api.md`
condition 13). A page that loads the bundle straight from the service's origin fails at mount with
a `SecurityError` naming the Worker. Row `direct-refused` asserts that failure. `/embed/` still sends
`Access-Control-Allow-Origin: *`, but that does not make a direct embed work.

```nginx
location /embed/ {
    proxy_pass http://127.0.0.1:8080;   # the container, published on the loopback
    proxy_http_version 1.1;
}
```

The page, as the gate serves it (`deploy/nav/serviceproxy.py`):

```html
<graph-studio wasm="/embed/<version>/graph_wasm.wasm"></graph-studio>
<script type="module" src="/embed/<version>/graph-studio.js"></script>
```

The element resolves `wasm` against the page, so the attribute names the full path. Without it, the
element looks for `graph_wasm.wasm` next to the page. The threads build is fetched from beside the
`wasm` URL.

The host page's own CSP, `Content-Security-Policy: script-src 'self' 'wasm-unsafe-eval';
worker-src 'self'`, is the least the bundle needs. The gate serves exactly this policy.

### Threads need COOP and COEP on the host page

The threads build of the motor needs `SharedArrayBuffer`, which the browser grants only to a
cross-origin isolated page. So the host page itself must send:

```
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Embedder-Policy: require-corp
```

The service sends COEP and CORP on every `/embed/` file, and the proxy must pass them through
(nginx does by default). The motor worker picks the threads build only when the page is isolated
and the device has two cores or more.

Without the two headers the studio still works, on the serial build. That fallback is about 4x
slower on a big graph. At 1M nodes a live tick takes about 700 ms (657–758 ms) on the serial build,
against 158–170 ms with 8 threads (`docs/measurements/perf-p3-browser.md`, under SwiftShader, one
run per row). Rows `isolated-wasm-build` and `plain-wasm-build` read the host's request log to
check which build each page fetched.

The image serves one version: the one in `/srv/embed/VERSION`. When the image is upgraded, every
other `<version>` returns 404, so the host updates its tags in the same release.

## The gate

```sh
scripts/orch/gate.sh <logdir> scripts/orch/rows/service-image.rows
```

`scripts/service-image.sh` builds the image, scans its files, runs it detached the way `run` does,
and drives `deploy/nav/service.py` in `gm-chromium` on the container's network. Every container
goes through `scripts/orch/drun`. The report goes to `target/service-image/<label>/` as `table.md`,
`report.json` and screenshots. The rows:

- the container: `svc-non-root`, `svc-healthy`, `svc-no-leak`;
- HTTP: `/healthz`, `/v1/meta` with and without the key, and the headers on each embed file;
- `svc-embed-unversioned`;
- the browser: per page, the smoke gate's load rows plus isolation and the wasm build fetched, and
  then `direct-refused`.

The negative controls are `SERVICE_IMAGE_BREAK=headers` (a pass-through strips COOP, COEP and CORP)
and `SERVICE_IMAGE_BREAK=leak` (the scanned image gains `embed/.env` and a `.git`).

Row `svc-sdk-live` (`SERVICE_IMAGE_CHECK=sdk`) runs the SDK's
`crates/graph-sdk-js/scripts/live-check.mjs` against the same image. It checks `meta()`, parity
with the local wasm build, and the typed 401 and 400. Its negative control,
`SERVICE_IMAGE_BREAK=key`, hands it a well-formed key that the key file does not hold. On a tree
without `live-check.mjs` the row exits 2, "could not run".

A control passes only on exit 1 together with its own failure in the report: the embed header
rows and `isolated-isolation` for `headers`, `svc-no-leak` and the planted paths in `leaks.txt`
for `leak`, and `FAIL meta` next to a passing wrong-key check for `key`. Each also needs the
service itself up. An exit of 2, or a red row for some other reason, turns the control red.

## What it does not do

- **No TLS.** The host's proxy terminates TLS.
- **No persistence.** No state crosses requests and nothing is written. The service reads the embed
  tree once at start, into memory, and after that only the key file, again on each `SIGHUP`.
- **No direct cross-origin embed.** That needs the proxy, as described above.
- **No fixtures in the bundle.** The studio opens on its synthetic source.
- **One embed version per image**, with no history of older bundles.
- **The nginx snippet has not been run.** The gate's proxy is the Python pass-through in
  `deploy/nav/serviceproxy.py`, which forwards headers the same way. Caveat: a proxy that strips
  or rewrites `Cross-Origin-*` headers leaves the page unisolated, and the studio then runs on one
  thread.
