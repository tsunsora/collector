"""Draws the app icon source: a purple tile with two tick bars, the top
one part-filled in white. Run, then `npx tauri icon src-tauri/app-icon.png` to
regenerate src-tauri/icons. The tile is the colour, not near-black, so the
icon stands out in a file listing and the tray in both dark and light mode.
"""
from pathlib import Path

from PIL import Image, ImageDraw

SIZE = 1024
SCALE = 4  # supersample for smooth corners
TILE = "#9146ff"
TRACK = "#6a2fc9"
ACCENT = "#ffffff"

s = SIZE * SCALE
img = Image.new("RGBA", (s, s), (0, 0, 0, 0))
d = ImageDraw.Draw(img)


def box(x0, y0, x1, y1, radius, fill):
    d.rounded_rectangle([x0 * SCALE, y0 * SCALE, x1 * SCALE, y1 * SCALE], radius * SCALE, fill=fill)


box(64, 64, 960, 960, 200, TILE)
for y, filled in ((336, 0.72), (568, 0.34)):
    box(216, y, 808, y + 120, 60, TRACK)
    box(216, y, 216 + (808 - 216) * filled, y + 120, 60, ACCENT)

out = Path(__file__).resolve().parent.parent / "src-tauri" / "app-icon.png"
img.resize((SIZE, SIZE), Image.LANCZOS).save(out)
print(out)
