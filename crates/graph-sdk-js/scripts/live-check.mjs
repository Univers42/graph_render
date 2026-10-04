// The svc-sdk check against a running service, the key in env `GRAPH_API_KEY`:
//
//   GRAPH_API_KEY=... node --experimental-strip-types crates/graph-sdk-js/scripts/live-check.mjs <baseUrl>
//
// It checks `meta()`, every `parity.mjs` case against the local wasm build (build it first:
// `scripts/orch/gr cargo build -p graph-wasm --release --target wasm32-unknown-unknown`), a
// wrong key as the typed 401, and `post=a,b` as a typed 400 (verdict C6; the SDK refuses that
// request itself, so this one call goes around it). One line per check. It prints SDK error
// messages, which never carry the key, and its own lines, which never name it.
//
// Exit 0 = passed, 1 = ran and failed, 2 = could not run (no baseUrl, no key, no wasm build,
// no response from the service).

import { randomBytes } from "node:crypto";
import { createRemote, RemoteError, SNAPSHOT_MEDIA_TYPE } from "../src/remote.ts";
import { baseOf } from "../src/remote/options.ts";
import { scrub } from "../src/remote/errors.ts";
import { CASES, caseDifferences, loadMotor } from "./parity.mjs";

const describe = (error) => (error instanceof Error ? error.message : String(error));

/** `[]` when `run` resolves to `[]`; otherwise its lines, or the error it threw. */
async function linesOf(run) {
  try {
    return await run();
  } catch (error) {
    return [describe(error)];
  }
}

async function metaLines(remote) {
  const meta = await remote.meta();
  const wanted = [...CASES.map((testCase) => testCase.request.layout), ...CASES.flatMap((testCase) => testCase.request.post ?? [])];
  const missing = wanted.filter((id) => !meta.layouts.includes(id) && !meta.posts.includes(id));
  return missing.length === 0 ? [] : [`meta() does not list ${missing.join(", ")}`];
}

async function wrongKeyLines(baseUrl) {
  const stranger = createRemote({ baseUrl, apiKey: `gm_${randomBytes(32).toString("base64url")}` });
  const error = await stranger.meta().then(() => undefined, (thrown) => thrown);
  if (error instanceof RemoteError && error.status === 401 && error.codeName === "Unauthorized") return [];
  return [`a wrong key answered ${error === undefined ? "200" : describe(error)}`];
}

async function twoPostLines(baseUrl, apiKey) {
  const url = `${baseOf(baseUrl)}/v1/layout?layout=layout.tree.tidy&post=post.style.bezier,post.style.bezier`;
  const headers = { Authorization: `Bearer ${apiKey}`, Accept: SNAPSHOT_MEDIA_TYPE, "Content-Type": "application/json" };
  let response;
  try {
    response = await fetch(url, { method: "POST", headers, body: "{}", redirect: "error" });
  } catch (error) {
    return [`post=a,b: no response: ${scrub(describe(error), apiKey)}`];
  }
  const body = await response.json().catch(() => ({}));
  const typed = new RemoteError("", { status: response.status, error: typeof body.error === "string" ? body.error : undefined });
  if (response.status === 400 && typed.codeName !== undefined) return [];
  return [`post=a,b answered HTTP ${response.status} ${scrub(String(body.error ?? "with no error body"), apiKey)}`];
}

function report(name, lines) {
  console.log(lines.length === 0 ? `PASS ${name}` : `FAIL ${name}\n  ${lines.join("\n  ")}`);
  return lines.length === 0;
}

/** The inputs, or `undefined` after saying why they are missing. */
async function inputsOf() {
  const baseUrl = process.argv[2];
  const apiKey = process.env.GRAPH_API_KEY;
  if (baseUrl === undefined || apiKey === undefined || apiKey === "") {
    console.error("usage: GRAPH_API_KEY=<key> live-check.mjs <baseUrl>");
    return undefined;
  }
  try {
    return { baseUrl, apiKey, remote: createRemote({ baseUrl, apiKey }), motor: await loadMotor() };
  } catch (error) {
    console.error(`could not run: ${describe(error)}`);
    return undefined;
  }
}

async function main() {
  const inputs = await inputsOf();
  if (inputs === undefined) return 2;
  const { baseUrl, apiKey, remote, motor } = inputs;
  const meta = await linesOf(() => metaLines(remote));
  if (meta.length > 0 && /: no response: /.test(meta[0])) {
    console.error(`could not run: ${meta[0]}`);
    return 2;
  }
  let passed = report("meta", meta);
  for (const testCase of CASES) passed = report(`parity: ${testCase.name}`, await linesOf(() => caseDifferences(remote, motor, testCase))) && passed;
  passed = report("wrong key is the typed 401", await linesOf(() => wrongKeyLines(baseUrl))) && passed;
  passed = report("post=a,b is a typed 400", await linesOf(() => twoPostLines(baseUrl, apiKey))) && passed;
  return passed ? 0 : 1;
}

process.exitCode = await main();
