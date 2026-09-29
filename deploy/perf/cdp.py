"""Chrome DevTools Protocol client for the studio perf gate. Standard library only.

Ponytail: single-threaded and event-blind. It reads frames until the reply with the
matching id arrives and drops everything else, so it cannot wait on a CDP event (page
load is polled through `document.readyState` instead). One page target is assumed.
"""

import base64
import json
import os
import socket
import struct
import time
import urllib.request

OPCODE_CLOSE = 0x8


class CdpError(RuntimeError):
    """The browser refused a call, or the page threw."""


def _handshake(port, path):
    sock = socket.create_connection(("127.0.0.1", port), timeout=30)
    key = base64.b64encode(os.urandom(16)).decode()
    request = (
        f"GET {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nUpgrade: websocket\r\n"
        f"Connection: Upgrade\r\nSec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n\r\n"
    )
    sock.sendall(request.encode())
    head = b""
    while b"\r\n\r\n" not in head:
        chunk = sock.recv(4096)
        if not chunk:
            raise CdpError("browser closed the socket during the websocket handshake")
        head += chunk
    if b" 101 " not in head.split(b"\r\n", 1)[0]:
        raise CdpError(f"websocket upgrade refused: {head[:80]!r}")
    return sock


def wait_for_browser(port, seconds=30):
    """The page target's websocket path, once the browser answers on `port`."""
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        try:
            with urllib.request.urlopen(f"http://127.0.0.1:{port}/json", timeout=2) as reply:
                pages = [t for t in json.load(reply) if t["type"] == "page"]
            if pages:
                return pages[0]["webSocketDebuggerUrl"].split(f":{port}", 1)[1]
        except OSError:
            pass
        time.sleep(0.2)
    raise CdpError(f"no page target on port {port} after {seconds}s")


class Page:
    """One page target. Every call is synchronous."""

    def __init__(self, port):
        self._sock = _handshake(port, wait_for_browser(port))
        self._next_id = 0

    def _send(self, payload):
        data = json.dumps(payload).encode()
        head = bytearray([0x81])
        size = len(data)
        if size < 126:
            head.append(0x80 | size)
        elif size < 65536:
            head.append(0x80 | 126)
            head += struct.pack(">H", size)
        else:
            head.append(0x80 | 127)
            head += struct.pack(">Q", size)
        mask = os.urandom(4)
        body = bytes(byte ^ mask[i % 4] for i, byte in enumerate(data))
        self._sock.sendall(bytes(head) + mask + body)

    def _read(self, size):
        out = b""
        while len(out) < size:
            chunk = self._sock.recv(size - len(out))
            if not chunk:
                raise CdpError("browser closed the socket")
            out += chunk
        return out

    def _receive(self):
        message = b""
        while True:
            first, second = self._read(2)
            if first & 0x0F == OPCODE_CLOSE:
                raise CdpError("browser sent a websocket close frame")
            size = second & 0x7F
            if size == 126:
                size = struct.unpack(">H", self._read(2))[0]
            elif size == 127:
                size = struct.unpack(">Q", self._read(8))[0]
            message += self._read(size)
            if first & 0x80:
                return json.loads(message)

    def call(self, method, params=None, timeout=60):
        self._next_id += 1
        ident = self._next_id
        self._sock.settimeout(timeout)
        self._send({"id": ident, "method": method, "params": params or {}})
        while True:
            reply = self._receive()
            if reply.get("id") != ident:
                continue
            if "error" in reply:
                raise CdpError(f"{method}: {reply['error'].get('message')}")
            return reply.get("result", {})

    def evaluate(self, expression, timeout=180):
        """The JSON value `expression` resolves to. A page exception raises."""
        result = self.call("Runtime.evaluate", {
            "expression": expression, "awaitPromise": True, "returnByValue": True,
            "timeout": timeout * 1000,
        }, timeout=timeout + 10)
        if "exceptionDetails" in result:
            detail = result["exceptionDetails"]
            text = detail.get("exception", {}).get("description") or detail.get("text")
            raise CdpError(f"page threw: {text}")
        return result.get("result", {}).get("value")

    def set_viewport(self, width, height, dpr):
        self.call("Emulation.setDeviceMetricsOverride", {
            "width": width, "height": height, "deviceScaleFactor": dpr, "mobile": False,
        })

    def navigate(self, url, seconds=30):
        self.call("Page.navigate", {"url": url})
        deadline = time.monotonic() + seconds
        while time.monotonic() < deadline:
            state = self.evaluate("document.readyState + '|' + location.href")
            if state == f"complete|{url}":
                return
            time.sleep(0.1)
        raise CdpError(f"{url} did not finish loading in {seconds}s")

    def screenshot(self, path):
        shot = self.call("Page.captureScreenshot", {"format": "png"})
        with open(path, "wb") as out:
            out.write(base64.b64decode(shot["data"]))
