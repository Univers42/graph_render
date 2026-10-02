"""The pixels of a screenshot: PNG decoding and two pixel counts, standard library only.

`Page.captureScreenshot` writes a PNG and `gm-chromium` holds Debian packages and nothing else
(docs/decisions/images-from-debian.md), so the backend gate reads the two files it saved itself.
Only what Chromium writes is decoded — 8 bits a channel, no interlacing, colour type 2 (RGB) or
6 (RGBA); anything else raises rather than guesses. Alpha is dropped, so the two counts compare
colour, not the picture's own transparency.

The five row filters are undone the cheap way where they can be and one byte at a time where
they cannot: `None` is a copy, `Up` is a C-level `map(add, ...)`, and `Sub`, `Average` and
`Paeth` each read the byte already rebuilt to their left, which nothing in the standard library
does in bulk. A screenshot is mostly flat ground, so the first two are what a graph canvas is
made of.

Caveat: the background is the pixel at (0, 0). A graph whose top-left corner happens to hold a
mark is measured against that mark, and the "off the background" count then reads as near zero.
The corner is empty on every case this gate opens, and `off_background` is only ever a
blank-canvas claim, never a measure of what is drawn.
"""
import struct
import zlib
from itertools import chain, repeat
from operator import add, and_
from pathlib import Path

SIGNATURE = b"\x89PNG\r\n\x1a\n"
DEPTH, NON_INTERLACED = 8, 0
CHANNELS = {2: 3, 6: 4}
FILTERS = {0: "None", 1: "Sub", 2: "Up", 3: "Average", 4: "Paeth"}


class PngError(ValueError):
    """The file is not a PNG this decoder reads."""


def _chunks(data):
    if data[:8] != SIGNATURE:
        raise PngError("the screenshot is not a PNG")
    at = 8
    while at + 8 <= len(data):
        size = struct.unpack(">I", data[at:at + 4])[0]
        yield data[at + 4:at + 8], data[at + 8:at + 8 + size]
        at += 12 + size


def _shape(head):
    width, height, depth, colour, _, _, interlace = struct.unpack(">IIBBBBB", head)
    if depth != DEPTH or colour not in CHANNELS or interlace != NON_INTERLACED:
        raise PngError(f"unsupported PNG: depth {depth}, colour {colour}, interlace {interlace}")
    return width, height, CHANNELS[colour]


def _paeth(left, up, corner):
    estimate = left + up - corner
    da, db, dc = abs(estimate - left), abs(estimate - up), abs(estimate - corner)
    if da <= db and da <= dc:
        return left
    return up if db <= dc else corner


def _pred(kind, left, up, corner):
    """The byte the filter took off each of `line`'s bytes: itself, up, their mean, or Paeth."""
    if kind == 1:
        return left
    if kind == 3:
        return (left + up) >> 1
    return _paeth(left, up, corner)


def _row(filtered, prior, stride, bpp):
    """One scanline rebuilt: `filtered` is the filter byte and `stride` bytes of differences."""
    kind = filtered[0]
    line = filtered[1:]
    if kind == 0:
        return line
    if kind == 2:
        return bytes(map(and_, map(add, line, prior), repeat(0xFF)))
    if kind not in (1, 3, 4):
        raise PngError(f"unknown PNG row filter {kind}")
    out = bytearray(stride)
    for at in range(stride):
        left = out[at - bpp] if at >= bpp else 0
        corner = prior[at - bpp] if at >= bpp else 0
        out[at] = (line[at] + _pred(kind, left, prior[at], corner)) & 0xFF
    return bytes(out)


def read(path):
    """`(width, height, pixels)` for the PNG at `path`, three bytes a pixel in row order."""
    width = height = bpp = None
    body = bytearray()
    for kind, chunk in _chunks(Path(path).read_bytes()):
        if kind == b"IHDR":
            width, height, bpp = _shape(chunk)
        elif kind == b"IDAT":
            body += chunk
        elif kind == b"IEND":
            break
    if width is None:
        raise PngError("the screenshot has no IHDR")
    body = zlib.decompress(bytes(body))
    stride, prior, rows, at = width * bpp, bytes(width * bpp), [], 0
    for _ in range(height):
        prior = _row(body[at:at + stride + 1], prior, stride, bpp)
        at += stride + 1
        rows.append(prior)
    flat = b"".join(rows)
    if bpp == 3:
        return width, height, flat
    return width, height, bytes(chain.from_iterable(zip(flat[0::4], flat[1::4], flat[2::4])))


def differs(first, second, threshold):
    """`(over, total)`: how many of `first`'s pixels are more than `threshold` off `second`'s."""
    if len(first) != len(second):
        raise PngError("the two screenshots are not the same size")
    over = 0
    for at in range(0, len(first), 3):
        r = abs(first[at] - second[at])
        g = abs(first[at + 1] - second[at + 1])
        b = abs(first[at + 2] - second[at + 2])
        if r > threshold or g > threshold or b > threshold:
            over += 1
    return over, len(first) // 3


def fraction(over, total):
    """The count as a share of the pixels, to four decimals; 0.0 when there are no pixels."""
    return round(over / total, 6) if total else 0.0


def off_background(path, threshold=0):
    """The share of the PNG's pixels that differ from its own top-left pixel."""
    width, height, pixels = read(path)
    return fraction(*differs(pixels, pixels[:3] * (width * height), threshold))