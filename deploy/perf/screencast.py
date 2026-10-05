"""A CDP screencast recorder: every frame Chromium paints, written to disk with its timestamp.

`Recorder` is a smokecdp.Watcher that also answers `Page.screencastFrame`: each frame is written as
a JPEG under `out/frames/`, acked at once (Chromium sends the next frame only after the ack), and
kept out of `events`, so a two-minute recording does not hold gigabytes of base64 in memory.

Frames arrive only while a call is reading the socket, so a still scene is held with `hold`, which
keeps reading, never with `time.sleep`, which would stall the stream. `pause`/`resume` drop a
stretch from the recording (a 1M-node load): `timeline()` then lays the segments end to end.

Caveat: Chromium sends a frame only when the compositor produces one, so a frame's duration is the
gap to the next one, and the first frame after `resume` is whatever was on screen then. A frame
whose ack is slow is dropped by Chromium rather than queued, so the frame rate is what the page and
the encoder kept up with, not a constant: `timeline()` returns real durations and the encoder
resamples to a constant rate.
"""
import base64
import os
import select
import time

import smokecdp

SCREENCAST = {"format": "jpeg", "quality": 92, "everyNthFrame": 1}


class Recorder(smokecdp.Watcher):
    def __init__(self, port, out):
        self.frames = []      # (path, wall seconds)
        self.segments = []    # [start, end) wall seconds of every recorded stretch
        self.marks = []       # (label, wall seconds)
        self.recording = False
        self.frame_dir = os.path.join(out, "frames")
        os.makedirs(self.frame_dir, exist_ok=True)
        super().__init__(port)

    def _receive(self):
        message = super()._receive()
        if message.get("method") == "Page.screencastFrame":
            self.events.pop()
            self._frame(message["params"])
        return message

    def _frame(self, params):
        self._next_id += 1
        self._send({"id": self._next_id, "method": "Page.screencastFrameAck",
                    "params": {"sessionId": params["sessionId"]}})
        if not self.recording:
            return
        path = os.path.join(self.frame_dir, f"f{len(self.frames):06d}.jpg")
        with open(path, "wb") as file:
            file.write(base64.b64decode(params["data"]))
        stamp = params.get("metadata", {}).get("timestamp") or time.time()
        self.frames.append((path, stamp))

    def resume(self, width, height):
        self.call("Page.startScreencast", {**SCREENCAST, "maxWidth": width, "maxHeight": height})
        self.recording = True
        self.segments.append([time.time(), None])

    def pause(self):
        self.recording = False
        self.segments[-1][1] = time.time()
        self.call("Page.stopScreencast")

    def mark(self, label):
        self.marks.append((label, time.time()))

    def hold(self, seconds):
        """Read the socket for `seconds`, acking every frame that arrives."""
        deadline = time.monotonic() + seconds
        while (left := deadline - time.monotonic()) > 0:
            readable, _, _ = select.select([self._sock], [], [], left)
            if readable:
                self._sock.settimeout(30)
                self._receive()

    def timeline(self):
        """(frames as (path, seconds on screen), marks as (label, video seconds)), pauses cut."""
        def video_time(wall):
            offset = 0.0
            for start, end in self.segments:
                if wall < start:
                    return offset
                if end is None or wall <= end:
                    return offset + wall - start
                offset += end - start
            return offset
        times = [video_time(stamp) for _, stamp in self.frames]
        total = video_time(self.segments[-1][1] or time.time())
        durations = [later - now for now, later in zip(times, times[1:] + [total])]
        frames = [(path, max(gap, 0.0)) for (path, _), gap in zip(self.frames, durations)]
        return frames, [(label, video_time(wall)) for label, wall in self.marks]
