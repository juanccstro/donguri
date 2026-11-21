"""Render every squirrel frame (plus variants) to a PNG contact sheet.

Usage: python scripts/preview-sprites.py out.png [palette] [accessory]
Mirrors the logic in src/lib/sprite.ts so sprites can be reviewed without the app.
"""
import json, sys
from pathlib import Path
from PIL import Image, ImageDraw

data = json.loads((Path(__file__).parent.parent / "src/lib/sprites.json").read_text(encoding="utf8"))
out = sys.argv[1]
palette_name = sys.argv[2] if len(sys.argv) > 2 else "rojiza"
accessory = sys.argv[3] if len(sys.argv) > 3 else None
N = data["size"]


def build(frame, extra=()):
    f = data["frames"][frame]
    rows = [list(f.get("rows", {}).get(str(y), r)) for y, r in enumerate(data["base"])]
    for x, y, ch in f.get("pixels", []):
        rows[y][x] = ch
    colors = {**data["common"], **data["palettes"][palette_name]}
    for name in extra:
        layer = data["overlays"].get(name) or data["accessories"][name]
        colors.update(layer.get("colors", {}))
        for x, y, ch in layer["pixels"]:
            rows[y][x] = ch
    if "shift" in f:
        s = f["shift"]
        for y in range(s["from"], s["to"] + 1):
            r = rows[y]
            rows[y] = (["."] * s["dx"] + r[: N - s["dx"]]) if s["dx"] > 0 else (r[-s["dx"]:] + ["."] * -s["dx"])
    for r in rows:
        assert len(r) == N, (frame, "".join(r))
    return rows, colors


extra = [accessory] if accessory else []
if palette_name == "ezo":
    extra = ["ezo", *extra]
names = list(data["frames"])
sc, pad, cols = 10, 14, 6
cell = N * sc + pad
im = Image.new("RGB", (cols * cell + pad, ((len(names) + cols - 1) // cols) * (cell + 12) + pad), "#FAF7F2")
d = ImageDraw.Draw(im)
for i, name in enumerate(names):
    rows, colors = build(name, extra)
    ox, oy = pad + (i % cols) * cell, pad + (i // cols) * (cell + 12)
    for y, r in enumerate(rows):
        for x, ch in enumerate(r):
            if ch in colors:
                d.rectangle([ox + x * sc, oy + y * sc, ox + x * sc + sc - 1, oy + y * sc + sc - 1], fill=colors[ch])
    d.text((ox, oy + N * sc + 1), name, fill="#555")
im.save(out)
print("ok", len(names), "frames")
