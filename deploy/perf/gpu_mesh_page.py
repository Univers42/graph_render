"""The two strings the `gpu-mesh` probe page is made of, and the keys they interpolate.

`gpu-mesh.py` is the harness — the flags, the browser, the verdicts. This file is the page it
opens: a one-line HTML shell whose module script exposes `window.gpuMesh(name, arm, fault)`, and
one JavaScript expression the harness evaluates to ask the page what adapter it can see. They are
here rather than inline because they are the only two places in the harness that are JavaScript,
and a Python file that is mostly JavaScript is a file whose line budget is spent on the wrong
language.

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
              "maxComputeInvocationsPerWorkGroup", "maxComputeWorkgroupsPerDimension")

PAGE = """<!doctype html><meta charset="utf-8"><title>gpu-mesh</title>
<script type="module">
import { runCharge } from "/target/gpu-js/gpu/charge.js";
window.gpuMesh = async (name, arm, fault) => {
  const bytes = await (await fetch("/target/gpu-fixtures/" + name + ".gmfx")).arrayBuffer();
  return await runCharge({ fixture: bytes, arm }, fault);
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
