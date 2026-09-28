#!/usr/bin/env python3
"""Builds the Vaultime logo files.

The wordmark is converted to outlines from the bundled Fraunces font, so the
files look the same everywhere, with or without the font installed.

    pip install fonttools brotli uharfbuzz
    python scripts/build-brand.py

Needs `npm ci` in apps/desktop first for the font file.

vaultime-avatar.png is vaultime-avatar.svg rendered at 1024 px with resvg, for
GitHub and other sites that round the corners of a square avatar themselves.

The app icons come from vaultime-icon-source.svg. From apps/desktop, run
`npx tauri icon ../../assets/vaultime-icon-source.svg -o <temp folder>` and copy
32x32.png, 128x128.png, 128x128@2x.png, icon.png, icon.ico and icon.icns into
src-tauri/icons. The favicon of docs/site is tile_svg(64).

The tray and the taskbar show vaultime-icon-signed-in.svg while the PC is
signed in to cloud backup, like the logo in the app. Render it at 256 px, the
size Tauri takes from icon.ico, to src-tauri/icons/signed-in.png.
"""

import io
from pathlib import Path

import uharfbuzz as hb
from fontTools.pens.boundsPen import BoundsPen
from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.pens.transformPen import TransformPen
from fontTools.ttLib import TTFont
from fontTools.varLib.instancer import instantiateVariableFont

ROOT = Path(__file__).resolve().parent.parent
FONT = ROOT / "apps/desktop/node_modules/@fontsource-variable/fraunces/files/fraunces-latin-full-normal.woff2"
ASSETS = ROOT / "assets"

TILE = "#1A1230"
LIGHT = "#F1EBFA"
VIOLET = "#9D7CFF"
VIOLET_ON_LIGHT = "#7A55F0"

# The mark on a 64 unit grid: a rounded vault frame and clock hands at five
# past eleven, which form a V, pivoting in violet at the center of the dial.
# The tick at six o'clock keeps it reading as a dial, not a checkbox.
FRAME = (7, 7, 50, 50, 15)
FRAME_WIDTH = 3.5
HANDS = "M25.5 21.3L32 32.5L41 17"
TICK = "M32 44.5v3.5"
PIVOT = (32, 32.5, 3.4)


def mark(ink, pivot, frame_width=FRAME_WIDTH, hand_width=5.5):
    x, y, w, h, r = FRAME
    cx, cy, pr = PIVOT
    return (
        f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{r}" fill="none" stroke="{ink}" stroke-width="{frame_width}"/>'
        f'<path d="{HANDS}" fill="none" stroke="{ink}" stroke-width="{hand_width}" stroke-linecap="round" stroke-linejoin="round"/>'
        f'<path d="{TICK}" fill="none" stroke="{ink}" stroke-width="{frame_width}" stroke-linecap="round"/>'
        f'<circle cx="{cx}" cy="{cy}" r="{pr}" fill="{pivot}"/>'
    )


def framed(size, margin, frame_width=FRAME_WIDTH):
    """Scale and offset that put the outer edge of the frame `margin` of the size from each edge."""
    x, _, w, _, _ = FRAME
    scale = size * (1 - 2 * margin) / (w + frame_width)
    offset = size * margin - (x - frame_width / 2) * scale
    return scale, offset


def tile_svg(size, margin=0.06, rounded=True, ink=LIGHT, pivot=VIOLET):
    """The mark on a square tile. Rounded tile corners follow the curve of the
    frame, so the margin stays even all the way around."""
    scale, offset = framed(size, margin)
    radius = (FRAME[4] + FRAME_WIDTH / 2) * scale + size * margin if rounded else 0
    return (
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{size}" height="{size}" viewBox="0 0 {size} {size}">'
        f'<rect width="{size}" height="{size}" rx="{radius:.1f}" fill="{TILE}"/>'
        f'<g transform="translate({offset:.1f} {offset:.1f}) scale({scale:.4f})">{mark(ink, pivot)}</g>'
        "</svg>\n"
    )


def wordmark_outline(text="Vaultime", tracking=-0.02):
    variable = TTFont(FONT)
    font = instantiateVariableFont(variable, {"wght": 600, "opsz": 72, "SOFT": 50, "WONK": 1})
    font.flavor = None
    data = io.BytesIO()
    font.save(data)

    hb_font = hb.Font(hb.Face(hb.Blob(data.getvalue())))
    buf = hb.Buffer()
    buf.add_str(text)
    buf.guess_segment_properties()
    hb.shape(hb_font, buf, {"kern": True, "liga": True})

    upm = font["head"].unitsPerEm
    glyphs = font.getGlyphSet()
    order = font.getGlyphOrder()
    path_pen = SVGPathPen(glyphs)
    bounds_pen = BoundsPen(glyphs)
    x = 0
    for info, pos in zip(buf.glyph_infos, buf.glyph_positions):
        glyph = glyphs[order[info.codepoint]]
        transform = (1, 0, 0, -1, x + pos.x_offset, -pos.y_offset)
        glyph.draw(TransformPen(path_pen, transform))
        glyph.draw(TransformPen(bounds_pen, transform))
        x += pos.x_advance + tracking * upm
    return path_pen.getCommands(), bounds_pen.bounds


def lockup_svg(ink, pivot, frame_width=4, hand_width=6):
    path, (x0, y0, x1, y1) = wordmark_outline()
    height = y1 - y0
    # The frame is as tall as the wordmark, then a gap.
    gap = height * 0.34
    scale, offset = framed(height, 0, frame_width)
    text_x = height + gap - x0
    width = height + gap + (x1 - x0)
    return (
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width:.0f} {height:.0f}" width="{width / 8:.0f}" height="{height / 8:.0f}">'
        f'<g transform="translate({offset:.1f} {offset:.1f}) scale({scale:.4f})">{mark(ink, pivot, frame_width, hand_width)}</g>'
        f'<path transform="translate({text_x:.1f} {-y0:.1f})" fill="{ink}" d="{path}"/>'
        "</svg>\n"
    )


def main():
    ASSETS.mkdir(exist_ok=True)
    (ASSETS / "vaultime-icon-source.svg").write_text(tile_svg(1024), encoding="utf8")
    (ASSETS / "vaultime-avatar.svg").write_text(tile_svg(1024, rounded=False), encoding="utf8")
    (ASSETS / "vaultime-icon-signed-in.svg").write_text(tile_svg(1024, ink=VIOLET, pivot=LIGHT), encoding="utf8")
    (ASSETS / "vaultime-lockup-light.svg").write_text(lockup_svg(LIGHT, VIOLET), encoding="utf8")
    (ASSETS / "vaultime-lockup-dark.svg").write_text(lockup_svg(TILE, VIOLET_ON_LIGHT), encoding="utf8")
    print("wrote", ", ".join(sorted(p.name for p in ASSETS.glob("vaultime-*.svg"))))


if __name__ == "__main__":
    main()
