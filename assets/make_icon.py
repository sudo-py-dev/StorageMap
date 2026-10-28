#!/usr/bin/env python3
"""Renders the StorageMap icon (a stylised treemap) using only the stdlib.

Outputs, next to this script:
  icon/storagemap-{16..512}.png   PNG sizes for Linux (hicolor) and the window
  storagemap.ico                  multi-size Windows icon (PNG-compressed entries)
  storagemap.svg                  scalable version for the .deb

Run: python3 assets/make_icon.py
"""
import os
import struct
import zlib

HERE = os.path.dirname(os.path.abspath(__file__))
SUPERSAMPLE = 4
PNG_SIZES = [16, 24, 32, 48, 64, 128, 256, 512]
ICO_SIZES = [16, 24, 32, 48, 64, 128, 256]

# Unit-square layout: (x, y, w, h, colour). A squarified-looking treemap.
BACKGROUND_TOP = (37, 99, 235)
BACKGROUND_BOTTOM = (30, 64, 175)
TILES = [
    (0.14, 0.14, 0.42, 0.72, (251, 191, 36)),   # large amber tile
    (0.60, 0.14, 0.26, 0.34, (52, 211, 153)),   # green
    (0.60, 0.52, 0.26, 0.16, (244, 114, 182)),  # pink
    (0.60, 0.72, 0.12, 0.14, (125, 211, 252)),  # sky
    (0.76, 0.72, 0.10, 0.14, (226, 232, 240)),  # light
]
CORNER = 0.22  # background corner radius, as a fraction of the size
TILE_CORNER = 0.035
GAP = 0.0  # tiles already include gaps


def inside_rounded(px, py, x, y, w, h, r):
    """Whether point (px, py) is inside the rounded rectangle."""
    if px < x or py < y or px > x + w or py > y + h:
        return False
    cx = min(max(px, x + r), x + w - r)
    cy = min(max(py, y + r), y + h - r)
    return (px - cx) ** 2 + (py - cy) ** 2 <= r * r


def shade(color, t):
    """Lightens towards the top-left of a tile for a soft cushion look."""
    k = 1.0 + 0.18 * (0.5 - t)
    return tuple(max(0, min(255, int(c * k))) for c in color)


def render(size):
    big = size * SUPERSAMPLE
    acc = [[0.0, 0.0, 0.0, 0.0] for _ in range(size * size)]
    for by in range(big):
        v = (by + 0.5) / big
        bg = tuple(int(a + (b - a) * v) for a, b in zip(BACKGROUND_TOP, BACKGROUND_BOTTOM))
        for bx in range(big):
            u = (bx + 0.5) / big
            if not inside_rounded(u, v, 0, 0, 1, 1, CORNER):
                continue
            color = bg
            for (x, y, w, h, c) in TILES:
                if inside_rounded(u, v, x, y, w, h, TILE_CORNER):
                    color = shade(c, ((u - x) / w + (v - y) / h) / 2)
                    break
            cell = acc[(by // SUPERSAMPLE) * size + bx // SUPERSAMPLE]
            cell[0] += color[0]
            cell[1] += color[1]
            cell[2] += color[2]
            cell[3] += 255
    samples = SUPERSAMPLE * SUPERSAMPLE
    out = bytearray()
    for r, g, b, a in acc:
        cover = a / 255
        if cover == 0:
            out += b"\0\0\0\0"
        else:
            out += bytes((int(r / cover), int(g / cover), int(b / cover), int(a / samples)))
    return bytes(out)


def png(size, rgba):
    rows = b"".join(b"\0" + rgba[y * size * 4:(y + 1) * size * 4] for y in range(size))

    def chunk(tag, body):
        c = tag + body
        return struct.pack(">I", len(body)) + c + struct.pack(">I", zlib.crc32(c) & 0xFFFFFFFF)

    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(rows, 9)) + chunk(b"IEND", b""))


def ico(images):
    """ICO container with PNG-compressed entries (Vista+)."""
    header = struct.pack("<HHH", 0, 1, len(images))
    offset = 6 + 16 * len(images)
    entries, blobs = b"", b""
    for size, data in images:
        dim = 0 if size >= 256 else size
        entries += struct.pack("<BBBBHHII", dim, dim, 0, 0, 1, 32, len(data), offset)
        blobs += data
        offset += len(data)
    return header + entries + blobs


def svg():
    def rect(x, y, w, h, rx, fill):
        return f'<rect x="{x * 512:.1f}" y="{y * 512:.1f}" width="{w * 512:.1f}" height="{h * 512:.1f}" rx="{rx * 512:.1f}" fill="{fill}"/>'
    hexc = lambda c: "#%02x%02x%02x" % c
    tiles = "".join(rect(x, y, w, h, TILE_CORNER, hexc(c)) for (x, y, w, h, c) in TILES)
    return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512">'
            f'<defs><linearGradient id="g" x1="0" y1="0" x2="0" y2="1">'
            f'<stop offset="0" stop-color="{hexc(BACKGROUND_TOP)}"/><stop offset="1" stop-color="{hexc(BACKGROUND_BOTTOM)}"/>'
            f'</linearGradient></defs>{rect(0, 0, 1, 1, CORNER, "url(#g)")}{tiles}</svg>\n')


def main():
    os.makedirs(os.path.join(HERE, "icon"), exist_ok=True)
    pngs = {}
    for size in PNG_SIZES:
        pngs[size] = png(size, render(size))
        with open(os.path.join(HERE, "icon", f"storagemap-{size}.png"), "wb") as f:
            f.write(pngs[size])
    with open(os.path.join(HERE, "storagemap.ico"), "wb") as f:
        f.write(ico([(s, pngs[s]) for s in ICO_SIZES]))
    with open(os.path.join(HERE, "storagemap.svg"), "w") as f:
        f.write(svg())
    print("icons written to", HERE)


if __name__ == "__main__":
    main()
