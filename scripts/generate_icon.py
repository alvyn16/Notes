"""Regenerate the Notes vector, PNG and Windows icon (requires Pillow)."""
from pathlib import Path
from math import cos, sin, radians
from PIL import Image, ImageDraw

ASSETS = Path(__file__).resolve().parents[1] / "assets"
SCALE = 32
WHITE = "#f5f5f5"
POLYGONS = [
    [(-23, 0), (-14, -6), (-14, 6)],
    [(-11, -6), (13, -6), (13, 6), (-11, 6)],
    [(16, -6), (20, -6), (23, -3), (23, 3), (20, 6), (16, 6)],
]

svg = ['<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64">',
       '  <rect width="64" height="64" fill="#000000"/>',
       f'  <g transform="translate(32 32) rotate(-45)" fill="{WHITE}">']
image = Image.new("RGBA", (64 * SCALE, 64 * SCALE), "black")
draw = ImageDraw.Draw(image)
angle = radians(-45)
for polygon in POLYGONS:
    points = " ".join(f"{x},{y}" for x, y in polygon)
    svg.append(f'    <polygon points="{points}"/>')
    transformed = [((32 + x * cos(angle) - y * sin(angle)) * SCALE,
                    (32 + x * sin(angle) + y * cos(angle)) * SCALE)
                   for x, y in polygon]
    draw.polygon(transformed, fill=WHITE)
svg.extend(['  </g>', '</svg>', ''])
(ASSETS / "icon.svg").write_text("\n".join(svg), encoding="utf-8")
image = image.resize((512, 512), Image.Resampling.LANCZOS)
image.save(ASSETS / "notes.png")
image.save(ASSETS / "notes.ico", sizes=[(s, s) for s in (16, 20, 24, 32, 40, 48, 64, 128, 256)])
print("Generated assets/icon.svg, assets/notes.png and assets/notes.ico")
