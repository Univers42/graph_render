"""The JavaScript `deploy/perf/open-document.py` evaluates, as constants.

Split out of that probe so it stays inside the house's 300-line limit: the four snippets are the
page-side half of the probe and read better beside each other than four literals among the CDP
calls. Nothing here changes behaviour — the probe imports these names and uses them as it used the
literals. Standard library only, like the probe.
"""

# The studio's own action, with the file control's own params. The shared driver
# (deploy/perf/drivers/hook.js) has no document opener, so the dispatch the dock's file control makes
# is spelled out here rather than added to a file both probes share. `__gmOpen` is the promise the
# probe's poll watches; `__gmSettled` is what it leaves behind, including on a refusal or a throw.
OPEN_DOCUMENT = """(async (name, url) => {
  window.__gmSettled = null;
  const started = performance.now();
  const answer = await fetch(url);
  if (!answer.ok) throw new Error(`the document server refused ${url}: ${answer.status}`);
  const text = await answer.text();
  const studio = document.querySelector('graph-studio')?.studio ?? null;
  if (studio === null) throw new Error('open-document: no <graph-studio> with a studio on the page');
  const entry = await studio.dispatch('source.document', { name, text });
  const done = { ms: Math.round(performance.now() - started), message: entry.ok
    ? entry.message : `REFUSED ${entry.command}: ${entry.message}` };
  window.__gmSettled = done;
})"""

# A refused or thrown open still has to leave `__gmSettled` behind: nothing else watches the page.
CAUGHT = """(error) => {
  window.__gmSettled = { error: String(error?.message ?? error) };
}"""

# What the poll reads each time round: `running` while the dispatch is in flight.
SETTLED = "(window.__gmSettled === null ? 'running' : JSON.stringify(window.__gmSettled))"

# The page's JavaScript heap in MB, or null where the browser does not report it.
HEAP = """(() => {
  const memory = performance.memory;
  return memory === undefined ? null : Math.round(memory.usedJSHeapSize / 1048576);
})()"""