"""Builds the README banner and feature icons straight from the sprites and
the app's icons (the cards and GIFs come from make-cards.py).

Usage: python scripts/make-media.py   (writes into docs/media/)
"""
import json
from pathlib import Path
from PIL import Image, ImageDraw

ROOT = Path(__file__).parent.parent
OUT = ROOT / "docs" / "media"
OUT.mkdir(parents=True, exist_ok=True)
data = json.loads((ROOT / "src/lib/sprites.json").read_text(encoding="utf8"))
N = data["size"]

BG = (36, 25, 21)        # --bg (dark)
PANEL = (47, 33, 28)     # --panel (dark)
INK = (246, 230, 216)    # --ink (dark)
SOFT = (200, 168, 150)   # --ink-soft (dark)
ACCENT = (217, 119, 87)  # --terracotta


def hexrgb(h):
    h = h.lstrip("#")
    return tuple(int(h[i:i + 2], 16) for i in (0, 2, 4))


def frame(name, palette="rojiza", accessory=None):
    """One sprite frame as a 16x16 RGBA image (same rules as sprite.ts)."""
    f = data["frames"][name]
    rows = [list(f.get("rows", {}).get(str(y), r)) for y, r in enumerate(data["base"])]
    for x, y, ch in f.get("pixels", []):
        rows[y][x] = ch
    colors = {**data["common"], **data["palettes"][palette]}
    layers = (["ezo"] if palette == "ezo" else []) + ([accessory] if accessory else [])
    for layer_name in layers:
        layer = data["overlays"].get(layer_name) or data["accessories"][layer_name]
        colors.update(layer.get("colors", {}))
        for x, y, ch in layer["pixels"]:
            rows[y][x] = ch
    img = Image.new("RGBA", (N, N), (0, 0, 0, 0))
    for y, row in enumerate(rows):
        for x, ch in enumerate(row):
            if ch in colors:
                img.putpixel((x, y), hexrgb(colors[ch]) + (255,))
    return img


def big(img, scale):
    return img.resize((img.width * scale, img.height * scale), Image.NEAREST)


# --- a tiny 5x7 pixel font for titles (only the letters we need) ------------
FONT = {
    "D": ["####.", "#...#", "#...#", "#...#", "#...#", "#...#", "####."],
    "O": [".###.", "#...#", "#...#", "#...#", "#...#", "#...#", ".###."],
    "N": ["#...#", "##..#", "#.#.#", "#..##", "#...#", "#...#", "#...#"],
    "G": [".###.", "#...#", "#....", "#.###", "#...#", "#...#", ".###."],
    "U": ["#...#", "#...#", "#...#", "#...#", "#...#", "#...#", ".###."],
    "R": ["####.", "#...#", "#...#", "####.", "#.#..", "#..#.", "#...#"],
    "I": ["###", ".#.", ".#.", ".#.", ".#.", ".#.", "###"],
}


def text(draw, s, x, y, px, color):
    for ch in s:
        glyph = FONT[ch]
        for gy, line in enumerate(glyph):
            for gx, c in enumerate(line):
                if c == "#":
                    draw.rectangle([x + gx * px, y + gy * px, x + (gx + 1) * px - 1, y + (gy + 1) * px - 1], fill=color)
        x += (len(glyph[0]) + 1) * px
    return x


def acorn(scale):
    colors = {**data["common"]}
    img = Image.new("RGBA", (len(data["acorn"][0]), len(data["acorn"])), (0, 0, 0, 0))
    for y, row in enumerate(data["acorn"]):
        for x, ch in enumerate(row):
            c = {"C": colors["C"], "A": colors["A"], "R": "#A24E33", "L": "#E89A78"}.get(ch)
            if c:
                img.putpixel((x, y), hexrgb(c) + (255,))
    return big(img, scale)


# --- banner -------------------------------------------------------------------
def banner():
    W, H = 1280, 400
    img = Image.new("RGB", (W, H), BG)
    d = ImageDraw.Draw(img)
    # pixel "ground" strip and scattered acorns
    d.rectangle([0, H - 24, W, H], fill=PANEL)
    for i, (x, y) in enumerate([(70, 60), (1180, 80), (980, 300), (440, 320), (640, 40)]):
        a = acorn(4 if i % 2 else 3)
        img.paste(a, (x, y), a)
    sq = big(frame("base"), 16)
    img.paste(sq, (150, H - 24 - sq.height), sq)
    end = text(d, "DONGURI", 480, 120, 14, INK)
    d.rectangle([480, 240, end - 14, 250], fill=ACCENT)
    # rounded corners, like every other image in the README
    mask = Image.new("L", img.size, 0)
    ImageDraw.Draw(mask).rounded_rectangle([0, 0, W - 1, H - 1], radius=28, fill=255)
    img = img.convert("RGBA")
    img.putalpha(mask)
    img.save(OUT / "banner.png")


# --- animated squirrel (what she does in lively mode) -----------------------
def lively_gif():
    seq = (
        [("base", 600), ("breathe", 600), ("base", 600), ("blink", 140), ("base", 500)]
        + [("tailLeft", 140), ("base", 120), ("tailRight", 140), ("base", 120), ("tailLeft", 140), ("base", 400)]
        + [("dropFall", 90), ("dropLanded", 420), ("dropLook", 520), ("stoop", 380), ("dropLook", 120),
           ("success", 420), ("crouch", 120), ("base", 300), ("blink", 140)]
        + [("chew1", 130), ("chew2", 130), ("chew1", 130), ("chew2", 130), ("base", 700)]
    )
    frames, durations = [], []
    for name, ms in seq:
        canvas = Image.new("RGBA", (220, 200), BG + (255,))
        s = big(frame(name), 12)
        canvas.paste(s, ((220 - s.width) // 2, 200 - 8 - s.height), s)
        frames.append(canvas.convert("P", palette=Image.ADAPTIVE, colors=32))
        durations.append(ms)
    frames[0].save(OUT / "squirrel.gif", save_all=True, append_images=frames[1:], duration=durations, loop=0, disposal=2)


# --- Pomodoro: the acorn fills with green, then she naps --------------------
def pomodoro_gif():
    seq = [("base", 500)] + [(f"pomo{i}", 450) for i in range(1, 6)] + [("success", 500)]
    seq += [("rest1", 600), ("rest2", 600)] * 3
    frames, durations = [], []
    for name, ms in seq:
        canvas = Image.new("RGBA", (220, 200), BG + (255,))
        s = big(frame(name), 12)
        canvas.paste(s, ((220 - s.width) // 2, 200 - 8 - s.height), s)
        frames.append(canvas.convert("P", palette=Image.ADAPTIVE, colors=32))
        durations.append(ms)
    frames[0].save(OUT / "pomodoro.gif", save_all=True, append_images=frames[1:], duration=durations, loop=0, disposal=2)


# --- looks: colours and accessories -----------------------------------------
def looks():
    pals = list(data["palettes"].keys())
    accs = list(data["accessories"].keys())
    cell, pad = 96, 12
    cols = max(len(pals), len(accs))
    W = pad + cols * (cell + pad)
    H = pad + 2 * (cell + pad)
    img = Image.new("RGB", (W, H), BG)
    for i, p in enumerate(pals):
        s = big(frame("base", palette=p), 6)
        img.paste(s, (pad + i * (cell + pad), pad), s)
    for i, a in enumerate(accs):
        s = big(frame("base", accessory=a), 6)
        img.paste(s, (pad + i * (cell + pad), pad * 2 + cell), s)
    img.save(OUT / "looks.png")


banner()
print("written:", ", ".join(sorted(p.name for p in OUT.iterdir())))

# --- README feature icons, from the app's own 9x9 icons ------------------------
def icons():
    import re
    src = (ROOT / "src/lib/Icon.svelte").read_text(encoding="utf8")
    grids = {m.group(1): re.findall(r'"([.#o]+)"', m.group(2)) for m in re.finditer(r"(\w+): \[(.*?)\]", src, re.S)}
    ink, accent = (217, 119, 87), (169, 122, 53)
    for name in ["acorn", "nest", "ask", "plug", "clock", "lock"]:
        rows = grids[name]
        img = Image.new("RGBA", (len(rows[0]), len(rows)), (0, 0, 0, 0))
        for y, row in enumerate(rows):
            for x, ch in enumerate(row):
                if ch == "#":
                    img.putpixel((x, y), ink + (255,))
                elif ch == "o":
                    img.putpixel((x, y), accent + (255,))
        big(img, 4).save(OUT / f"icon-{name}.png")


icons()
