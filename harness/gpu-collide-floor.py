#!/usr/bin/env python3
"""f32 input floor of the collide pass — is the collide guard attainable at a fixture?

The GPU arm narrows every position to f32 before it hashes or resolves
(`gpu/collide.ts`, `Math.fround`). This script computes the collide delta in f64 three ways:
  (a) from the fixture's f64 positions — validates the transcription against delta_collide;
  (b) from the positions narrowed to f32, arithmetic still f64 — the input floor, the error
      no f32 implementation can go under;
  (c) from the narrowed positions with f32 arithmetic per pair — the arm's own arithmetic.
Each is reported as rmsAbs / rmsRef / rmsRel / maxAbs against the fixture column, with the
fixture's coordinate extent and the Amendment-1 guard for comparison.

Caveat: pairs come from a k-d tree over every pair with d² < reach², which is the CPU's set
only while no cell overflows its 256-candidate window (the CPU's window is exact there); the
coincidence jiggle (dx or dy exactly 0) is skipped, so a fixture with coincident nodes reads
slightly wrong in (a) and the script says so through (a)'s residual.
"""
import struct
import sys

import numpy as np
from scipy.spatial import cKDTree

U23 = 2.0 ** -23


def load(path):
    data = open(path, "rb").read()
    n, m, P = struct.unpack_from("<III", data, 12)
    state = struct.unpack_from("<I", data, 24)[0]
    off = 64 + 8 + 16 * m
    px = np.frombuffer(data, "<f8", n, off); off += 8 * n
    py = np.frombuffer(data, "<f8", n, off); off += 8 * n
    off += 16 * P + 16 * P * P + 32 * n
    cx = np.frombuffer(data, "<f8", n, off); off += 8 * n
    cy = np.frombuffer(data, "<f8", n, off)
    return n, state, px, py, cx, cy


def deltas(px, py, pairs, reach, dtype):
    """Each node's half of every overlap, `collide.rs::resolve` per pair, in `dtype`."""
    x = px.astype(dtype); y = py.astype(dtype)
    i, j = pairs[:, 0], pairs[:, 1]
    dx = (x[i] - x[j]).astype(dtype); dy = (y[i] - y[j]).astype(dtype)
    dist = np.sqrt(dx * dx + dy * dy).astype(dtype)
    keep = (dist > 0) & (dist < dtype(reach))
    push = ((dtype(reach) - dist) / np.where(dist > 0, dist, 1) * dtype(0.5)).astype(dtype)
    push = np.where(keep, push, 0).astype(dtype)
    out_x = np.zeros(len(px)); out_y = np.zeros(len(px))
    np.add.at(out_x, i, (dx * push).astype(np.float64)); np.add.at(out_x, j, -(dx * push).astype(np.float64))
    np.add.at(out_y, i, (dy * push).astype(np.float64)); np.add.at(out_y, j, -(dy * push).astype(np.float64))
    return out_x, out_y


def stats(name, ex, ey, rx, ry):
    err = np.concatenate([ex - rx, ey - ry]); ref = np.concatenate([rx, ry])
    rms_abs = float(np.sqrt(np.mean(err ** 2))); rms_ref = float(np.sqrt(np.mean(ref ** 2)))
    print(f"{name}: rmsAbs={rms_abs:.4g} rmsRef={rms_ref:.4g} rmsRel={rms_abs / rms_ref:.4g} "
          f"maxAbs={float(np.max(np.abs(err))):.4g}")


def main(path, reach):
    n, state, px, py, cx, cy = load(path)
    pairs = cKDTree(np.column_stack([px, py])).query_pairs(reach, output_type="ndarray")
    contacts = np.bincount(pairs.ravel(), minlength=n)
    extent = float(max(np.max(np.abs(px)), np.max(np.abs(py))))
    print(f"{path}: n={n} state={state} pairs={len(pairs)} k_c={int(contacts.max())} "
          f"extent={extent:.6g} ulp(extent)={np.spacing(np.float32(extent)):.4g}")
    ax, ay = deltas(px, py, pairs, reach, np.float64)
    scale = float(np.dot(np.concatenate([cx, cy]), np.concatenate([ax, ay]))
                  / np.dot(np.concatenate([ax, ay]), np.concatenate([ax, ay])))
    print(f"merge scale fitted: {scale:.17g}")
    stats("(a) f64", scale * ax, scale * ay, cx, cy)
    bx, by = deltas(px.astype(np.float32).astype(np.float64), py.astype(np.float32).astype(np.float64),
                    pairs, reach, np.float64)
    stats("(b) f32 input, f64 math", scale * bx, scale * by, cx, cy)
    fx, fy = deltas(px, py, pairs, reach, np.float32)
    stats("(c) f32 input, f32 math", scale * fx, scale * fy, cx, cy)
    rms_ref = float(np.sqrt(np.mean(np.concatenate([cx, cy]) ** 2)))
    guard = 1e-4 + int(contacts.max()) * 5 * U23 * (reach / 2) / rms_ref
    print(f"Amendment-1 guard rmsRel: {guard:.4g}")


if __name__ == "__main__":
    main(sys.argv[1], float(sys.argv[2]) if len(sys.argv) > 2 else 32.0)
