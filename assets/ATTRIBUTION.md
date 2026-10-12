# Third-party assets

Everything else in this project is generated in code — the sky, the water textures,
the sails, the sound. This directory is the one exception, and it exists because a
hull is the one thing worth having a real measurement of rather than a guess.

## `venus.glb`

**"Venus", a Shetland fourareen** — photogrammetric scan by the Scottish Maritime
Museum, Irvine.

* Licence: **CC0 1.0 Universal** (public domain dedication). No attribution is
  required; this file records it because the work deserves it.
* Source: <https://zenodo.org/records/10218992>
  (also on Sketchfab: <https://sketchfab.com/3d-models/venus-a-shetland-fourareen-ce4d6915e1d041459e08f2d8da521e86>)
* 50k triangles, two 8K textures, 9.6 MB. Scanned at true scale: 5.40 m overall,
  1.57 m beam, which matches the museum's recorded eighteen feet.

Built in 1898 on Foula, the most remote inhabited island of the Shetlands, from
pieces of driftwood, by a crofter using techniques handed down generation to
generation. Bought in 1964 by Paul Earling Johnstone, who decked her, gave her a
tiny cabin and a Bermudan yawl rig, and sailed her single-handed across the
Atlantic in 1965.

**The game uses the hull and nothing else.** Her rig, her size and her sailing
characteristics are not modelled: the boat you sail is the `archipelago_packet`
config in `sail-core`, which is a designed vessel rather than this one. Taking the
fourareen's own numbers would mean modelling an eighteen-foot open boat whose
stability comes from crew weight rather than ballast — a different physics problem,
and not the one this game is about.

## `VT323-Regular.ttf`

**VT323** by Peter Hull — a revival of the character set of the DEC VT320 terminal.

* Licence: **SIL Open Font License 1.1**, text in `VT323-OFL.txt` alongside it.
* Source: <https://github.com/google/fonts/tree/main/ofl/vt323>

Used for every word on the console and the chart. A boat's instrument panel *is* a
terminal, so a terminal face is not a costume on it — and in the pixelated picture it
does the job no setting could: the lettering is drawn at the full resolution of the
screen, so it stays legible, while its letterforms are built on a pixel grid, so it
still belongs to the same decade as the sea behind it.

Chosen over the two other obvious candidates after looking at all three. Press Start 2P
is eight pixels wide per glyph and would burst every box on the console. Silkscreen is
the more aggressively square of the two that fit, and remains the alternative if this
one ever reads as too soft.
