# Example plugin: a JSON rows file into a graph-hub workspace

The smallest `graph-hub` plugin there is. It reads a rows file, maps it with
`rowsToIngest` (`src/adapters/rows.ts`), declares what its collections mean, and calls
`plugin.sync` — which reads the plugin's own stored ids from the hub and sends only the
difference. Nothing here reads `/graph`, so a key holding just `write:<plugin>` is enough.

## Running it

    GRAPH_HUB_URL=http://127.0.0.1:8081 \
    GRAPH_HUB_KEY=<a key with write:rows-file> \
    GRAPH_HUB_PLUGIN=rows-file \
    node --experimental-strip-types examples/plugins/rows-file/sync.mjs

Four environment names:

| name | meaning |
| --- | --- |
| `GRAPH_HUB_URL` | the hub's root. Defaults to `http://127.0.0.1:8081`. |
| `GRAPH_HUB_KEY` | the API key, sent as `Authorization: Bearer`. Omit for a hub with the auth off. |
| `GRAPH_HUB_PLUGIN` | the plugin id, and so the name of the collection's namespace. Defaults to `rows-file`. |
| `GRAPH_HUB_ROWS` | the rows file. Defaults to this directory's `rows.json`. |

It prints `<n> batches`, one line, and exits 0.

## The `hub-sdk` row

    hub-sdk|0|scripts/orch/timed scripts/orch/gate.sh <logdir>/hub-sdk scripts/orch/rows/hub-sdk-live.rows

That row runs this `sync.mjs` against live graph-hub, PostgreSQL and graph-server containers,
and then re-runs it with `GM_HUB_SDK_BREAK=1`, which makes it push one record that is not in the
rows file. The second run must fail; that is the negative control.

## Caveat

The rows file is read on every run, so a run is only as fresh as the file: edit `rows.json` (or
point `GRAPH_HUB_ROWS` elsewhere) and run again. There is no polling and no watch, and `sync`
re-sends every desired record on every call — §7's own caveat — so a large file is a large
request every time. This is an example of the shape of a plugin, not a pipeline to leave
running.
