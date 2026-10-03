"""The page a load-smoke gate needs: one that keeps the frames the shared CDP client drops.

`deploy/perf/cdp.py` is event-blind on purpose (its own docstring says so): `call` reads frames
until the reply with the matching id arrives and throws the rest away. This subclass keeps every
event frame it passes over, so the gate can read what the page and the motor worker threw and
logged. Nothing else in the repository changes to get that, and the perf gate's recorded numbers
are untouched.

The motor runs in a Worker, and the fault this gate exists for (2026-10-01: a stale
`graph_wasm.wasm`) happens inside that worker — `wasm.ts` catches the loader's refusal and calls
`console.error` there, not on the page. `Target.setAutoAttach` attaches to the worker and
`Runtime.enable` on its session is what brings that line, and any uncaught worker error, into the
same buffer as the page's.

Ponytail: `watch_workers` must be called while the page is being polled, not once at the end. A
CDP event sits in the socket until something reads, and this client only reads inside `call`, so
a worker that attached after the last enable would have its console line land in the buffer only
by accident of the next poll's timing. Polling makes it a matter of the next 200 ms.
"""

import cdp

# A valid wasm module exporting `memory` and nothing else — the negative control's fault: a module
# of the right shape and the wrong contents, which is what a stale or half-written graph_wasm.wasm
# looks like to the loader. 25 bytes: one memory section and one export section.
MEMORY_ONLY_WASM = bytes([
    0x00, 0x61, 0x73, 0x6D, 0x01, 0x00, 0x00, 0x00,  # \0asm, version 1
    0x05, 0x03, 0x01, 0x00, 0x01,  # section 5 (memory), 3 bytes: one page, no maximum
    0x07, 0x0A, 0x01, 0x06, 0x6D, 0x65, 0x6D, 0x6F, 0x72, 0x79,  # section 7 (export), 10 bytes
    0x02, 0x00,  # its one export, "memory", is memory 0
])

# The domains a load smoke reads. `Runtime` carries exceptions and console calls, `Log` carries the
# browser's own errors (a refused request, a bad MIME type) and, on this target, the worker's.
# `Page` is the page's alone: a worker has no Page domain and asking for one is refused.
DOMAINS = ("Runtime", "Log")
PAGE_DOMAINS = (*DOMAINS, "Page")

# The negative control's second fault, and the only one that reaches an exception row. The
# 2026-10-01 incident threw `TypeError: exports.gm_dim is not a function` out of the motor worker;
# the SDK since latched the loader's refusal instead (crates/graph-sdk-js/src/index.ts), so that
# same stale wasm now reaches the page as store state and a console line and never as a thrown
# exception. The exception row's control therefore has to be a throw, and the laziest one is a
# script that throws before any studio code runs — injected by the harness, so nothing in app/ or
# packages/ is touched and the control cannot mask a real regression.
INJECTED_THROW = "throw new Error('studio-smoke negative control: the page threw on load');"


class Watcher(cdp.Page):
    """One page target, plus every event frame seen from load onwards."""

    def __init__(self, port):
        self.events = []
        self.sessions = []
        super().__init__(port)

    def _receive(self):
        message = super()._receive()
        if "method" in message:
            self.events.append(message)
        return message

    def session_call(self, session_id, method, params=None, timeout=30):
        """`call` on an auto-attached target, with no domain support in the shared client.

        A target that detaches first never answers: a retired motor worker with a thread pool
        takes about 3 s to end (studio-embed, isolated run, 2026-10-04) and ignores every call
        meanwhile. That is a refusal now, not a wait for the timeout.
        """
        self._next_id += 1
        ident = self._next_id
        self._sock.settimeout(timeout)
        self._send({"id": ident, "method": method, "params": params or {}, "sessionId": session_id})
        while True:
            reply = self._receive()
            if (reply.get("method") == "Target.detachedFromTarget"
                    and reply.get("params", {}).get("sessionId") == session_id):
                raise cdp.CdpError(f"{method} on {session_id}: the target detached before it answered")
            if reply.get("id") != ident:
                continue
            if "error" in reply:
                raise cdp.CdpError(f"{method} on {session_id}: {reply['error'].get('message')}")
            return reply.get("result", {})

    def detached(self, session):
        return any(event["method"] == "Target.detachedFromTarget"
                   and event.get("params", {}).get("sessionId") == session for event in self.events)

    def watch_workers(self):
        """Turn the error domains on for every target auto-attached since the last call.

        A target that detached before its domains were on (a page navigated away, a retired motor
        worker) has nothing left to watch: skipped, but only once the browser has said it
        detached. Any other refusal still raises.
        """
        for event in self.events:
            params = event.get("params", {})
            session = params.get("sessionId") if event["method"] == "Target.attachedToTarget" else None
            if session is None or session in self.sessions:
                continue
            self.sessions.append(session)
            self.enable_on(session)

    def enable_on(self, session):
        for domain in DOMAINS:
            if self.detached(session):
                return
            try:
                self.session_call(session, f"{domain}.enable")
            except cdp.CdpError:
                if not self.detached(session):
                    raise
                return

    def start_watching(self):
        """Before the first navigation: the error domains, and auto-attach for the motor worker."""
        for domain in PAGE_DOMAINS:
            self.call(f"{domain}.enable")
        self.call("Target.setAutoAttach", {"autoAttach": True, "waitForDebuggerOnStart": False,
                                           "flatten": True})

    def throw_on_load(self, source):
        """The negative control's second fault: `source` runs, and throws, before the page's own."""
        self.call("Page.addScriptToEvaluateOnNewDocument", {"source": source})
