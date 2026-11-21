"""Branded README cards and the animated demo, built from clean renders of
the real windows (docs/media/raw/, captured from the app with a transparent
background) plus the sprites. Same size, same background, nothing else.

Usage: python scripts/make-cards.py   (writes docs/media/card-*.png, hero.gif)
"""
import json
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).parent.parent
MEDIA = ROOT / "docs" / "media"
RAW = MEDIA / "raw"
data = json.loads((ROOT / "src/lib/sprites.json").read_text(encoding="utf8"))
FONTS = ROOT / "node_modules/@fontsource"

BG = (36, 25, 21)
FLOOR = (47, 33, 28)
PANEL = (47, 33, 28)
SUNK = (58, 42, 36)
LINE = (18, 12, 11)
INK = (246, 230, 216)
SOFT = (200, 168, 150)
ACCENT = (217, 119, 87)
SELECT = (59, 111, 209)

W, H = 1600, 1000          # every card, 2x
FLOOR_H = 72
RADIUS = 40


def font(name, size):
    files = {
        "pixel": FONTS / "pixelify-sans/files/pixelify-sans-latin-400-normal.woff",
        "pixel-bold": FONTS / "pixelify-sans/files/pixelify-sans-latin-600-normal.woff",
        "title": FONTS / "silkscreen/files/silkscreen-latin-400-normal.woff",
    }
    if name == "mono":
        for f in ["C:/Windows/Fonts/CascadiaMono.ttf", "/System/Library/Fonts/Menlo.ttc", "DejaVuSansMono.ttf"]:
            try:
                return ImageFont.truetype(f, size)
            except OSError:
                pass
        return ImageFont.load_default()
    return ImageFont.truetype(str(files[name]), size)


# --- sprites ------------------------------------------------------------------
def hexrgb(h):
    h = h.lstrip("#")
    return tuple(int(h[i:i + 2], 16) for i in (0, 2, 4))


def sprite(name, scale=8):
    f = data["frames"][name]
    rows = [list(f.get("rows", {}).get(str(y), r)) for y, r in enumerate(data["base"])]
    for x, y, ch in f.get("pixels", []):
        rows[y][x] = ch
    colors = {**data["common"], **data["palettes"]["rojiza"]}
    img = Image.new("RGBA", (16, 16), (0, 0, 0, 0))
    for y, row in enumerate(rows):
        for x, ch in enumerate(row):
            if ch in colors:
                img.putpixel((x, y), hexrgb(colors[ch]) + (255,))
    return img.resize((16 * scale, 16 * scale), Image.NEAREST)


def acorn_icon(scale):
    colors = {"C": data["common"]["C"], "A": data["common"]["A"], "R": "#A24E33", "L": "#E89A78"}
    rows = data["acorn"]
    img = Image.new("RGBA", (len(rows[0]), len(rows)), (0, 0, 0, 0))
    for y, row in enumerate(rows):
        for x, ch in enumerate(row):
            if ch in colors:
                img.putpixel((x, y), hexrgb(colors[ch]) + (255,))
    return img.resize((img.width * scale, img.height * scale), Image.NEAREST)


# --- canvas --------------------------------------------------------------------
def backdrop(w=W, h=H):
    img = Image.new("RGBA", (w, h), BG + (255,))
    d = ImageDraw.Draw(img)
    d.rectangle([0, h - FLOOR_H, w, h], fill=FLOOR + (255,))
    # a few acorns, quiet, in the corners
    for (x, y, s) in [(70, 70, 4), (w - 150, 110, 5), (w - 260, 60, 3), (110, h - FLOOR_H - 150, 3)]:
        a = acorn_icon(s)
        a.putalpha(a.getchannel("A").point(lambda v: v * 0.45))
        img.alpha_composite(a, (x, y))
    return img


def rounded(img, radius=RADIUS):
    mask = Image.new("L", img.size, 0)
    ImageDraw.Draw(mask).rounded_rectangle([0, 0, img.width - 1, img.height - 1], radius=radius, fill=255)
    out = img.copy()
    out.putalpha(mask)
    return out


def trim(img):
    return img.crop(img.getbbox())


def fit(img, max_w, max_h):
    s = min(1.0, max_w / img.width, max_h / img.height)
    return img if s == 1 else img.resize((round(img.width * s), round(img.height * s)), Image.LANCZOS)


def ground_y(h=H):
    return h - FLOOR_H


def card_window(raw, frame="base", side="right", top=90, scale_sq=8):
    """A window floating in the card with the squirrel on the floor beside it."""
    img = backdrop()
    win = fit(trim(Image.open(RAW / raw)), W - 520, ground_y() - top - 40)
    sq = sprite(frame, scale_sq)
    gap = 70
    total = win.width + gap + sq.width
    x0 = (W - total) // 2
    wx = x0 if side == "right" else x0 + sq.width + gap
    sx = x0 + win.width + gap if side == "right" else x0
    wy = top + (ground_y() - top - win.height) // 2
    img.alpha_composite(win, (wx, wy))
    img.alpha_composite(sq, (sx, ground_y() - sq.height))
    return img


# --- cards ---------------------------------------------------------------------
def card_menu():
    img = backdrop()
    menu = Image.open(RAW / "menu.png")  # rendered at 3x
    # the squirrel sits where the menu's centre button is (198, 396) x3
    sq = sprite("base", 12)
    menu.alpha_composite(sq, (594, 1188))
    menu = trim(menu)
    menu = fit(menu, W - 160, ground_y() - 80)
    img.alpha_composite(menu, ((W - menu.width) // 2, ground_y() - menu.height))
    return img


def card_answer():
    img = backdrop()
    bubble = trim(Image.open(RAW / "answer.png"))
    sq = sprite("talk", 8)
    gap = 18
    total_h = bubble.height + gap + sq.height
    scale = min(1.0, (ground_y() - 50) / total_h)
    if scale < 1:
        bubble = bubble.resize((round(bubble.width * scale), round(bubble.height * scale)), Image.LANCZOS)
    bx = (W - bubble.width) // 2
    by = ground_y() - sq.height - gap - bubble.height
    img.alpha_composite(bubble, (bx, by))
    # the beak points down at her: centre her under it (beak ~ half of the bubble)
    img.alpha_composite(sq, (bx + bubble.width // 2 - sq.width // 2, ground_y() - sq.height))
    return img


def editor(code_sel=None, width=1180):
    """A small code editor in the app's style; `code_sel` = selected chars."""
    lines = [
        ("// notas del proyecto web", SOFT),
        ("const doubled = [1, 4, 9, 16].map((x) => x * 2);", INK),
    ]
    mono, title = font("mono", 30), font("title", 22)
    line_h, pad, head = 54, 34, 58
    room = 120  # below the code: where the acorn and its actions show up
    h = head + pad + line_h * len(lines) + room
    img = Image.new("RGBA", (width + 12, h + 18), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    # pixel box: dark outline with notched corners, shadow below
    d.rectangle([6, 12, width + 6, h + 12], fill=(0, 0, 0, 90))
    d.rectangle([0, 0, width, h], fill=LINE + (255,))
    for cx, cy in [(0, 0), (width - 5, 0), (0, h - 5), (width - 5, h - 5)]:
        d.rectangle([cx, cy, cx + 5, cy + 5], fill=(0, 0, 0, 0))
    d.rectangle([6, 6, width - 6, h - 6], fill=PANEL + (255,))
    d.rectangle([6, 6, width - 6, head], fill=SUNK + (255,))
    d.text((30, 20), "NOTAS.JS", font=title, fill=SOFT)
    for i, c in enumerate([(217, 119, 87), (169, 122, 53), (106, 168, 79)]):
        d.rectangle([width - 130 + i * 34, 22, width - 112 + i * 34, 40], fill=c)
    y = head + pad
    sel_end = None
    for i, (text, color) in enumerate(lines):
        x = 34
        if i == 1 and code_sel:
            part = text[:code_sel]
            w = d.textlength(part, font=mono)
            d.rectangle([x - 4, y - 6, x + w + 4, y + line_h - 14], fill=SELECT + (255,))
            d.text((x, y), part, font=mono, fill=(255, 255, 255))
            d.text((x + w, y), text[code_sel:], font=mono, fill=color)
            sel_end = (int(x + w), y + line_h - 14)
        else:
            d.text((x, y), text, font=mono, fill=color)
        y += line_h
    return img, sel_end


def card_quick():
    img = backdrop()
    ed, end = editor(code_sel=48)
    ex, ey = (W - ed.width) // 2, 230
    img.alpha_composite(ed, (ex, ey))
    bar = Image.open(RAW / "bar.png")
    bar = bar.crop(bar.getbbox())
    bx = min(ex + end[0] - 120, W - bar.width - 60)
    img.alpha_composite(bar, (bx, ey + end[1] + 26))
    sq = sprite("alert", 8)
    img.alpha_composite(sq, (W - sq.width - 150, ground_y() - sq.height))
    return img


def cards():
    out = {
        "card-quick.png": card_quick(),
        "card-answer.png": card_answer(),
        "card-menu.png": card_menu(),
        "card-kura.png": card_window("kura.png", "chew1"),
        "card-pomodoro.png": card_window("pomo-run.png", "pomo3"),
        "card-setup.png": card_window("setup3.png", "success", side="left"),
        "card-looks.png": card_looks(),
    }
    for name, img in out.items():
        rounded(img).save(MEDIA / name, optimize=True)
    return list(out)


# --- GIFs ----------------------------------------------------------------------
def save_gif(frames, durs, path, radius=24, colors=96):
    """Frames (RGB, same size) to a GIF whose corners are rounded (transparent)."""
    w, h = frames[0].size
    mask = Image.new("L", (w, h), 0)
    ImageDraw.Draw(mask).rounded_rectangle([0, 0, w - 1, h - 1], radius=radius, fill=255)
    src = Image.new("RGB", (w, h * 2))
    src.paste(frames[0], (0, 0))
    src.paste(frames[-1], (0, h))
    pal = src.quantize(colors=colors, method=Image.MEDIANCUT)
    clear = 255  # a palette slot nothing else uses: the transparent corners
    hole = Image.new("P", (w, h), clear)
    out = []
    for f in frames:
        q = f.quantize(palette=pal, dither=Image.Dither.NONE)
        q.paste(hole, (0, 0), Image.eval(mask, lambda v: 255 - v))
        out.append(q)
    out[0].save(path, save_all=True, append_images=out[1:], duration=durs, loop=0, transparency=clear, disposal=2)


def card_looks():
    img = backdrop()
    pals = list(data["palettes"])
    accs = list(data["accessories"])
    d = 10
    cell = 16 * d + 40
    def row(items, y, make):
        x0 = (W - len(items) * cell + 40) // 2
        for i, it in enumerate(items):
            img.alpha_composite(make(it), (x0 + i * cell, y))
    def look(palette="rojiza", accessory=None):
        f = data["frames"]["base"]
        rows = [list(r) for r in data["base"]]
        colors = {**data["common"], **data["palettes"][palette]}
        layers = (["ezo"] if palette == "ezo" else []) + ([accessory] if accessory else [])
        for name in layers:
            layer = data["overlays"].get(name) or data["accessories"][name]
            colors.update(layer.get("colors", {}))
            for x, y, ch in layer["pixels"]:
                rows[y][x] = ch
        im = Image.new("RGBA", (16, 16), (0, 0, 0, 0))
        for y, r in enumerate(rows):
            for x, ch in enumerate(r):
                if ch in colors:
                    im.putpixel((x, y), hexrgb(colors[ch]) + (255,))
        return im.resize((16 * d, 16 * d), Image.NEAREST)
    gap = 110
    top = (ground_y() - (2 * 16 * d + gap)) // 2 + 20
    row(pals, top, lambda p: look(palette=p))
    row(accs, top + 16 * d + gap, lambda a: look(accessory=a))
    return img


def lively_card():
    seq = (
        [("base", 700), ("breathe", 700), ("blink", 140), ("base", 500)]
        + [("tailLeft", 140), ("base", 120), ("tailRight", 140), ("base", 120), ("tailLeft", 140), ("base", 500)]
        + [("dropFall", 90), ("dropLanded", 420), ("dropLook", 520), ("stoop", 380), ("dropLook", 120),
           ("success", 420), ("crouch", 120), ("base", 400), ("blink", 140)]
        + [("chew1", 130), ("chew2", 130), ("chew1", 130), ("chew2", 130), ("base", 500)]
        + [(f"pomo{i}", 400) for i in range(1, 6)] + [("rest1", 600), ("rest2", 600), ("rest1", 600), ("base", 600)]
    )
    frames, durs = [], []
    for name, ms in seq:
        img = backdrop()
        sq = sprite(name, 24)
        img.alpha_composite(sq, ((W - sq.width) // 2, ground_y() - sq.height))
        frames.append(img.convert("RGB").resize((W // 2, H // 2), Image.NEAREST))
        durs.append(ms)
    save_gif(frames, durs, MEDIA / "card-lively.gif", radius=20, colors=48)


# --- animated demo -------------------------------------------------------------
def cursor(scale=3):
    rows = [
        "X...........",
        "XX..........",
        "XWX.........",
        "XWWX........",
        "XWWWX.......",
        "XWWWWX......",
        "XWWWWWX.....",
        "XWWWWWWX....",
        "XWWWWWWWX...",
        "XWWWWWXXXX..",
        "XWWXWWX.....",
        "XWX.XWWX....",
        "XX..XWWX....",
        "X....XWWX...",
        ".....XXX....",
    ]
    img = Image.new("RGBA", (len(rows[0]), len(rows)), (0, 0, 0, 0))
    for y, row in enumerate(rows):
        for x, ch in enumerate(row):
            if ch == "X":
                img.putpixel((x, y), (18, 12, 11, 255))
            elif ch == "W":
                img.putpixel((x, y), (255, 255, 255, 255))
    return img.resize((img.width * scale, img.height * scale), Image.NEAREST)


def hero():
    CW, CH = 2000, 1150
    ed0, _ = editor(None, 1120)
    edx, edy = 110, 230
    raw_bubble = trim(Image.open(RAW / "answer.png"))
    bubble = fit(raw_bubble, 720, 800)
    sf = bubble.width / raw_bubble.width
    # bubble parts (measured on the render): header, streamed text, buttons
    head_end, text_end = int(72 * sf), int(360 * sf)
    acorn = trim(Image.open(RAW / "acorn.png"))
    bar = trim(Image.open(RAW / "bar.png"))
    ptr = cursor(3)
    # the bubble hangs over the squirrel: she stands right under its beak
    bx = CW - bubble.width - 70
    sq_x = bx + bubble.width // 2 - 64
    frames, durs = [], []

    # where each text line ends (measured on the render), so it streams by lines
    line_ends = [int(v * sf) for v in (115, 153, 192, 229, 266, 303, 341)] + [text_end]

    def streaming(k):
        """The bubble with only `k` (0..1) of its lines, as while it streams."""
        n = max(1, round(k * len(line_ends)))
        body = line_ends[n - 1] - head_end
        foot = bubble.crop((0, text_end, bubble.width, bubble.height))
        out = Image.new("RGBA", (bubble.width, head_end + body + foot.height), (0, 0, 0, 0))
        out.alpha_composite(bubble.crop((0, 0, bubble.width, head_end + body)), (0, 0))
        out.alpha_composite(foot, (0, head_end + body))
        return out

    def scene(sel=None, show_acorn=0.0, show_bar=False, ptr_at=None, sq="base", reveal=None):
        img = backdrop(CW, CH)
        ed, end = editor(sel, 1120)
        img.alpha_composite(ed, (edx, edy))
        if end and show_acorn:
            a = acorn if show_acorn >= 1 else acorn.resize((max(1, int(acorn.width * show_acorn)),) * 2, Image.NEAREST)
            ax, ay = edx + end[0] + 24, edy + end[1] + 24
            if show_bar:
                img.alpha_composite(bar, (min(ax, CW - bar.width - 40), ay))
            else:
                img.alpha_composite(a, (ax, ay))
        s = sprite(sq, 8)
        img.alpha_composite(s, (sq_x, CH - FLOOR_H - s.height))
        if reveal is not None:
            shown = bubble if reveal >= 1 else streaming(reveal)
            # anchored at the beak, like the real window: it grows upward
            img.alpha_composite(shown, (bx, CH - FLOOR_H - s.height - 14 - shown.height))
        if ptr_at:
            img.alpha_composite(ptr, ptr_at)
        return img

    def add(img, ms):
        frames.append(img.convert("RGB").resize((CW // 2, CH // 2), Image.LANCZOS))
        durs.append(ms)

    line_y = edy + 58 + 34 + 54 + 18  # middle of the code line
    start_x, end_x = edx + 34, edx + 34 + 860
    add(scene(ptr_at=(start_x - 6, line_y)), 700)
    add(scene(sq="blink", ptr_at=(start_x - 6, line_y)), 140)
    for i in range(1, 9):  # drag to select the line
        n = round(48 * i / 8)
        add(scene(sel=n, ptr_at=(start_x + int((end_x - start_x) * i / 8), line_y)), 70)
    add(scene(sel=48, ptr_at=(end_x, line_y)), 250)
    for k in (0.4, 0.7, 1.0):  # the acorn pops
        add(scene(sel=48, show_acorn=k, ptr_at=(end_x + 200, line_y + 120)), 70)
    add(scene(sel=48, show_acorn=1, ptr_at=(end_x + 200, line_y + 120)), 700)
    add(scene(sel=48, show_acorn=1, ptr_at=(end_x + 40, line_y + 90)), 300)
    add(scene(sel=48, show_acorn=1, show_bar=True, ptr_at=(end_x + 40, line_y + 90)), 500)
    add(scene(sel=48, show_acorn=1, show_bar=True, ptr_at=(end_x + 560, line_y + 90)), 600)
    for i, f in enumerate(["think1", "think2", "think3", "think2", "think1", "think2"]):  # she thinks
        add(scene(sel=48, sq=f), 230)
    steps = 8
    for i in range(1, steps + 1):  # the answer streams in, line by line
        add(scene(sel=48, sq="talk" if i % 2 else "base", reveal=i / steps), 170)
    add(scene(sel=48, sq="success", reveal=1), 700)
    add(scene(sel=48, sq="base", reveal=1), 3200)

    save_gif(frames, durs, MEDIA / "hero.gif", radius=22)


names = cards()
lively_card()
hero()
print("written:", ", ".join(names + ["card-lively.gif", "hero.gif"]))
