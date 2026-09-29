"""Read the pixels of a PNG, with no image library in the gate's image.

Only what a Chromium screenshot is: 8 bits per channel, no interlacing, RGB or RGBA or
grey. The zlib stream is the only compression, and the filters are the five of the spec.
A region can be limited, because the filter chain runs down the image: decoding the top
left 1403x1001 of a 1920x1080 frame is the whole of what the gate samples and a third of
the work.
"""

import struct
import zlib

CHANNELS = {0: 1, 2: 3, 4: 2, 6: 4}


class PngError(RuntimeError):
    """The file is not a PNG this reader can read."""


class Image:
    """The decoded pixels, as rows of bytes, and where they came from."""

    def __init__(self, width, height, channels, rows):
        self.width = width
        self.height = height
        self.channels = channels
        self.rows = rows

    def pixel(self, x, y):
        """The three channels at (x, y), or None when the point is off the image."""
        if x < 0 or y < 0 or x >= self.width or y >= len(self.rows):
            return None
        line = self.rows[y]
        at = x * self.channels
        if at + 3 > len(line):
            return None
        if self.channels == 1:
            return (line[at], line[at], line[at])
        return (line[at], line[at + 1], line[at + 2])


def chunks(data):
    """(kind, body) for every chunk of a PNG, after the 8-byte signature."""
    at = 8
    while at + 8 <= len(data):
        size = struct.unpack(">I", data[at:at + 4])[0]
        yield data[at + 4:at + 8], data[at + 8:at + 8 + size]
        at += 12 + size


def header(parts):
    for kind, body in parts:
        if kind != b"IHDR":
            continue
        width, height, depth, colour = struct.unpack(">IIBB", body[:10])
        if depth != 8 or body[12] != 0:
            raise PngError(f"only 8-bit non-interlaced PNGs, got depth {depth} interlace {body[12]}")
        if colour not in CHANNELS:
            raise PngError(f"colour type {colour} is not one this reader knows")
        return width, height, CHANNELS[colour]
    raise PngError("no IHDR chunk")


def paeth(a, b, c):
    p = a + b - c
    pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
    if pa <= pb and pa <= pc:
        return a
    return b if pb <= pc else c


def unfilter(kind, line, previous, step):
    """One scanline of the region, unfiltered in place, over `step` bytes per pixel."""
    for i in range(len(line)):
        a = line[i - step] if i >= step else 0
        b = previous[i]
        c = previous[i - step] if i >= step else 0
        if kind == 1:
            line[i] = (line[i] + a) & 255
        elif kind == 2:
            line[i] = (line[i] + b) & 255
        elif kind == 3:
            line[i] = (line[i] + (a + b) // 2) & 255
        elif kind == 4:
            line[i] = (line[i] + paeth(a, b, c)) & 255
    return line


def read(path, limit=None):
    """The image at `path`, decoded down to `limit` = (width, height) of its top left."""
    data = open(path, "rb").read()
    parts = list(chunks(data))
    width, height, channels = header(parts)
    want_width, want_height = limit if limit is not None else (width, height)
    stride = min(width, want_width) * channels
    raw = zlib.decompress(b"".join(body for kind, body in parts if kind == b"IDAT"))
    rows = []
    previous = bytearray(stride)
    at = 0
    for _ in range(min(height, want_height)):
        kind = raw[at]
        line = bytearray(raw[at + 1:at + 1 + stride])
        unfilter(kind, line, previous, channels)
        rows.append(line)
        previous = line
        at += 1 + width * channels
    return Image(min(width, want_width), min(height, want_height), channels, rows)
