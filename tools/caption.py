"""One caption, drawn into a strip with the game's own font.

Separate from `demo.sh` because a caption is a sentence, and a sentence has colons,
commas and apostrophes in it — every one of which is punctuation inside an ffmpeg
filter description. Drawing it here and overlaying the result is one less layer of
escaping than `drawtext`, and it uses VT323, which is the typeface the instruments are
lettered in.
"""

import sys

from PIL import Image, ImageDraw, ImageFont

out, text, font_path = sys.argv[1], sys.argv[2], sys.argv[3]
font = ImageFont.truetype(font_path, 40)
PAD = 18
box = ImageDraw.Draw(Image.new("RGBA", (1, 1))).textbbox((0, 0), text, font=font)
size = (box[2] - box[0] + PAD * 2, box[3] - box[1] + PAD * 2)
strip = Image.new("RGBA", size, (16, 20, 24, 170))
ImageDraw.Draw(strip).text(
    (PAD - box[0], PAD - box[1]), text, font=font, fill=(240, 244, 248, 255)
)
strip.save(out)
