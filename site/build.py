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
# Where the browser build is, if one was handed to us. `--play dist/out/web` copies it
# in beside the pages and turns on the link to it; without it the link is not written at
# all, so a local preview does not advertise something it has not got.
PLAY = None

sys.path.insert(0, str(HERE))
import content  # noqa: E402
import content_pl  # noqa: E402
import i18n  # noqa: E402
import keys as keylist  # noqa: E402

# The prose, by language. Everything else — the key list, the polar, the model table —
# is generated from the program or the README and stays in English; `i18n` says why.
TEXT = {"en": content, "pl": content_pl}

REPO = "https://github.com/kapoost/sail"
RAW = REPO.replace("github.com", "raw.githubusercontent.com")


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
    # English on purpose — see `i18n`.
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


def shots(lang: str = "en") -> list[tuple[str, str, str]]:
    """The pictures, in the order they tell the story. Missing ones are skipped."""
    captions = TEXT[lang].SHOTS
    return [(f, t, c) for f, t, c in captions if (HERE / "shots" / f).exists()]


# ---- the pages -------------------------------------------------------------


def up(lang: str) -> str:
    """How to reach the site root from this language's directory."""
    return "../" if lang != "en" else ""


def alternates(lang: str, page: str) -> str:
    """`hreflang` links, so a search engine knows these are the same page twice."""
    out = []
    for other in i18n.LANGS:
        href = f"{up(lang)}{i18n.LANG_PATH[other]}{page}"
        out.append(f'<link rel="alternate" hreflang="{other}" href="{href}">')
    out.append(f'<link rel="alternate" hreflang="x-default" href="{up(lang)}{page}">')
    return "\n".join(out)


def switcher(lang: str, page: str) -> str:
    """The other language, as a link. Two languages need a link, not a menu."""
    others = [l for l in i18n.LANGS if l != lang]
    links = " ".join(
        f'<a href="{up(lang)}{i18n.LANG_PATH[o]}{page}" hreflang="{o}" '
        f'lang="{o}">{i18n.LANG_NAMES[o]}</a>'
        for o in others
    )
    return f'<div class="langs"><span>{i18n.UI[lang]["switch_label"]}</span>{links}</div>'


def head(title: str, description: str, lang: str = "en", page: str = "index.html",
         css_extra: str = "") -> str:
    return f"""<!doctype html>
<html lang="{lang}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{html.escape(title)}</title>
<meta name="description" content="{html.escape(description)}">
<meta property="og:title" content="{html.escape(title)}">
<meta property="og:description" content="{html.escape(description)}">
<meta property="og:type" content="website">
<link rel="stylesheet" href="{up(lang)}style.css">
<link rel="icon" href="{up(lang)}favicon.svg" type="image/svg+xml">
{alternates(lang, page)}
{css_extra}
</head>
<body>"""


def help_shot(lang: str = "en") -> str:
    """The help screen itself, which is where this list actually lives."""
    f = HERE / "shots" / "keys.webp"
    if not f.exists():
        return ""
    w, h = size_of(f)
    t = TEXT[lang]
    head_, body = t.HELP_SHOT_CAPTION
    return (
        f'<figure><img src="{up(lang)}shots/keys.webp" alt="{html.escape(t.HELP_SHOT_ALT)}" '
        f'loading="lazy" width="{w}" height="{h}">'
        f"<figcaption><b>{html.escape(head_)}</b>{html.escape(body)}</figcaption></figure>"
    )


def picture_pair(lang: str = "en") -> str:
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
    t = TEXT[lang]
    head_, body = t.WIDTHS_CAPTION
    return (
        f'<figure><img src="{up(lang)}shots/widths.webp" '
        f'alt="{html.escape(t.WIDTHS_ALT)}" '
        f'loading="lazy" width="{w}" height="{h}">'
        f"<figcaption><b>{html.escape(head_)}</b>{html.escape(body)}</figcaption></figure>"
    )


def reel_html(lang: str = "en") -> str:
    """The loop, if there is one. Silence rather than a broken image if not."""
    f = HERE / "shots" / "sailing.webp"
    if not f.exists():
        return ""
    w, h = size_of(f)
    t = TEXT[lang]
    head_, body = t.REEL_CAPTION
    return (
        f'<figure class="hero reel"><img src="{up(lang)}shots/sailing.webp" '
        f'alt="{html.escape(t.REEL_ALT)}" width="{w}" height="{h}">'
        f"<figcaption><b>{html.escape(head_)}</b>{html.escape(body)}</figcaption></figure>"
    )


def index(v: str, lang: str = "en") -> str:
    core, workspace = test_counts()
    t = TEXT[lang]
    u = i18n.UI[lang]
    root = up(lang)
    pictures = shots(lang)
    hero = ""
    gallery = ""
    if pictures:
        f, title, caption = pictures[0]
        w, h = size_of(HERE / "shots" / f)
        hero = (
            f'<figure class="hero"><img src="{root}shots/{f}" alt="{html.escape(title)}" '
            f'width="{w}" height="{h}">'
            f"<figcaption><b>{html.escape(title)}</b>{html.escape(caption)}</figcaption>"
            "</figure>"
        )
        gallery = "\n".join(
            f'<figure><img src="{root}shots/{f}" alt="{html.escape(title)}" '
            f'loading="lazy" width="{size_of(HERE / "shots" / f)[0]}" '
            f'height="{size_of(HERE / "shots" / f)[1]}">'
            f"<figcaption><b>{html.escape(title)}</b>{html.escape(caption)}</figcaption>"
            "</figure>"
            for f, title, caption in pictures[1:]
        )

    what = "\n".join(
        f"<h3>{html.escape(h)}</h3><p>{b.strip()}</p>" for h, b in t.WHAT_IT_IS
    )
    nav_points = "\n".join(
        f"<h3>{html.escape(h)}</h3><p>{b.strip()}</p>" for h, b in t.NAVIGATION_POINTS
    )
    model = "\n".join(
        f"<tr><td>{md_inline(part)}</td><td>{md_inline(how)}</td></tr>"
        for part, how in model_rows()
    )
    manual_href = "manual.html"
    pdf_href = "manual.pdf"
    # The playable build sits at the root of the site in both languages, because there
    # is only one of it: the game inside the tab speaks whatever the game speaks.
    play_href = "play/" if lang == "en" else "../play/"
    # **The first button on the page, when there is a build to put behind it.**
    # Somebody who has just arrived will try the thing that costs them nothing before
    # they will download a binary from a stranger, and a link to the demo three screens
    # down the page is a link nobody follows. The download keeps its place, quietly.
    top_play = (
        f'''<a class="button" href="{play_href}">{u["in_a_tab"]}</a>
      '''
        if PLAY
        else ""
    )
    get_it_quiet = " quiet" if PLAY else ""
    # And the paragraph under the pictures stops saying there is no demo, because there
    # is one: the same words would be a lie on a page with a button to it at the top.
    pictures_lede = t.PICTURES_AND_A_TAB if PLAY else t.WHY_NOT_BROWSER
    in_a_tab = (
        f"""    <h3>{u["in_a_tab"]}</h3>
    <p>{t.IN_A_TAB}</p>
    <div class="buttons">
      <a class="button" href="{play_href}">{u["in_a_tab"]}</a>
    </div>"""
        if PLAY
        else ""
    )

    return f"""{head(u["page_title"], u["page_desc"], lang, "index.html")}
<header class="mast">
  <div class="wrap">
    {switcher(lang, "index.html")}
    <h1 class="title">sail<span>.</span></h1>
    <p class="standfirst">{t.STANDFIRST}</p>
    <div class="buttons">
      {top_play}<a class="button{get_it_quiet}" href="#get-it">{u["get_it"]}</a>
      <a class="button quiet" href="{manual_href}">{u["read_manual"]}</a>
      <a class="button quiet" href="{REPO}">{u["source"]}</a>
    </div>
    <nav class="top">
      <a href="#pictures">{u["nav"]["pictures"]}</a>
      <a href="#the-model">{u["nav"]["model"]}</a>
      <a href="#navigation">{u["nav"]["navigation"]}</a>
      <a href="#sound">{u["nav"]["sound"]}</a>
      <a href="#together">{u["nav"]["together"]}</a>
      <a href="#keys">{u["nav"]["keys"]}</a>
      <a href="#polar">{u["nav"]["polar"]}</a>
    </nav>
    {hero}
  </div>
</header>

<section id="what-it-is">
  <div class="wrap">
    <div class="heading"><h2>{u["what_it_is"]}</h2></div>
    <div class="narrow">{what}</div>
    <div class="stats">
      <div class="stat"><b>{workspace}</b><span>{u["tests_workspace"]}</span></div>
      <div class="stat"><b>{core}</b><span>{u["tests_core"]}</span></div>
      <div class="stat"><b>200&nbsp;Hz</b><span>{u["substeps"]}</span></div>
      <div class="stat"><b>4 DOF</b><span>{u["dof"]}</span></div>
    </div>
  </div>
</section>

<section id="pictures">
  <div class="wrap">
    <div class="heading"><h2>{u["pictures"]}</h2></div>
    <p class="lede narrow">{pictures_lede}</p>
    {reel_html(lang)}
    <div class="gallery">{gallery}</div>
  </div>
</section>

<section id="the-model">
  <div class="wrap">
    <div class="heading"><h2>{u["the_model"]}</h2></div>
    <p class="lede narrow">{t.MODEL_INTRO}</p>
    <div class="table-scroll">
      <table><thead><tr><th>{u["part"]}</th><th>{u["treatment"]}</th></tr></thead>
      <tbody>{model}</tbody></table>
    </div>
    <div class="caveat narrow">{t.NOT_IN_THE_BOX}</div>
  </div>
</section>

<section id="navigation">
  <div class="wrap">
    <div class="heading"><h2>{u["the_navigation"]}</h2></div>
    <p class="lede narrow">{t.NAVIGATION}</p>
    <div class="narrow">{nav_points}</div>
  </div>
</section>

<section id="sound">
  <div class="wrap">
    <div class="heading"><h2>{u["the_sound"]}</h2></div>
    <div class="narrow"><p>{t.SOUND}</p><p>{t.SOUND_BAND}</p></div>
  </div>
</section>

<section id="together">
  <div class="wrap">
    <div class="heading"><h2>{u["sailing_together"]}</h2></div>
    <div class="narrow"><p>{t.TOGETHER}</p></div>
    <pre>sail-server --seed 42</pre>
  </div>
</section>

<section id="get-it">
  <div class="wrap">
    <div class="heading"><h2>{u["get_it_heading"]}</h2></div>
    <div class="two">
      <div>
        <h3>{u["macos"]}</h3>
        <pre>curl -fsSL {RAW}/main/dist/install.sh | sh</pre>
        <p>{t.INSTALL_MAC}</p>
      </div>
      <div>
        <h3>{u["windows"]}</h3>
        <pre>irm {RAW}/main/dist/install.ps1 | iex</pre>
        <p>{t.INSTALL_WINDOWS}</p>
      </div>
    </div>
{in_a_tab}
    <h3>{u["from_source"]}</h3>
    <pre>cargo run -p sail-app --release      # {t.SOURCE_GAME}
cargo test -p sail-core              # {t.SOURCE_TESTS}
cargo run -p sail-core --release --example polar   # {t.SOURCE_POLAR}</pre>
    <div class="buttons">
      <a class="button" href="{REPO}/releases/latest">{u["releases"]}</a>
      <a class="button quiet" href="{manual_href}">{u["the_manual"]}</a>
      <a class="button quiet" href="{pdf_href}">{u["manual_pdf"]}</a>
    </div>
  </div>
</section>

<section id="keys">
  <div class="wrap">
    <div class="heading"><h2>{u["the_keys"]}</h2></div>
    <p class="lede narrow">{u["keys_lede"]}</p>
    {help_shot(lang)}
    {keys_html()}
  </div>
</section>

<section id="polar">
  <div class="wrap">
    <div class="heading"><h2>{u["the_polar"]}</h2></div>
    <p class="lede narrow">{u["polar_lede"]}</p>
    <pre class="polar">{html.escape(polar())}</pre>
  </div>
</section>

<footer>
  <div class="wrap">
    <p>sail {html.escape(v)} — {u["footer_built"]}
    <a href="{REPO}">{REPO.replace("https://", "")}</a>.</p>
    <p>{u["footer_type"]}</p>
  </div>
</footer>
</body></html>"""


def manual(v: str, lang: str = "en", for_print: bool = False) -> str:
    t = TEXT[lang]
    u = i18n.UI[lang]
    toc = "\n".join(
        f'<li><a href="#{k}">{html.escape(title)}</a></li>' for k, title, _ in t.MANUAL
    ) + f'<li><a href="#every-key">{u["every_key"]}</a></li>'

    body_parts = []
    for k, title, b in t.MANUAL:
        body_parts.append(f'<h2 id="{k}">{html.escape(title)}</h2>\n{b.strip()}')
        # The comparison belongs with the prose that describes the option, and the
        # print stylesheet has to carry it too — a manual that mentions a difference it
        # does not show is the thing this picture exists to prevent.
        if k == "before-you-start":
            body_parts.append(picture_pair(lang))
    body = "\n".join(part for part in body_parts if part)

    extra = '<link rel="stylesheet" href="print.css">' if for_print else ""
    root = up(lang)
    home = "index.html"
    back = "" if for_print else f'<nav class="top"><a href="{home}">{u["back"]}</a></nav>'
    # On paper the reader is holding the thing these would link to, and the language
    # switcher has nowhere to go.
    buttons = "" if for_print else (
        '<div class="buttons">'
        f'<a class="button" href="manual.pdf">{u["download_pdf"]}</a>'
        f'<a class="button quiet" href="{home}#get-it">{u["get_the_game"]}</a>'
        "</div>"
    )
    switch = "" if for_print else switcher(lang, "manual.html")

    return f"""{head(u["manual_title"], u["manual_desc"], lang, "manual.html", extra)}
<header class="mast">
  <div class="wrap">
    {switch}
    {back}
    <h1 class="title" style="font-size:clamp(3rem,9vw,5.5rem)">{u["manual_heading"]}<span>.</span></h1>
    <p class="standfirst">{t.MANUAL_STANDFIRST}</p>
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
    <div class="heading"><h2>{u["every_key"]}</h2></div>
    <p class="lede narrow">{u["every_key_lede"]}</p>
    {keys_html()}
  </div>
</section>

<footer>
  <div class="wrap"><p>sail {html.escape(v)} — {u["footer_built"]}
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
        @bottom-left { content: "FOOTER_TEXT";
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
nav.top, .langs { display: none; }
p, li { orphans: 2; widows: 2; }
.stats, figure { break-inside: avoid; }
.heading::after { background: #d8d2c4; }
"""


def build_pdf(lang: str = "en") -> bool:
    """Render that language's manual to A4. False if weasyprint is not here."""
    where = OUT / i18n.LANG_PATH[lang]
    where.mkdir(parents=True, exist_ok=True)
    # The running foot is the one piece of the print stylesheet that is prose, so the
    # sheet is written per language rather than shared.
    foot = i18n.UI[lang]["running_foot"]
    (where / "print.css").write_text(
        PRINT_CSS.replace("FOOTER_TEXT", foot), encoding="utf-8"
    )
    src = where / "manual-print.html"
    src.write_text(manual(version(), lang, for_print=True), encoding="utf-8")
    if not shutil.which("weasyprint"):
        print("  no weasyprint on PATH — skipping the PDF", file=sys.stderr)
        src.unlink()
        return False
    run = subprocess.run(
        ["weasyprint", str(src), str(where / "manual.pdf")],
        capture_output=True,
        text=True,
    )
    if run.returncode != 0:
        print(run.stderr[-2000:], file=sys.stderr)
        raise SystemExit(f"weasyprint failed on the {lang} manual")
    src.unlink()
    return True


def every_reference_resolves(expect_pdf: bool = True) -> None:
    """Every local file the pages point at is actually in the output.

    Written after publishing a page whose moving picture and help screenshot were
    both 404: they are referenced from code paths that `shots()` does not list, so
    nothing copied them, and nothing noticed. A missing image is the quietest kind of
    broken — the page still loads, still scrolls, still looks deliberate.
    """
    missing = []
    pages = [
        f"{i18n.LANG_PATH[lang]}{name}"
        for lang in i18n.LANGS
        for name in ("index.html", "manual.html")
    ]
    for page in pages:
        html_text = (OUT / page).read_text(encoding="utf-8")
        for ref in re.findall(r'(?:src|href)="([^"]+)"', html_text):
            if ref.startswith(("http://", "https://", "mailto:", "#", "data:")):
                continue
            target = ref.split("#", 1)[0]
            # Resolve relative to the page, not the site root: the Polish pages sit a
            # directory down and reach their assets with "../".
            import posixpath

            target = posixpath.normpath(
                posixpath.join(posixpath.dirname(page), target)
            )
            # A build without `--pdf` links to a PDF it did not make; that is a
            # preview, not a publish, and the workflow always passes `--pdf`.
            if target.endswith("manual.pdf") and not expect_pdf:
                continue
            if target and not (OUT / target).exists():
                missing.append(f"{page} -> {target}")
    if missing:
        raise SystemExit("built page refers to files that are not there:\n  " + "\n  ".join(missing))


def main() -> None:
    global PLAY
    if "--play" in sys.argv:
        PLAY = pathlib.Path(sys.argv[sys.argv.index("--play") + 1])
        if not (PLAY / "sail-app_bg.wasm").is_file():
            raise SystemExit(f"--play {PLAY}: no sail-app_bg.wasm in there")
    v = version()
    if OUT.exists():
        shutil.rmtree(OUT)
    (OUT / "shots").mkdir(parents=True)

    for lang in i18n.LANGS:
        where = OUT / i18n.LANG_PATH[lang]
        where.mkdir(parents=True, exist_ok=True)
        (where / "index.html").write_text(index(v, lang), encoding="utf-8")
        (where / "manual.html").write_text(manual(v, lang), encoding="utf-8")
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
    if PLAY:
        shutil.copytree(PLAY, OUT / "play")
        wasm = (OUT / "play/sail-app_bg.wasm").stat().st_size
        print(f"  play/ — {wasm / 1e6:.0f} MB of wasm and the assets beside it")

    # GitHub Pages would otherwise hand the whole thing to Jekyll.
    (OUT / ".nojekyll").write_text("", encoding="utf-8")

    # Before the PDF as well as after: WeasyPrint reads these same pages, and when an
    # image is missing it does not say so — it fell over inside its column layout with
    # `AttributeError: 'NoneType' object has no attribute 'children'`, which names
    # nothing a person could act on.
    every_reference_resolves(expect_pdf=False)
    pdf = False
    if "--pdf" in sys.argv:
        pdf = all(build_pdf(lang) for lang in i18n.LANGS)
    every_reference_resolves(expect_pdf=pdf)
    langs = "+".join(i18n.LANGS)
    print(f"built _site/ — sail {v}, {langs}, {len(pictures)} pictures"
          f"{', manual.pdf' if pdf else ''}")


if __name__ == "__main__":
    main()
