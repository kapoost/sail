#!/usr/bin/env python3
"""Build the page, the manual and the PDF into `_site/`.

Three outputs, one source of truth for each fact:

* the key list comes out of `crates/sail-app/src/help.rs`, which has a test in it
  that checks the list against every module that reads the keyboard;
* the polar and the test counts come out of `README.md`, which is generated from
  the simulation by `cargo run --example polar`;
* the version comes out of `Cargo.toml`.

Nothing a reader could check against the program is typed in by hand here. The
one time a key list was written from memory on a download page, three of the
keys were wrong, and nobody noticed until somebody tried to sail with it.

    python3 site/build.py            # page + manual
    python3 site/build.py --pdf      # and the PDF, if weasyprint is installed
"""

from __future__ import annotations

import html
import pathlib
import re
import shutil
import subprocess
import sys

HERE = pathlib.Path(__file__).resolve().parent
ROOT = HERE.parent
OUT = ROOT / "_site"

sys.path.insert(0, str(HERE))
import content  # noqa: E402
import keys as keylist  # noqa: E402

REPO = "https://github.com/kapoost/sail"


# ---- facts read out of the repository -------------------------------------


def version() -> str:
    """The workspace version, which is what a release is tagged with."""
    for crate in ("sail-app", "sail-core"):
        text = (ROOT / "crates" / crate / "Cargo.toml").read_text(encoding="utf-8")
        found = re.search(r'^version\s*=\s*"([^"]+)"', text, re.M)
        if found:
            return found.group(1)
    raise SystemExit("no version in any crate's Cargo.toml")


def readme() -> str:
    return (ROOT / "README.md").read_text(encoding="utf-8")


def polar() -> str:
    """The speed polar, pasted into the README from the simulation that made it."""
    section = readme().split("## Current polar", 1)
    if len(section) < 2:
        raise SystemExit("README.md: no `## Current polar` section to read the polar from")
    block = re.search(r"```\n(.*?)```", section[1], re.S)
    if not block:
        raise SystemExit("README.md: the polar section has no code block in it")
    return block.group(1).rstrip()


def model_rows() -> list[tuple[str, str]]:
    """The model table — what each part of the physics actually is."""
    section = readme().split("## Model, and what it approximates", 1)[1]
    rows = []
    for line in section.splitlines():
        if not line.startswith("|"):
            if rows:
                break
            continue
        cells = [c.strip() for c in line.strip("|").split("|")]
        if len(cells) != 2 or cells[0] in ("Part", "---") or set(cells[0]) <= set("-"):
            continue
        rows.append((cells[0], cells[1]))
    if len(rows) < 8:
        raise SystemExit(f"README.md: only {len(rows)} rows parsed out of the model table")
    return rows


def test_counts() -> tuple[str, str]:
    """How many assertions stand behind the physics, as the README states them."""
    found = re.search(r"(\d+) tests in the core .*? (\d+) across the workspace", readme(), re.S)
    if not found:
        raise SystemExit("README.md: cannot find the test counts")
    return found.group(1), found.group(2)


# ---- small helpers ---------------------------------------------------------


def md_inline(text: str) -> str:
    """The little bit of markdown the README's table cells use."""
    text = html.escape(text)
    text = re.sub(r"\*\*(.+?)\*\*", r"<b>\1</b>", text)
    text = re.sub(r"\*(.+?)\*", r"<em>\1</em>", text)
    text = re.sub(r"`(.+?)`", r"<code>\1</code>", text)
    return text


def key_rows(rows: list[tuple[str, str]]) -> str:
    """The help's own rows, including the ones whose key column is blank.

    A blank key is not a gap — it is a sentence that belongs to the key above it,
    and the help screen draws it that way on purpose.
    """
    out = []
    for key, what in rows:
        cont = not key.strip()
        what = re.sub(r"\*(.+?)\*", r"<em>\1</em>", html.escape(what))
        out.append(
            f'<div class="row{" cont" if cont else ""}">'
            f'<div class="key">{html.escape(key) if not cont else "&nbsp;"}</div>'
            f'<div class="what">{what}</div></div>'
        )
    return "\n".join(out)


def keys_html() -> str:
    keylist.check()
    groups = []
    for heading, rows in keylist.sections():
        groups.append(
            f'<div class="group"><p class="plate">{html.escape(heading)}</p>\n'
            f"{key_rows(rows)}</div>"
        )
    return '<div class="keys">\n' + "\n".join(groups) + "\n</div>"


def size_of(path: pathlib.Path) -> tuple[int, int]:
    """Width and height of a WebP, read out of its own header.

    Parsed here rather than imported, because the only thing this needs Pillow for is
    two integers, and a build that works with nothing but the standard library is a
    build that still works on a runner image nobody has updated.
    """
    b = path.read_bytes()
    if b[:4] != b"RIFF" or b[8:12] != b"WEBP":
        raise SystemExit(f"{path}: not a WebP")
    chunk = b[12:16]
    if chunk == b"VP8X":
        w = int.from_bytes(b[24:27], "little") + 1
        h = int.from_bytes(b[27:30], "little") + 1
        return w, h
    if chunk == b"VP8 ":
        return (int.from_bytes(b[26:28], "little") & 0x3FFF,
                int.from_bytes(b[28:30], "little") & 0x3FFF)
    if chunk == b"VP8L":
        bits = int.from_bytes(b[21:25], "little")
        return (bits & 0x3FFF) + 1, ((bits >> 14) & 0x3FFF) + 1
    raise SystemExit(f"{path}: unrecognised WebP chunk {chunk!r}")


def shots() -> list[tuple[str, str, str]]:
    """The pictures, in the order they tell the story. Missing ones are skipped."""
    captions = [
        (
            "brig.webp",
            "A brig wants a pilot",
            "She comes in from the open sea and will not come to you. Pilotage is leading, "
            "not boarding: catch her up, get out in front, and she follows your water from "
            "there — in a hull that draws more than yours and cannot turn like yours.",
        ),
        (
            "chart.webp",
            "The chart table",
            "Dividers, a plain ruler, a parallel rule, a protractor, a pencil, a "
            "stopwatch, a hand log and a rubber — and the deck log beside the paper, "
            "because a chart is not where a navigator writes things down. The coast is "
            "read off the same field the boat runs aground on, so it cannot lie about "
            "the depths.",
        ),
        (
            "lost.webp",
            "Somewhere at sea, with no idea where",
            "The names come off the marks and the chart stops following her; getting them "
            "back is the exercise. Seeing a thing and knowing which thing it is are "
            "different questions, and the second is the whole difficulty — three marks "
            "well spread apart are a shape that belongs in one place on the coast and "
            "nowhere else.",
        ),
        (
            "squall.webp",
            "A squall, announced",
            "Bearing, distance, and whether it is passing or coming for you — on the "
            "console before it is on the boat. The wind in it both builds and veers, and a "
            "squall that has been announced and ignored is the commonest way to be knocked "
            "down here.",
        ),
        (
            "gennaker.webp",
            "The wardrobe",
            "Genoa, code 0, gennaker, spinnaker. Asking for one is an order rather than a "
            "switch: the foredeck rolls the old sail away, bends the new one on and sets "
            "it, which is a job with stages and takes the time it takes.",
        ),
        (
            "race.webp",
            "Fifteen seconds to the gun",
            "You cannot stop a sailing boat and you cannot park her, so the start of a "
            "race is not a button but a manoeuvre: five minutes to sail away from the "
            "line, read the wind, pick the favoured end and arrive at full speed without "
            "being early. The panel here is saying you will not make it — 233 m to the "
            "line and 124 seconds of sailing to cover it — and that the line is square, "
            "so neither end is worth the detour.",
        ),
        (
            "drill.webp",
            "Taught, not announced",
            "The overboard drill coaches the figure of eight — the reach, tack, reach the "
            "RYA and US Sailing both teach — one instruction at a time, with a target "
            "heading and a mark in the water for each leg. When something has to be done "
            "in the next two seconds it is four words across the screen.",
        ),
        (
            "below.webp",
            "Under the surface, beside the lure",
            "This project argued against the view on the grounds that the sea is opaque "
            "and there would be nothing to see — which was an argument about the "
            "surface. Ten metres of visibility, green rather than blue because this is a "
            "shelf sea with a spring bloom in it.",
        ),
        (
            "rod.webp",
            "A rod over the stern",
            "Not a minigame beside the sailing. A fish on the line at six knots is a fish "
            "you are going to lose, so the first thing you do when something takes is get "
            "the way off her — with the mainsheet, the dump and the helm you already have.",
        ),
    ]
    return [(f, t, c) for f, t, c in captions if (HERE / "shots" / f).exists()]


# ---- the pages -------------------------------------------------------------


def head(title: str, description: str, css_extra: str = "") -> str:
    return f"""<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{html.escape(title)}</title>
<meta name="description" content="{html.escape(description)}">
<meta property="og:title" content="{html.escape(title)}">
<meta property="og:description" content="{html.escape(description)}">
<meta property="og:type" content="website">
<link rel="stylesheet" href="style.css">
<link rel="icon" href="favicon.svg" type="image/svg+xml">
{css_extra}
</head>
<body>"""


def help_shot() -> str:
    """The help screen itself, which is where this list actually lives."""
    f = HERE / "shots" / "keys.webp"
    if not f.exists():
        return ""
    w, h = size_of(f)
    return (
        f'<figure><img src="shots/keys.webp" alt="The help screen in the game" '
        f'loading="lazy" width="{w}" height="{h}">'
        "<figcaption><b>The same list, in the program</b>Behind a mark in the corner, "
        "which is the first thing anybody tries. The page above is generated from the "
        "source of this screen.</figcaption></figure>"
    )


def picture_pair() -> str:
    """The two drawn widths, same frame, side by side.

    Put in because the option is new and the only question worth asking about it is
    *what does it change* — which is answered with a picture rather than a sentence.
    The crop is rigging on purpose: on open water the two are indistinguishable, and a
    comparison shot of a thing that does not differ is worse than none.
    """
    f = HERE / "shots" / "widths.webp"
    if not f.exists():
        return ""
    w, h = size_of(f)
    return (
        f'<figure><img src="shots/widths.webp" '
        f'alt="The same rigging drawn at 720 pixels across and at 960" '
        f'loading="lazy" width="{w}" height="{h}">'
        "<figcaption><b>720 on the left, 960 on the right</b>The same shrouds, the same "
        "moment, enlarged equally. On open water the two are indistinguishable; it is "
        "rigging that shows the difference, which is also what the coarsest width used "
        "to lose.</figcaption></figure>"
    )


def reel_html() -> str:
    """The loop, if there is one. Silence rather than a broken image if not."""
    f = HERE / "shots" / "sailing.webp"
    if not f.exists():
        return ""
    w, h = size_of(f)
    return (
        f'<figure class="hero reel"><img src="shots/sailing.webp" '
        f'alt="The boat sailing in a squall, heeled over" width="{w}" height="{h}">'
        "<figcaption><b>Thirty frames of a squall</b>She heels, the sea runs under her, "
        "and the instruments answer — which is the half of this that a still photograph "
        "cannot say. Recorded by the game photographing itself, because screen capture "
        "wants a permission a build server does not have.</figcaption></figure>"
    )


def index(v: str) -> str:
    core, workspace = test_counts()
    pictures = shots()
    hero = ""
    gallery = ""
    if pictures:
        f, t, c = pictures[0]
        w, h = size_of(HERE / "shots" / f)
        hero = (
            f'<figure class="hero"><img src="shots/{f}" alt="{html.escape(t)}" '
            f'width="{w}" height="{h}">'
            f"<figcaption><b>{html.escape(t)}</b>{html.escape(c)}</figcaption></figure>"
        )
        gallery = "\n".join(
            f'<figure><img src="shots/{f}" alt="{html.escape(t)}" loading="lazy" '
            f'width="{size_of(HERE / "shots" / f)[0]}" '
            f'height="{size_of(HERE / "shots" / f)[1]}">'
            f"<figcaption><b>{html.escape(t)}</b>{html.escape(c)}</figcaption></figure>"
            for f, t, c in pictures[1:]
        )

    what = "\n".join(
        f"<h3>{html.escape(h)}</h3><p>{b.strip()}</p>" for h, b in content.WHAT_IT_IS
    )
    nav_points = "\n".join(
        f"<h3>{html.escape(h)}</h3><p>{b.strip()}</p>" for h, b in content.NAVIGATION_POINTS
    )
    model = "\n".join(
        f"<tr><td>{md_inline(part)}</td><td>{md_inline(how)}</td></tr>"
        for part, how in model_rows()
    )

    return f"""{head(
        "sail — a sailing simulator",
        "A sailing simulator in the spirit of a long-haul truck sim: engine-free yacht "
        "dynamics in Rust, validated against real boat numbers.",
    )}
<header class="mast">
  <div class="wrap">
    <h1 class="title">sail<span>.</span></h1>
    <p class="standfirst">{content.STANDFIRST}</p>
    <div class="buttons">
      <a class="button" href="#get-it">Get it</a>
      <a class="button quiet" href="manual.html">Read the manual</a>
      <a class="button quiet" href="{REPO}">Source</a>
    </div>
    <nav class="top">
      <a href="#pictures">Pictures</a>
      <a href="#the-model">The model</a>
      <a href="#navigation">Navigation</a>
      <a href="#sound">Sound</a>
      <a href="#together">Together</a>
      <a href="#keys">Keys</a>
      <a href="#polar">Polar</a>
    </nav>
    {hero}
  </div>
</header>

<section id="what-it-is">
  <div class="wrap">
    <div class="heading"><h2>What it is</h2></div>
    <div class="narrow">{what}</div>
    <div class="stats">
      <div class="stat"><b>{workspace}</b><span>tests across the workspace</span></div>
      <div class="stat"><b>{core}</b><span>in the dynamics alone</span></div>
      <div class="stat"><b>200&nbsp;Hz</b><span>fixed physics substeps</span></div>
      <div class="stat"><b>4 DOF</b><span>surge, sway, roll, yaw — with added mass</span></div>
    </div>
  </div>
</section>

<section id="pictures">
  <div class="wrap">
    <div class="heading"><h2>Pictures</h2></div>
    <p class="lede narrow">{content.WHY_NOT_BROWSER}</p>
    {reel_html()}
    <div class="gallery">{gallery}</div>
  </div>
</section>

<section id="the-model">
  <div class="wrap">
    <div class="heading"><h2>The model</h2></div>
    <p class="lede narrow">{content.MODEL_INTRO}</p>
    <div class="table-scroll">
      <table><thead><tr><th>Part</th><th>Treatment</th></tr></thead>
      <tbody>{model}</tbody></table>
    </div>
    <div class="caveat narrow">{content.NOT_IN_THE_BOX}</div>
  </div>
</section>

<section id="navigation">
  <div class="wrap">
    <div class="heading"><h2>Navigation</h2></div>
    <p class="lede narrow">{content.NAVIGATION}</p>
    <div class="narrow">{nav_points}</div>
  </div>
</section>

<section id="sound">
  <div class="wrap">
    <div class="heading"><h2>Sound</h2></div>
    <div class="narrow"><p>{content.SOUND}</p><p>{content.SOUND_BAND}</p></div>
  </div>
</section>

<section id="together">
  <div class="wrap">
    <div class="heading"><h2>Sailing together</h2></div>
    <div class="narrow"><p>{content.TOGETHER}</p></div>
    <pre>sail-server --seed 42</pre>
  </div>
</section>

<section id="get-it">
  <div class="wrap">
    <div class="heading"><h2>Get it</h2></div>
    <div class="two">
      <div>
        <h3>macOS</h3>
        <pre>curl -fsSL {REPO.replace("github.com", "raw.githubusercontent.com")}/main/dist/install.sh | sh</pre>
        <p>A line in a terminal rather than a download on purpose: macOS quarantines what a
        <em>browser</em> saves and not what <code>curl</code> fetches, and since macOS 15
        removed right-click-Open there is otherwise no straightforward way past
        <em>“Sail is damaged and can't be opened”</em> on an application nobody has paid
        Apple to vouch for.</p>
      </div>
      <div>
        <h3>Windows</h3>
        <pre>irm {REPO.replace("github.com", "raw.githubusercontent.com")}/main/dist/install.ps1 | iex</pre>
        <p>Same idea: <em>“Windows protected your PC”</em> comes from the mark of the web,
        which the program doing the downloading attaches — a browser does,
        <code>Invoke-WebRequest</code> does not. <code>Sail.exe</code> from the releases
        page works too, if you would rather click <b>More info</b> and <b>Run anyway</b>.</p>
      </div>
    </div>
    <h3>From source</h3>
    <pre>cargo run -p sail-app --release      # the game
cargo test -p sail-core              # the validation suite
cargo run -p sail-core --release --example polar   # the speed polar</pre>
    <div class="buttons">
      <a class="button" href="{REPO}/releases/latest">Releases</a>
      <a class="button quiet" href="manual.html">The manual</a>
      <a class="button quiet" href="manual.pdf">Manual as PDF</a>
    </div>
  </div>
</section>

<section id="keys">
  <div class="wrap">
    <div class="heading"><h2>The keys</h2></div>
    <p class="lede narrow">Generated from the program's own help screen — press
    <code>?</code> or <code>F1</code> in the game for the same list. It is not copied here
    by hand, because the one time a key list was written from memory on a download page,
    three of the keys were wrong.</p>
    {help_shot()}
    {keys_html()}
  </div>
</section>

<section id="polar">
  <div class="wrap">
    <div class="heading"><h2>The polar</h2></div>
    <p class="lede narrow">Boat speed in knots, by true wind angle and true wind speed —
    generated by the simulation rather than drawn. Best upwind VMG in twelve knots lands at
    40 degrees true: 5.6 knots through the water, 4.1 made good, 21 degrees of heel, 3.6 of
    leeway. Those are the numbers off a real boat.</p>
    <pre class="polar">{html.escape(polar())}</pre>
  </div>
</section>

<footer>
  <div class="wrap">
    <p>sail {html.escape(v)} — MIT or Apache-2.0. Built by one person with an agent, in
    the open: <a href="{REPO}">{REPO.replace("https://", "")}</a>.</p>
    <p>Type is <a href="https://fonts.google.com/specimen/VT323">VT323</a> under the SIL
    Open Font License.</p>
  </div>
</footer>
</body></html>"""


def manual(v: str, for_print: bool = False) -> str:
    toc = "\n".join(
        f'<li><a href="#{k}">{html.escape(t)}</a></li>' for k, t, _ in content.MANUAL
    ) + '<li><a href="#every-key">Every key</a></li>'

    body_parts = []
    for k, t, b in content.MANUAL:
        body_parts.append(f'<h2 id="{k}">{html.escape(t)}</h2>\n{b.strip()}')
        # The comparison belongs with the prose that describes the option, and the
        # print stylesheet has to carry it too — a manual that mentions a difference it
        # does not show is the thing this picture exists to prevent.
        if k == "before-you-start":
            body_parts.append(picture_pair())
    body = "\n".join(part for part in body_parts if part)

    extra = '<link rel="stylesheet" href="print.css">' if for_print else ""
    back = "" if for_print else '<nav class="top"><a href="index.html">&larr; sail</a></nav>'
    # On paper the reader is holding the thing these would link to.
    buttons = "" if for_print else (
        '<div class="buttons">'
        '<a class="button" href="manual.pdf">Download as PDF</a>'
        '<a class="button quiet" href="index.html#get-it">Get the game</a>'
        "</div>"
    )

    return f"""{head(f"sail — the manual", "How to sail her: the passage, the instruments, "
                     "the chart, and what to do when it goes wrong.", extra)}
<header class="mast">
  <div class="wrap">
    {back}
    <h1 class="title" style="font-size:clamp(3rem,9vw,5.5rem)">The manual<span>.</span></h1>
    <p class="standfirst">{content.MANUAL_STANDFIRST}</p>
    <p class="lede">sail {html.escape(v)}</p>
    {buttons}
    <ul class="toc">{toc}</ul>
  </div>
</header>

<section>
  <div class="wrap">
    <div class="manual-body">{body}</div>
  </div>
</section>

<section id="every-key">
  <div class="wrap">
    <div class="heading"><h2>Every key</h2></div>
    <p class="lede narrow">The same list the program shows on <code>?</code> or
    <code>F1</code>, generated from its source. A row with no key against it is a sentence
    belonging to the key above it.</p>
    {keys_html()}
  </div>
</section>

<footer>
  <div class="wrap"><p>sail {html.escape(v)} — MIT or Apache-2.0 —
  <a href="{REPO}">{REPO.replace("https://", "")}</a></p></div>
</footer>
</body></html>"""


# ---- the PDF ---------------------------------------------------------------

PRINT_CSS = """/* Paper wants the opposite of a screen: ink on white, and page breaks that do
   not cut a section off at the knees. The palette survives as accents only.

   The body text is whatever serif the building machine has — Iowan Old Style on the
   Mac this was written on, DejaVu Serif on the runner — so the PDF runs to eight pages
   here and nine there. The published file is the one CI builds, and that is the
   canonical one; a local build is a preview of the words rather than of the pagination.
   Pinning it would mean committing a text face to the repository, and the only font
   this project ships is the one the instruments are set in. VT323 *is* embedded, so
   the headings are the same everywhere. */
@page { size: A4; margin: 18mm 16mm 16mm;
        @bottom-right { content: counter(page) " / " counter(pages);
                        font-family: "VT323", monospace; font-size: 9pt; color: #8a8578; }
        @bottom-left { content: "sail — the manual";
                       font-family: "VT323", monospace; font-size: 9pt; color: #8a8578; } }
@page :first { @bottom-left { content: ""; } @bottom-right { content: ""; } }
body { background: #fff; color: #15181d; font-size: 10.5pt; line-height: 1.5; }
header.mast { background: none; padding: 0; }
section { border-top: none; padding: 0.6rem 0 0; }
.wrap { padding: 0; max-width: none; }
.title { color: #15181d; font-size: 40pt !important; line-height: 0.9;
         margin-bottom: 0.4rem; }
.title span { color: #9a7429; }
.standfirst, .lede, .what, td, footer, figcaption { color: #3d434c; }
.standfirst strong, .manual-body b { color: #15181d; }
h2 { color: #8a6a26; font-size: 16pt; margin-top: 1.4rem; page-break-after: avoid; }
h3 { color: #15181d; font-size: 12pt; page-break-after: avoid; }
.manual-body { max-width: none; }
.manual-body ul, .manual-body ol { color: #3d434c; }
a { color: #8a6a26; }
pre { background: #f4f1ea; border-color: #d8d2c4; border-left-color: #9a7429; color: #15181d; }
code { background: #f4f1ea; padding: 0 2px; }
.group { background: #faf8f3; border-color: #d8d2c4; break-inside: avoid; }
.plate { color: #8a6a26; border-bottom-color: #d8d2c4; }
.key { color: #15181d; }
/* One column on paper, after trying both of the nicer shapes.

   A CSS grid strands an empty half-page wherever it crosses a page break — the last
   sheet came out with a column of nothing beside the last group. CSS columns fixed
   that and then crashed the renderer outright: WeasyPrint falls over inside
   `columns_layout` with `AttributeError: 'NoneType' object has no attribute
   'children'`, which names nothing a person could act on and took a bisection to pin
   on the stylesheet rather than on the page.

   A list that runs straight down does neither, costs no extra pages — ten either way —
   and is the shape a reference wants anyway. */
.keys { display: block; }
.group { margin-bottom: 0.8rem; padding: 0.6rem 0.8rem 0.7rem; max-width: 34rem; }
.row { font-size: 8.5pt; grid-template-columns: 4.6rem 1fr; padding: 0.15rem 0; }
.caveat { background: #faf8f3; border-color: #d8d2c4; border-left-color: #a8382a;
          break-inside: avoid; }
.caveat b { color: #a8382a; }
.toc { font-size: 9.5pt; }
.toc a { color: #3d434c; text-decoration: none; }
footer { border-top: 1px solid #d8d2c4; margin-top: 1.5rem; font-size: 8.5pt; }
nav.top { display: none; }
p, li { orphans: 2; widows: 2; }
.stats, figure { break-inside: avoid; }
.heading::after { background: #d8d2c4; }
"""


def build_pdf() -> bool:
    """Render the manual to A4. Returns False if weasyprint is not here."""
    (OUT / "print.css").write_text(PRINT_CSS, encoding="utf-8")
    (OUT / "manual-print.html").write_text(manual(version(), for_print=True), encoding="utf-8")
    if not shutil.which("weasyprint"):
        print("  no weasyprint on PATH — skipping the PDF", file=sys.stderr)
        return False
    run = subprocess.run(
        ["weasyprint", str(OUT / "manual-print.html"), str(OUT / "manual.pdf")],
        capture_output=True,
        text=True,
    )
    if run.returncode != 0:
        print(run.stderr[-2000:], file=sys.stderr)
        raise SystemExit("weasyprint failed")
    (OUT / "manual-print.html").unlink()
    return True


def every_reference_resolves(expect_pdf: bool = True) -> None:
    """Every local file the pages point at is actually in the output.

    Written after publishing a page whose moving picture and help screenshot were
    both 404: they are referenced from code paths that `shots()` does not list, so
    nothing copied them, and nothing noticed. A missing image is the quietest kind of
    broken — the page still loads, still scrolls, still looks deliberate.
    """
    missing = []
    for page in ("index.html", "manual.html"):
        html_text = (OUT / page).read_text(encoding="utf-8")
        for ref in re.findall(r'(?:src|href)="([^"]+)"', html_text):
            if ref.startswith(("http://", "https://", "mailto:", "#", "data:")):
                continue
            target = ref.split("#", 1)[0]
            # A build without `--pdf` links to a PDF it did not make; that is a
            # preview, not a publish, and the workflow always passes `--pdf`.
            if target == "manual.pdf" and not expect_pdf:
                continue
            if target and not (OUT / target).exists():
                missing.append(f"{page} -> {target}")
    if missing:
        raise SystemExit("built page refers to files that are not there:\n  " + "\n  ".join(missing))


def main() -> None:
    v = version()
    if OUT.exists():
        shutil.rmtree(OUT)
    (OUT / "shots").mkdir(parents=True)

    (OUT / "index.html").write_text(index(v), encoding="utf-8")
    (OUT / "manual.html").write_text(manual(v), encoding="utf-8")
    shutil.copy(HERE / "style.css", OUT / "style.css")
    shutil.copy(HERE / "favicon.svg", OUT / "favicon.svg")
    shutil.copy(ROOT / "assets/VT323-Regular.ttf", OUT / "VT323-Regular.ttf")
    shutil.copy(ROOT / "assets/VT323-OFL.txt", OUT / "VT323-OFL.txt")

    pictures = shots()
    for f, _, _ in pictures:
        shutil.copy(HERE / "shots" / f, OUT / "shots" / f)
    # The two that are not in the gallery and so are not in that list: the loop under
    # the masthead and the photograph of the help screen. Copying only what `shots()`
    # names published a page referring to two files that were never put beside it.
    for extra in ("sailing.webp", "keys.webp", "widths.webp"):
        if (HERE / "shots" / extra).exists():
            shutil.copy(HERE / "shots" / extra, OUT / "shots" / extra)
    # GitHub Pages would otherwise hand the whole thing to Jekyll.
    (OUT / ".nojekyll").write_text("", encoding="utf-8")

    # Before the PDF as well as after: WeasyPrint reads these same pages, and when an
    # image is missing it does not say so — it fell over inside its column layout with
    # `AttributeError: 'NoneType' object has no attribute 'children'`, which names
    # nothing a person could act on.
    every_reference_resolves(expect_pdf=False)
    pdf = "--pdf" in sys.argv and build_pdf()
    every_reference_resolves(expect_pdf=pdf)
    print(f"built _site/ — sail {v}, {len(pictures)} pictures"
          f"{', manual.pdf' if pdf else ''}")


if __name__ == "__main__":
    main()
