#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
# SPDX-License-Identifier: GPL-3.0-or-later
"""Builds the website into dist-site: the landing page, its styles and art,
the fonts the app ships, the documentation, the screenshots and HTML versions
of the legal pages and the changelog.

    python scripts/build-site.py

Needs `npm ci` in apps/desktop first for the font files. The fonts are served
with the site, so visitors make no requests to font services.

The documentation lives in docs/site/docs, one Markdown file per page. Its
sidebar, page order and groups come from DOCS_NAV below, so a new page is a
new file plus one line there. The guides in docs/site/guides work the same
way with GUIDES_NAV.

Every page gets its own search title, description, canonical address and
Open Graph tags with assets/social-preview.png, which
scripts/social-preview.sh draws. The landing page also describes the app
for search engines, and sitemap.xml and robots.txt list what they may show.
"""

import html
import json
import re
import shutil
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SITE = ROOT / "docs/site"
DOCS = SITE / "docs"
GUIDES = SITE / "guides"
SCREENSHOTS = ROOT / "assets/screenshots"
ART = SITE / "art"
OUT = ROOT / "dist-site"
FONTS = ROOT / "apps/desktop/node_modules/@fontsource-variable"
REPO_URL = "https://github.com/vaultime/vaultime"
SITE_URL = "https://vaultime.codfishcloud.de"
SOCIAL_PREVIEW = ROOT / "assets/social-preview.png"
TAURI_CONFIG = ROOT / "apps/desktop/src-tauri/tauri.conf.json"

FONT_FILES = {
    "mona-sans-normal.woff2": FONTS / "mona-sans/files/mona-sans-latin-standard-normal.woff2",
    "mona-sans-italic.woff2": FONTS / "mona-sans/files/mona-sans-latin-standard-italic.woff2",
    "newsreader-normal.woff2": FONTS / "newsreader/files/newsreader-latin-standard-normal.woff2",
    "newsreader-italic.woff2": FONTS / "newsreader/files/newsreader-latin-standard-italic.woff2",
    "geist.woff2": FONTS / "geist/files/geist-latin-wght-normal.woff2",
    "geist-mono.woff2": FONTS / "geist-mono/files/geist-mono-latin-wght-normal.woff2",
    # The Open Font License asks for its text next to every copy of the fonts.
    "mona-sans-LICENSE.txt": FONTS / "mona-sans/LICENSE",
    "newsreader-LICENSE.txt": FONTS / "newsreader/LICENSE",
    "geist-LICENSE.txt": FONTS / "geist/LICENSE",
    "geist-mono-LICENSE.txt": FONTS / "geist-mono/LICENSE",
}

# Output page, title, Markdown source and search description of every text
# page. A page without a description stays out of search results.
PAGES = [
    ("privacy.html", "Privacy policy", ROOT / "docs/legal/privacy-policy.md",
     "What Vaultime keeps on your PC, what a cloud backup server stores and how to reach the person responsible for the website."),
    ("terms.html", "Terms of service", ROOT / "docs/legal/terms-of-service.md",
     "The terms for the Vaultime website and the software you download from it."),
    ("changelog.html", "Changelog", ROOT / "CHANGELOG.md",
     "Every Vaultime release and what changed in it, newest first."),
    ("download-soon.html", "Download not available", SITE / "pages/download-soon.md", None),
    ("404.html", "Page not found", SITE / "pages/404.md", None),
]
UNLISTED = {name for name, _, _, description in PAGES if description is None}

# The documentation in sidebar order: groups of source file and sidebar label.
DOCS_NAV = [
    ("Start", [
        ("index.md", "Overview"),
        ("getting-started.md", "Getting started"),
        ("library.md", "Home and the library"),
    ]),
    ("Playtime", [
        ("counting.md", "How playtime is counted"),
        ("journal-and-stats.md", "The journal and stats"),
        ("game-pages.md", "Game pages and artwork"),
    ]),
    ("Trust", [
        ("trust-labels.md", "Trust labels"),
        ("ledger.md", "The ledger and integrity"),
        ("several-pcs.md", "Several PCs"),
    ]),
    ("Your data", [
        ("backups.md", "Backups"),
        ("own-server.md", "Your own server"),
        ("data-and-privacy.md", "Your data and privacy"),
    ]),
    ("Help", [
        ("platforms.md", "Platforms and Linux"),
        ("troubleshooting.md", "Troubleshooting"),
    ]),
]

# The guides to playtime in each launcher, as DOCS_NAV.
GUIDES_NAV = [
    ("Overview", [
        ("index.md", "All guides"),
        ("all-your-playtime.md", "All your playtime in one place"),
    ]),
    ("Launchers", [
        ("steam-playtime.md", "Steam"),
        ("steam-deck-playtime.md", "Steam Deck"),
        ("epic-games-playtime.md", "Epic Games"),
        ("gog-galaxy-playtime.md", "GOG Galaxy"),
        ("xbox-app-playtime.md", "Xbox app"),
        ("ea-app-playtime.md", "EA app"),
        ("ubisoft-connect-playtime.md", "Ubisoft Connect"),
        ("battle-net-playtime.md", "Battle.net"),
        ("riot-games-playtime.md", "League of Legends and VALORANT"),
        ("linux-playtime.md", "Heroic and Lutris on Linux"),
    ]),
]

# A page lists its sections beside the text once it has this many.
TOC_MIN_SECTIONS = 3

CALLOUTS = {"NOTE": "Note", "TIP": "Tip", "CAUTION": "Caution"}


def section_url(prefix, source):
    return prefix if source == "index.md" else f"{prefix}{source.removesuffix('.md')}/"


def doc_url(source):
    return section_url("/docs/", source)


def guide_url(source):
    return section_url("/guides/", source)


# Links between the Markdown files point at their pages instead, found by
# file name, so two pages may not share one. The guides link to their
# overview as /guides/, since the docs have an index.md too.
def page_urls():
    urls = {}
    pages = [(source.name, f"/{page}") for page, _, source, _ in PAGES]
    pages += [(source, doc_url(source)) for _, entries in DOCS_NAV for source, _ in entries]
    pages += [(source, guide_url(source)) for _, entries in GUIDES_NAV for source, _ in entries if source != "index.md"]
    for name, url in pages:
        if name in urls:
            raise SystemExit(f"Two pages are named {name}, so links to it would be ambiguous")
        urls[name] = url
    return urls


URL_FOR_SOURCE = page_urls()


def link_target(target):
    if re.match(r"[a-z]+:", target) or target.startswith(("/", "#")):
        return target
    path, _, anchor = target.partition("#")
    name = Path(path).name
    folder = Path(path).parent.name
    if folder == "screenshots":
        return f"/screenshots/{name}"
    if folder == "art":
        return f"/art/{name}"
    if name.endswith(".md") and name not in URL_FOR_SOURCE:
        raise SystemExit(f"Link to a page the site does not have: {target}")
    url = URL_FOR_SOURCE.get(name, path)
    return url + (f"#{anchor}" if anchor else "")


def inline(text):
    codes = []

    def keep_code(match):
        codes.append(f"<code>{match.group(1)}</code>")
        return f"\x00{len(codes) - 1}\x00"

    text = html.escape(text, quote=False)
    text = re.sub(r"`(.+?)`", keep_code, text)
    text = re.sub(
        r"\+\+(.+?)\+\+",
        lambda match: "+".join(f"<kbd>{key}</kbd>" for key in match.group(1).split("+")),
        text,
    )
    text = re.sub(r"\*\*(.+?)\*\*", r"<strong>\1</strong>", text)
    text = re.sub(r"(?<![*\w])\*(?=\S)(.+?)(?<=\S)\*(?![*\w])", r"<em>\1</em>", text)

    def link(match):
        label, target = match.group(1), html.unescape(match.group(2))
        return f'<a href="{html.escape(link_target(target))}">{label}</a>'

    text = re.sub(r"\[([^\]]+)\]\(([^)]+)\)", link, text)
    # "[Unreleased]" in a heading is a reference without a target.
    text = re.sub(r"\[([^\]]+)\]", r"\1", text)
    return re.sub(r"\x00(\d+)\x00", lambda match: codes[int(match.group(1))], text)


def plain(fragment):
    """The text of an HTML fragment, without tags and entities."""
    return html.unescape(re.sub(r"<[^>]+>", "", fragment))


def slug(text):
    words = re.sub(r"[^\w\s-]", "", plain(text).lower())
    return re.sub(r"[\s-]+", "-", words).strip("-") or "section"


class Rendered:
    """HTML of a page with its title, lead sentence and sections."""

    def __init__(self):
        self.blocks = []
        self.title = ""
        self.lead = ""
        self.sections = []
        self.ids = set()

    def unique_id(self, text):
        base = slug(text)
        candidate, count = base, 2
        while candidate in self.ids:
            candidate, count = f"{base}-{count}", count + 1
        self.ids.add(candidate)
        return candidate


def markdown(text, anchors=False, rendered=None):
    """The Markdown the pages use: headings with ids, paragraphs, lists,
    tables, code blocks, callouts, definition lists, figures and inline marks.
    `::: name` on a line of its own leaves a marker the caller fills in."""
    out = rendered or Rendered()
    lines = text.splitlines()
    paragraph = []
    items = []
    list_tag = "ul"
    terms = []
    after_title = False

    def flush():
        nonlocal after_title
        if paragraph:
            body = inline(" ".join(paragraph))
            if after_title and not out.lead:
                out.lead = plain(body)
                out.blocks.append(f'<p class="lead">{body}</p>')
            else:
                out.blocks.append(f"<p>{body}</p>")
            paragraph.clear()
            after_title = False
        if items:
            out.blocks.append(f"<{list_tag}>" + "".join(f"<li>{inline(item)}</li>" for item in items) + f"</{list_tag}>")
            items.clear()
        if terms:
            rows = "".join(
                f"<div><dt>{inline(term)}</dt>" + "".join(f"<dd>{inline(part)}</dd>" for part in parts) + "</div>"
                for term, parts in terms
            )
            out.blocks.append(f"<dl>{rows}</dl>")
            terms.clear()

    index = 0
    while index < len(lines):
        line = lines[index]
        stripped = line.strip()
        heading = re.match(r"(#{1,4}) (.+)", stripped)
        ordered = re.match(r"\d+\. (.+)", stripped)
        figure = re.fullmatch(r'!\[([^\]]*)\]\(([^)\s]+)(?: "([^"]*)")?\)', stripped)
        upcoming_term = index + 1 < len(lines) and lines[index + 1].strip().startswith(": ")
        if not stripped:
            # Terms separated by blank lines stay in one list.
            next_term = index + 2 < len(lines) and lines[index + 1].strip() and lines[index + 2].strip().startswith(": ")
            if not (terms and next_term):
                flush()
        elif stripped.startswith("```"):
            flush()
            code = []
            index += 1
            while index < len(lines) and not lines[index].strip().startswith("```"):
                code.append(lines[index])
                index += 1
            out.blocks.append(f"<pre><code>{html.escape(chr(10).join(code), quote=False)}</code></pre>")
        elif heading:
            flush()
            level = len(heading.group(1))
            body = inline(heading.group(2))
            if level == 1:
                out.title = plain(body)
                out.blocks.append(f"<h1>{body}</h1>")
                after_title = True
            else:
                anchor_id = out.unique_id(body)
                link = f' <a class="anchor" href="#{anchor_id}" aria-label="Link to this section">#</a>' if anchors else ""
                out.blocks.append(f'<h{level} id="{anchor_id}">{body}{link}</h{level}>')
                if level in (2, 3):
                    out.sections.append((level, anchor_id, plain(body)))
        elif stripped.startswith("::: "):
            flush()
            out.blocks.append(f"<!--{stripped[4:].strip()}-->")
        elif figure:
            flush()
            alt, target, caption = figure.groups()
            caption_html = f"<figcaption>{inline(caption)}</figcaption>" if caption else ""
            out.blocks.append(
                f'<figure><img src="{html.escape(link_target(target))}" alt="{html.escape(alt)}" '
                f'width="1600" height="1000" loading="lazy" />{caption_html}</figure>'
            )
        elif stripped.startswith("|") and index + 1 < len(lines) and re.fullmatch(r"\|[\s:|-]+\|", lines[index + 1].strip()):
            flush()

            def cells(row):
                return [cell.strip() for cell in row.strip().strip("|").split("|")]

            head = "".join(f"<th>{inline(cell)}</th>" for cell in cells(stripped))
            rows = []
            index += 2
            while index < len(lines) and lines[index].strip().startswith("|"):
                rows.append("<tr>" + "".join(f"<td>{inline(cell)}</td>" for cell in cells(lines[index])) + "</tr>")
                index += 1
            index -= 1
            out.blocks.append(f'<div class="table"><table><thead><tr>{head}</tr></thead><tbody>{"".join(rows)}</tbody></table></div>')
        elif stripped.startswith(">"):
            flush()
            quoted = []
            while index < len(lines) and lines[index].strip().startswith(">"):
                quoted.append(re.sub(r"^\s*> ?", "", lines[index]))
                index += 1
            index -= 1
            kind = re.fullmatch(r"\[!(\w+)\]", quoted[0].strip())
            inner = Rendered()
            inner.ids = out.ids
            if kind and kind.group(1) in CALLOUTS:
                markdown("\n".join(quoted[1:]), anchors, inner)
                name = kind.group(1).lower()
                out.blocks.append(
                    f'<aside class="callout {name}"><div class="label-small">{CALLOUTS[kind.group(1)]}</div>{"".join(inner.blocks)}</aside>'
                )
            else:
                markdown("\n".join(quoted), anchors, inner)
                out.blocks.append(f"<blockquote>{''.join(inner.blocks)}</blockquote>")
        elif stripped.startswith(": ") and (paragraph or terms):
            if paragraph:
                terms.append((" ".join(paragraph), []))
                paragraph.clear()
            terms[-1][1].append(stripped[2:])
        elif stripped.startswith("- ") or ordered:
            tag = "ol" if ordered else "ul"
            if paragraph or terms or (items and tag != list_tag):
                flush()
            list_tag = tag
            items.append(ordered.group(1) if ordered else stripped[2:])
        elif items:
            items[-1] += " " + stripped
        elif terms and line.startswith(" "):
            terms[-1][1][-1] += " " + stripped
        else:
            if terms and not upcoming_term:
                flush()
            paragraph.append(stripped)
        index += 1
    flush()
    return out


def set_content(head, attribute, value):
    """Sets the content of the one meta tag with `attribute` in `head`."""
    head, count = re.subn(
        rf'(<meta {re.escape(attribute)} content=")[^"]*', lambda match: match.group(1) + html.escape(value), head
    )
    if count != 1:
        raise ValueError(f"the page template has {count} meta tags with {attribute}")
    return head


def page(template, title, main, path, description=None, current=None, body_class=None):
    head_end = template.index("</head>")
    header = template[template.index("<header>"):template.index("</header>") + len("</header>")]
    footer = template[template.index("<footer>"):template.index("</footer>") + len("</footer>")]
    full_title = f"{title}, Vaultime"
    head = re.sub(r"<title>.*?</title>", f"<title>{html.escape(full_title)}</title>", template[:head_end])
    head = set_content(head, 'property="og:title"', full_title)
    if description:
        head = set_content(head, 'name="description"', description)
        head = set_content(head, 'property="og:description"', description)
    url = SITE_URL + path
    head = set_content(head, 'property="og:url"', url)
    if path.removeprefix("/") in UNLISTED:
        head = re.sub(r'\n\s*<link rel="canonical"[^>]*>', "", head)
        head += '  <meta name="robots" content="noindex" />\n'
    else:
        head = re.sub(r'(<link rel="canonical" href=")[^"]*', lambda match: match.group(1) + html.escape(url), head)
    # Anchors of the landing page live on the landing page.
    header = header.replace('href="#', 'href="/#')
    if current:
        header = header.replace(f'href="{current}"', f'href="{current}" aria-current="page"')
    body = f'<body class="{body_class}">' if body_class else "<body>"
    return f"{head}</head>\n{body}\n  {header}\n\n{main}\n\n  {footer}\n</body>\n</html>\n"


def text_page(template, title, body, path, description):
    main = f'  <main class="page">\n    <div class="wrap prose">\n{body}\n    </div>\n  </main>'
    return page(template, title, main, path, description)


def landing_page(template):
    """The landing page with what search engines show about the app."""
    description = html.unescape(re.search(r'<meta name="description" content="([^"]*)"', template).group(1))
    app = {
        "@context": "https://schema.org",
        "@type": "SoftwareApplication",
        "name": "Vaultime",
        "description": description,
        "applicationCategory": "UtilitiesApplication",
        "operatingSystem": "Windows, Linux",
        "softwareVersion": json.loads(TAURI_CONFIG.read_text(encoding="utf8"))["version"],
        "url": f"{SITE_URL}/",
        "downloadUrl": f"{SITE_URL}/#download",
        "image": f"{SITE_URL}/social-preview.png",
        "screenshot": f"{SITE_URL}/screenshots/library.png",
        "license": "https://www.gnu.org/licenses/gpl-3.0.html",
        "isAccessibleForFree": True,
        "offers": {"@type": "Offer", "price": "0", "priceCurrency": "EUR"},
        "author": {"@type": "Person", "name": "Dominik Schwimmbeck"},
        "sameAs": [REPO_URL],
    }
    # A data block, so the site's content security policy leaves it alone.
    data = json.dumps(app, indent=2).replace("</", "<\\/")
    block = f'  <script type="application/ld+json">\n{data}\n  </script>\n'
    head_end = template.index("</head>")
    return template[:head_end] + block + template[head_end:]


def sitemap():
    """Every page search engines may show, for sitemap.xml."""
    paths = ["/"]
    paths += [doc_url(source) for _, entries in DOCS_NAV for source, _ in entries]
    paths += [guide_url(source) for _, entries in GUIDES_NAV for source, _ in entries]
    paths += [f"/{name}" for name, _, _, _ in PAGES if name not in UNLISTED]
    urls = "".join(f"  <url><loc>{html.escape(SITE_URL + path)}</loc></url>\n" for path in paths)
    return f'<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n{urls}</urlset>\n'


# The API and the installers are no pages to show.
ROBOTS = f"""User-agent: *
Disallow: /v1/
Disallow: /downloads/

Sitemap: {SITE_URL}/sitemap.xml
"""


def docs_nav(current, nav=DOCS_NAV, url=doc_url):
    groups = []
    for group, entries in nav:
        links = "".join(
            f'<li><a href="{url(source)}"' + (' aria-current="page"' if source == current else "") + f">{html.escape(label)}</a></li>"
            for source, label in entries
        )
        groups.append(f'<div class="docs-nav-group"><div class="label-small">{html.escape(group)}</div><ul>{links}</ul></div>')
    return "".join(groups)


def docs_toc(sections):
    items = "".join(
        f'<li class="toc-{level}"><a href="#{anchor_id}">{html.escape(text)}</a></li>' for level, anchor_id, text in sections
    )
    return f"<ol>{items}</ol>"


def docs_contents(pages, nav=DOCS_NAV, url=doc_url):
    """The overview's list of every page with its lead sentence."""
    groups = []
    for group, entries in nav:
        rows = "".join(
            f'<a class="docs-card" href="{url(source)}"><span class="docs-card-title">{html.escape(label)}</span>'
            f'<span class="docs-card-lead">{html.escape(pages[source].lead)}</span></a>'
            for source, label in entries
            if source != "index.md"
        )
        if rows:
            groups.append(f'<section class="docs-group"><h2 class="label-small">{html.escape(group)}</h2><div class="docs-cards">{rows}</div></section>')
    return "".join(groups)


def build_docs(template, nav=DOCS_NAV, folder=DOCS, url=doc_url, name="Docs", index_title="Documentation"):
    """The pages of one section of the site, the docs or the guides."""
    home = url("index.md")
    order = [(group, source, label) for group, entries in nav for source, label in entries]
    pages = {source: markdown((folder / source).read_text(encoding="utf8"), anchors=True) for _, source, _ in order}
    for position, (group, source, label) in enumerate(order):
        rendered = pages[source]
        body = "\n".join(rendered.blocks).replace("<!--contents-->", docs_contents(pages, nav, url))
        sections = rendered.sections if len([s for s in rendered.sections if s[0] == 2]) >= TOC_MIN_SECTIONS else []
        pager = []
        if position > 0:
            _, previous, previous_label = order[position - 1]
            pager.append(f'<a class="prev" href="{url(previous)}"><span class="label-small">Previous</span><span class="pager-title">{html.escape(previous_label)}</span></a>')
        if position + 1 < len(order):
            _, following, following_label = order[position + 1]
            pager.append(f'<a class="next" href="{url(following)}"><span class="label-small">Next</span><span class="pager-title">{html.escape(following_label)}</span></a>')
        crumb = f'<div class="label-small crumb"><a href="{home}">{name}</a>' + (f" <span>/</span> {html.escape(group)}" if source != "index.md" else "") + "</div>"
        toc_side = ""
        if sections:
            toc_side = f'<aside class="docs-toc" aria-label="On this page"><div class="label-small">On this page</div>{docs_toc(sections)}</aside>'
            # Narrow windows get the section list right after the lead sentence.
            toc_inline = f'<details class="toc-inline"><summary class="label-small">On this page</summary>{docs_toc(sections)}</details>'
            at = body.find('<p class="lead">')
            end = body.find("</p>", at) + len("</p>") if at >= 0 else body.find("</h1>") + len("</h1>")
            body = body[:end] + toc_inline + body[end:]
        edit = f"{REPO_URL}/blob/main/docs/site/{folder.name}/{source}"
        links = docs_nav(source, nav, url)
        main = (
            '  <div class="docs' + (" has-toc" if sections else "") + '">\n'
            f'    <aside class="docs-side" aria-label="{name}">\n'
            f'      <details class="docs-menu"><summary><span class="label-small">{name}</span>'
            f'<span class="docs-menu-current">{html.escape(label)}</span></summary><nav>{links}</nav></details>\n'
            f'      <nav class="docs-nav">{links}</nav>\n'
            "    </aside>\n"
            '    <main class="docs-main">\n'
            f'      <article class="docs-article">\n{crumb}\n{body}\n      </article>\n'
            f'      <nav class="pager" aria-label="More pages">{"".join(pager)}</nav>\n'
            f'      <p class="edit"><a href="{edit}">Edit this page on GitHub</a></p>\n'
            "    </main>\n"
            f"    {toc_side}\n"
            "  </div>"
        )
        title = rendered.title if source != "index.md" else index_title
        target = OUT / home.strip("/") / ("" if source == "index.md" else source.removesuffix(".md")) / "index.html"
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(
            page(template, title, main, url(source), rendered.lead, home, "docs-page"), encoding="utf8", newline="\n"
        )


def main():
    # Empties the folder instead of removing it, so a server running in it
    # keeps working on Windows.
    OUT.mkdir(exist_ok=True)
    for path in OUT.iterdir():
        if path.is_dir():
            shutil.rmtree(path)
        else:
            path.unlink()
    (OUT / "fonts").mkdir()
    for name, source in FONT_FILES.items():
        shutil.copyfile(source, OUT / "fonts" / name)
    shutil.copytree(SCREENSHOTS, OUT / "screenshots")
    shutil.copytree(ART, OUT / "art")
    shutil.copyfile(SOCIAL_PREVIEW, OUT / "social-preview.png")
    for asset in ("site.css", "site.js"):
        shutil.copyfile(SITE / asset, OUT / asset)
    # Search Console verification files, served as they are.
    for verification in SITE.glob("google*.html"):
        shutil.copyfile(verification, OUT / verification.name)

    template = (SITE / "index.html").read_text(encoding="utf8")
    (OUT / "index.html").write_text(landing_page(template), encoding="utf8", newline="\n")
    for name, title, source, description in PAGES:
        body = "\n".join(markdown(source.read_text(encoding="utf8")).blocks)
        (OUT / name).write_text(text_page(template, title, body, f"/{name}", description), encoding="utf8", newline="\n")
    build_docs(template)
    build_docs(template, GUIDES_NAV, GUIDES, guide_url, "Guides", "Playtime guides")
    (OUT / "sitemap.xml").write_text(sitemap(), encoding="utf8", newline="\n")
    (OUT / "robots.txt").write_text(ROBOTS, encoding="utf8", newline="\n")
    print("wrote", ", ".join(sorted(path.name for path in OUT.iterdir())))


if __name__ == "__main__":
    main()
