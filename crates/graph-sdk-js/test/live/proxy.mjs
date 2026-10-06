// A forwarding proxy in front of the live hub, for the faults a real hub cannot be asked to make:
// it records every request and lets a test answer one locally, hold it, cut the socket after the
// hub committed it, rewrite its answer, or end an event stream after its first `change`.
import http from "node:http";

/**
 * Starts a proxy on 127.0.0.1 to `target` (the hub's base URL). `rule(match, action, times)` adds
 * a rule consulted in order; the first live one whose `match(request)` holds decides the request,
 * and `times` (default 1, `Infinity` for always) counts it down. With no rule the request is
 * forwarded unchanged.
 */
export async function startProxy(target) {
  const seen = [];
  const rules = [];
  const server = http.createServer((req, res) => {
    // A refused or failed forward must still end the client's request, or the test waits on it.
    handle(target, rules, seen, req, res).catch(() => res.destroy());
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const { port } = server.address();
  return {
    url: `http://127.0.0.1:${port}`,
    seen,
    rule(match, action, times = 1) {
      rules.push({ match, action, times });
    },
    close: () =>
      new Promise((resolve) => {
        server.closeAllConnections();
        server.close(resolve);
      }),
  };
}

async function handle(target, rules, seen, req, res) {
  const body = await bodyOf(req);
  const entry = {
    method: req.method,
    url: req.url,
    key: req.headers["idempotency-key"] ?? null,
    status: null,
  };
  seen.push(entry);
  const action = take(rules, req) ?? {};
  if (action.before !== undefined) await action.before();
  if (action.reply !== undefined) return replyLocally(res, action.reply, entry);
  const upstream = await forward(target, req, body);
  entry.status = upstream.statusCode;
  // A client that stops reading (a stopped subscriber) closes its side; the hub's side of an
  // event stream would otherwise stay open past the test.
  res.on("close", () => upstream.destroy());
  if (action.drop) return dropAfterCommit(upstream, req);
  if (action.untilChange) return streamUntilChange(upstream, res);
  if (action.body !== undefined || action.headers !== undefined) return rewritten(upstream, res, action);
  res.writeHead(upstream.statusCode, upstream.headers);
  upstream.pipe(res);
}

function take(rules, req) {
  const rule = rules.find((r) => r.times > 0 && r.match(req));
  if (rule === undefined) return undefined;
  rule.times -= 1;
  return rule.action;
}

function bodyOf(req) {
  return new Promise((resolve, reject) => {
    const chunks = [];
    req.on("data", (chunk) => chunks.push(chunk));
    req.on("end", () => resolve(Buffer.concat(chunks)));
    req.on("error", reject);
  });
}

function forward(target, req, body) {
  const url = new URL(req.url, target);
  return new Promise((resolve, reject) => {
    const out = http.request(url, { method: req.method, headers: { ...req.headers, host: url.host } }, resolve);
    out.on("error", reject);
    out.end(body);
  });
}

function replyLocally(res, reply, entry) {
  entry.status = reply.status;
  res.writeHead(reply.status, reply.headers ?? {});
  res.end(reply.body ?? "");
}

// The hub has answered, so whatever it did is committed; the client gets no answer at all, which
// is a transport failure to it and the case an idempotency key exists for.
function dropAfterCommit(upstream, req) {
  upstream.resume();
  upstream.on("end", () => req.socket.destroy());
}

function streamUntilChange(upstream, res) {
  res.writeHead(upstream.statusCode, upstream.headers);
  let text = "";
  upstream.setEncoding("utf8");
  upstream.on("data", (chunk) => {
    text += chunk;
    const frames = text.split("\n\n");
    text = frames.pop() ?? "";
    for (const frame of frames) {
      res.write(`${frame}\n\n`);
      if (/^event: change$/m.test(frame)) {
        upstream.destroy();
        res.end();
        return;
      }
    }
  });
}

async function rewritten(upstream, res, action) {
  const text = (await bodyOf(upstream)).toString("utf8");
  const headers = { ...upstream.headers };
  delete headers["content-length"];
  const out = action.body === undefined ? text : action.body(text);
  res.writeHead(upstream.statusCode, action.headers === undefined ? headers : action.headers(headers));
  res.end(out);
}
