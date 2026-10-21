#!/usr/bin/env python3
"""The shop's capsules, composed from a picture the game took of itself.

    dist/capsules.py dist/store/capsule.png      # the whole set

**A capsule is a poster, not a screenshot.** It is looked at for a third of a second in a
list of forty, at a size where a boat is nine millimetres long, and the only two things
that can survive that are a silhouette and a word. So this does what a poster does: crops
hard around the boat, drops the exposure and lifts the contrast so the hull reads black
against lit water, and sets the name in the game's own face with the game's own accent —
the same VT323 and the same amber the console and the website are built from, because a
shop page that looks like a different product is a shop page that was made by somebody
else.

Seven sizes, and they are Valve's: a capsule at the wrong size is rejected on upload, so
the list below is the specification rather than a preference. Each one gets its own crop
rather than a scale of the same rectangle — a 462 by 174 strip and a 748 by 896 panel
cannot hold the same composition, and squeezing one into the other is how a store page
comes to look like a thumbnail farm.

The art direction here is deliberately minimal and the reason is honest: this is the
first pass, made to look at. If it reads weakly beside real capsules on a store page, the
answer is an illustrator, and this script's job is then to be thrown away rather than
argued for.
"""

import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageEnhance, ImageFont

HERE = Path(__file__).resolve().parent.parent
FONT = HERE / "assets" / "VT323-Regular.ttf"
OUT = HERE / "dist" / "store" / "capsules"

# Valve's sizes, with the name each file has to be uploaded under.
SIZES = {
    "small_capsule": (462, 174),
    "header_capsule": (920, 430),
    "main_capsule": (1232, 706),
    "vertical_capsule": (748, 896),
    "library_capsule": (600, 900),
    "library_header": (920, 430),
    "library_hero": (3840, 1240),
    "page_background": (1438, 810),
}

# Where the boat is in the source frame, as a fraction. Every crop is taken around this
# point rather than around the middle: the middle of a sailing photograph is water — and
# it sits above her hull rather than on it, because what has to survive at nine
# millimetres is the *rig*, and a crop centred on the boat cuts the masthead off.
FOCUS = (0.44, 0.52)

# And a margin taken off the source before anything else. The game's own furniture lives
# at the edges of the frame — a help button in one corner, a watch in another — and a
# capsule with half a button in it is a capsule somebody made by accident.
EDGE = 0.055

AMBER = (230, 198, 120)
INK = (228, 232, 238)


def crop_to(image: Image.Image, size: tuple[int, int]) -> Image.Image:
    """The largest rectangle of `image` with the aspect of `size`, taken around FOCUS."""
    want = size[0] / size[1]
    w, h = image.size
    if w / h > want:
        box_w, box_h = int(h * want), h
    else:
        box_w, box_h = w, int(w / want)
    cx, cy = int(w * FOCUS[0]), int(h * FOCUS[1])
    left = max(0, min(w - box_w, cx - box_w // 2))
    top = max(0, min(h - box_h, cy - box_h // 2))
    return image.crop((left, top, left + box_w, top + box_h)).resize(size, Image.LANCZOS)


def graded(image: Image.Image) -> Image.Image:
    """Down a stop and up in contrast, so the rig reads as a silhouette."""
    image = ImageEnhance.Brightness(image).enhance(0.86)
    return ImageEnhance.Contrast(image).enhance(1.18)


def wordmark(canvas: Image.Image, size: tuple[int, int], tall: bool) -> None:
    """The name, and the one piece of colour on the page.

    Top left on everything wide, because that is where an eye lands and where the shop's
    own furniture is not; centred near the top on the two tall panels, which have room
    for nothing else. The full stop is amber and the word is not — one accent, the same
    one the console uses for the thing that matters on a dial.
    """
    w, h = size
    draw = ImageDraw.Draw(canvas, "RGBA")
    # A soft wash under the type: white letters on lit water are unreadable, and a
    # rectangle behind them looks like a sticker.
    wash = Image.new("RGBA", size, (0, 0, 0, 0))
    gradient = ImageDraw.Draw(wash)
    band = int(h * (0.52 if tall else 0.46))
    for y in range(band):
        shade = int(165 * (1 - y / band) ** 1.5)
        gradient.line([(0, y), (w, y)], fill=(6, 10, 16, shade))
    canvas.alpha_composite(wash)

    point = int(h * (0.20 if tall else 0.26))
    font = ImageFont.truetype(str(FONT), point)
    text = "sail"
    span = draw.textbbox((0, 0), text, font=font)
    tw = span[2] - span[0]
    x = (w - tw) // 2 if tall else int(w * 0.055)
    y = int(h * (0.08 if tall else 0.10))
    draw.text((x, y), text, font=font, fill=INK)
    dot = int(point * 0.17)
    draw.rectangle(
        [x + tw + int(point * 0.10), y + int(point * 0.62), x + tw + int(point * 0.10) + dot, y + int(point * 0.62) + dot],
        fill=AMBER,
    )
    # And a rule under it on the wide shapes, which is the website's own device.
    if not tall and w > 500:
        rule = y + int(point * 0.98)
        draw.line([(x, rule), (x + tw + int(point * 0.3), rule)], fill=(*AMBER, 140), width=max(1, h // 300))


def main() -> int:
    if len(sys.argv) < 2:
        print(__doc__.strip().splitlines()[2])
        return 2
    source = Path(sys.argv[1])
    if not source.exists():
        print(f"no picture at {source} — take one with dist/store.sh")
        return 1
    if not FONT.exists():
        print(f"no font at {FONT}")
        return 1
    OUT.mkdir(parents=True, exist_ok=True)
    raw = Image.open(source).convert("RGB")
    w, h = raw.size
    inset = raw.crop(
        (int(w * EDGE), int(h * EDGE), int(w * (1 - EDGE)), int(h * (1 - EDGE)))
    )
    base = graded(inset)
    for name, size in SIZES.items():
        canvas = crop_to(base, size).convert("RGBA")
        wordmark(canvas, size, tall=size[1] > size[0])
        path = OUT / f"{name}.png"
        canvas.convert("RGB").save(path)
        print(f"{name:17} {size[0]}x{size[1]}")
    print(f"{OUT} — and the logo over the hero is still a job for somebody with a pen")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
