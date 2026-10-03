import { readFile } from "node:fs/promises";
import { wasmBytes } from "./support.ts";

const bytes = (await wasmBytes()) ?? new Uint8Array(0);
const module = await WebAssembly.compile(bytes);
const names = WebAssembly.Module.imports(module).map((entry) => entry.module);
console.log("import namespaces", [...new Set(names)]);
const seen = new Map();
const made = (name) => (...args) => { seen.set(name, args.length); return 0; };
const proxy = new Proxy({}, { get: (_t, key) => (typeof key === "string" ? made(key) : undefined) });
for (const ns of new Set(names)) proxy[ns] = proxy;
const memory = new WebAssembly.Memory({ initial: 256 });
proxy.env = new Proxy({ memory }, { get: (t, key) => (key in t ? t[key] : made(String(key))) });
try {
  const instance = await WebAssembly.instantiate(module, { env: proxy });
  console.log("instantiated", Object.keys(instance.exports).length, "exports");
  console.log("gm_run arity", instance.exports.gm_run.length);
  console.log("gm_layout_params arity", instance.exports.gm_layout_params.length);
  console.log("gm_abi_version", instance.exports.gm_abi_version());
} catch (error) {
  console.log("could not instantiate:", String(error).slice(0, 300));
  console.log("imports", WebAssembly.Module.imports(module).map((e) => `${e.module}.${e.name}`).join(" "));
}
