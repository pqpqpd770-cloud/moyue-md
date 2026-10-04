# -*- coding: utf-8 -*-
"""Generate the 墨阅 app icon: multi-size .ico + raw RGBA blob for the window icon.

Everything is drawn from scratch (rounded tile, gradient, glyph, chevron), then
supersampled down so the small sizes stay crisp.
"""

import io
import os
import struct

from PIL import Image, ImageDraw, ImageFont

HERE = os.path.dirname(os.path.abspath(__file__))
ASSETS = os.path.normpath(os.path.join(HERE, "..", "assets"))
os.makedirs(ASSETS, exist_ok=True)

# Palette: the app's 朱砂 seal — a red stamp with the mark in paper white.
# Kept in step with --seal / --seal-ink in web/styles.css.
TOP = (196, 73, 58)      # #c4493a
BOTTOM = (181, 55, 42)   # #b5372a
GLYPH = (251, 247, 242)  # #fbf7f2

SS = 8  # supersample factor for the master render


def lerp(a, b, t):
    return tuple(int(round(a[i] + (b[i] - a[i]) * t)) for i in range(3))


def diagonal_gradient(size, c1, c2):
    """Simple (x+y) diagonal gradient built from drawn lines."""
    img = Image.new("RGB", (size, size))
    draw = ImageDraw.Draw(img)
    n = 2 * size
    for i in range(n):
        t = i / (n - 1)
        colour = lerp(c1, c2, t)
        draw.line([(i, 0), (0, i)], fill=colour)
    return img


def rounded_mask(size, radius):
    mask = Image.new("L", (size, size), 0)
    ImageDraw.Draw(mask).rounded_rectangle([0, 0, size - 1, size - 1], radius=radius, fill=255)
    return mask


def gloss(size):
    """Soft top-left highlight, built as a radial falloff mask."""
    g = Image.new("L", (64, 64), 0)
    px = g.load()
    cx, cy = 0.18, 0.12
    for y in range(64):
        for x in range(64):
            nx, ny = x / 63.0, y / 63.0
            d = ((nx - cx) ** 2 + (ny - cy) ** 2) ** 0.5
            v = max(0.0, 1.0 - d / 0.95)
            px[x, y] = int(255 * (v ** 1.8) * 0.12)
    return g.resize((size, size), Image.LANCZOS)


def draw_chevron(draw, size, cx, cy, half_w, height, width, colour, caps=True):
    x1, y1 = cx - half_w, cy - height * 0.5
    x2, y2 = cx, cy + height * 0.5
    x3, y3 = cx + half_w, cy - height * 0.5
    draw.line([(x1, y1), (x2, y2), (x3, y3)], fill=colour, width=int(width), joint="curve")
    if caps:
        r = width / 2.0
        for (px, py) in ((x1, y1), (x2, y2), (x3, y3)):
            draw.ellipse([px - r, py - r, px + r, py + r], fill=colour)


def stroke(draw, points, width, colour):
    """Polyline with rounded caps/joins - the same geometry the in-app SVG logo uses."""
    draw.line(points, fill=colour, width=int(round(width)), joint="curve")
    r = width / 2.0
    for (px, py) in (points[0], points[-1]):
        draw.ellipse([px - r, py - r, px + r, py + r], fill=colour)


def draw_mark(draw, s, top, stem, mid, left, right, w):
    """Stroked "M": down-up-down-up, exactly like the SVG logo."""
    cx = s * 0.5
    stroke(
        draw,
        [
            (s * left, s * stem),
            (s * left, s * top),
            (cx, s * mid),
            (s * right, s * top),
            (s * right, s * stem),
        ],
        w,
        GLYPH,
    )


def draw_icon(size, detail=True):
    """Render one icon size; returns an RGBA image."""
    s = size * SS
    radius = int(s * 0.15)

    base = diagonal_gradient(s, TOP, BOTTOM).convert("RGBA")
    white = Image.new("RGBA", (s, s), (255, 255, 255, 255))
    white.putalpha(gloss(s))
    base = Image.alpha_composite(base, white)

    draw = ImageDraw.Draw(base)

    # Glyph block: stroked "M" over a chevron, optically centred.
    if detail:
        draw_mark(draw, s, top=0.200, stem=0.520, mid=0.552, left=0.272, right=0.728, w=s * 0.099)
        draw_chevron(
            draw,
            s,
            cx=s * 0.5,
            cy=s * 0.742,
            half_w=s * 0.160,
            height=s * 0.098,
            width=s * 0.099,
            colour=GLYPH,
        )
    else:
        # Small sizes: a single bold "M" survives the downscale far better.
        draw_mark(draw, s, top=0.285, stem=0.585, mid=0.640, left=0.235, right=0.765, w=s * 0.140)

    base.putalpha(Image.composite(base.getchannel("A"), Image.new("L", (s, s), 0), rounded_mask(s, s * 0.15)))
    return base.resize((size, size), Image.LANCZOS)


def write_ico(path, images):
    entries = []
    offset = 6 + 16 * len(images)
    for size, im in images:
        buf = io.BytesIO()
        im.save(buf, format="PNG", optimize=True)
        data = buf.getvalue()
        entries.append((size, data, offset))
        offset += len(data)
    with open(path, "wb") as fh:
        fh.write(struct.pack("<HHH", 0, 1, len(images)))
        for size, data, off in entries:
            dim = 0 if size >= 256 else size
            fh.write(struct.pack("<BBBBHHII", dim, dim, 0, 0, 1, 32, len(data), off))
        for _, data, _ in entries:
            fh.write(data)


def main():
    big = [16, 24, 32, 48, 64, 128, 256]
    images = [(n, draw_icon(n, detail=n >= 48)) for n in big]
    write_ico(os.path.join(ASSETS, "icon.ico"), images)

    # Raw RGBA for tao's window icon (128px is plenty for the title bar / taskbar).
    win = draw_icon(128, detail=True)
    with open(os.path.join(ASSETS, "icon_window.rgba"), "wb") as fh:
        fh.write(win.tobytes())

    # Preview sheet: every size on a checker background, zoomed 4x.
    sheet = Image.new("RGBA", (sum(big) * 4 + 40, 256 * 4 + 40), (245, 246, 250, 255))
    x = 20
    for n, im in images:
        sheet.alpha_composite(im.resize((n * 4, n * 4), Image.NEAREST), (x, 20 + (256 * 4 - n * 4) // 2))
        x += n * 4 + 8
    sheet.save(os.path.join(ASSETS, "icon_preview.png"))
    print("wrote icon.ico, icon_window.rgba, icon_preview.png")


if __name__ == "__main__":
    main()
