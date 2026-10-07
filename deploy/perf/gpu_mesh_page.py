"""The two strings the `gpu-mesh` probe page is made of, and the keys they interpolate.

`gpu-mesh.py` is the harness — the flags, the browser, the verdicts. This file is the page it
opens: a one-line HTML shell whose module script exposes
`window.gpuMesh(name, arm, fault, pass)`, and one JavaScript expression the harness evaluates to
ask the page what adapter it can see. They are here rather than inline because they are the only
two places in the harness that are JavaScript, and a Python file that is mostly JavaScript is a
file whose line budget is spent on the wrong language.

**The fixture crosses by `fetch` over the harness's own origin, not over `import()`.** At 1M a
`.gmfx` is 100.7 MiB and a base64 handoff would be a 134 MiB string through `import()`
(`fixtures/gpu/README.md:188-191`). The page is served from the repository root, so the same
origin serves `/target/gpu-fixtures/` and `/target/gpu-js/`.

Caveat: the page is a shell, not a document — there is no error surface a person can read. A
failure shows up as a refused `page.evaluate`, which is why `run_fixtures` prints the CDP error
rather than swallowing it.
"""

INFO_KEYS = ("vendor", "architecture", "device", "description")
LIMIT_KEYS = ("maxStorageBufferBindingSize", "maxBufferSize", "maxComputeWorkgroupStorageSize",
              "maxComputeInvocationsPerWorkgroup", "maxComputeWorkgroupsPerDimension")

# The harness's readiness question. `navigate` returns on the load event, and a module script is
# deferred behind that, so `window.gpuMesh !== undefined` is a question about a page that has not
# finished arming — it answers `false` and the harness then calls a function that is not there.
# This polls instead, and on a timeout re-imports the module itself: a page whose import failed
# says so here, rather than as `TypeError: window.gpuMesh is not a function` on every fixture.
READY_JS = """(async () => {
  const deadline = Date.now() + 60000;
  while (typeof window.gpuMesh !== 'function') {
    if (Date.now() > deadline) {
      try { await import('/target/gpu-js/gpu/charge.js'); return 'module loaded, gpuMesh absent'; }
      catch (error) { return 'import failed: ' + error; }
    }
    await new Promise((done) => setTimeout(done, 50));
  }
  return 'ready';
})()"""

# The pass dispatch: `gpu/<pass>.js` and its `run<Pass>(request, fault)` export, `runCharge`,
# `runLink`, `runCollide` or `runTick`; `live` is `gpu/live-probe.js`'s `runLive`, which also
# takes the wasm module's bytes, fetched here. A module without the export is a throw naming it, not a
# call on `undefined` that reads as a kernel fault. The tick's report carries the final f32
# positions, which the harness writes for `gpu-stress`; they cross as a plain array because a
# Float32Array does not survive the CDP's JSON.
PAGE = """<!doctype html><meta charset="utf-8"><title>gpu-mesh</title>
<script type="module">
window.gpuMesh = async (name, arm, fault, pass = "charge", ticks = 1) => {
  const file = pass === "live" ? "live-probe" : pass;
  const module = await import("/target/gpu-js/gpu/" + file + ".js");
  const entry = "run" + pass[0].toUpperCase() + pass.slice(1);
  if (typeof module[entry] !== "function") { throw new Error(file + ".js has no " + entry); }
  const bytes = await (await fetch("/target/gpu-fixtures/" + name + ".gmfx")).arrayBuffer();
  const request = { fixture: bytes, arm, ticks };
  if (pass === "live") {
    const wasm = await fetch("/target/wasm32-unknown-unknown/release/graph_wasm.wasm");
    if (!wasm.ok) { throw new Error("the wasm module is not built: " + wasm.status); }
    request.wasm = await wasm.arrayBuffer();
  }
  const report = await module[entry](request, fault);
  if (pass === "tick" && report.finalPositions) {
    return { ...report, finalPositions: Array.from(report.finalPositions) };
  }
  return report;
};
</script>"""


def json_array(keys):
    """The tuple as a JavaScript array literal, for the expressions interpolated below."""
    return "[" + ", ".join(f'"{key}"' for key in keys) + "]"


ASK_JS = """(async () => {
  const out = {present: ('gpu' in navigator), adapter: false, fallback: false, marks: ''};
  if (!out.present) { out.why = "no navigator.gpu"; return out; }
  let adapter = null;
  try { adapter = await navigator.gpu.requestAdapter({powerPreference: 'high-performance'}); }
  catch (error) { out.why = 'requestAdapter threw: ' + error; return out; }
  if (!adapter) { out.why = 'adapter none'; return out; }
  out.adapter = true;
  const info = adapter.info || {};
  const parts = [];
  for (const key of %s) {
    const value = info[key];
    parts.push(key + ' ' + (value === undefined || value === '' ? '(absent)' : value));
    if (value) { out.marks = out.marks + ' ' + value; }
  }
  out.lines = parts;
  const fallback = adapter.isFallbackAdapter !== undefined
    ? adapter.isFallbackAdapter : info.isFallbackAdapter;
  out.fallback = fallback === true;
  out.lines.push('isFallbackAdapter ' + fallback);
  for (const key of %s) { out.lines.push('limit ' + key + ' ' + adapter.limits[key]); }
  out.features = Array.from(adapter.features).sort();
  out.lines.push('features ' + out.features.join(','));
  return out;
})()""" % (json_array(INFO_KEYS), json_array(LIMIT_KEYS))
