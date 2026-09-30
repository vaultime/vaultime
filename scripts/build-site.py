#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
# SPDX-License-Identifier: GPL-3.0-or-later
"""Builds the website into dist-site: the landing page, its styles, the fonts
the app ships and HTML versions of the legal pages and the changelog.

    python scripts/build-site.py

Needs `npm ci` in apps/desktop first for the font files. The fonts are served
with the site, so visitors make no requests to font services.
"""

import html
import re
import shutil
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SITE = ROOT / "docs/site"
OUT = ROOT / "dist-site"
FONTS = ROOT / "apps/desktop/node_modules/@fontsource-variable"

FONT_FILES = {
    "fraunces-normal.woff2": FONTS / "fraunces/files/fraunces-latin-full-normal.woff2",
    "fraunces-italic.woff2": FONTS / "fraunces/files/fraunces-latin-full-italic.woff2",
    "hanken-grotesk.woff2": FONTS / "hanken-grotesk/files/hanken-grotesk-latin-wght-normal.woff2",
    "jetbrains-mono.woff2": FONTS / "jetbrains-mono/files/jetbrains-mono-latin-wght-normal.woff2",
}

# Output page, title and Markdown source of every text page.
PAGES = [
    ("privacy.html", "Privacy policy", ROOT / "docs/legal/privacy-policy.md"),
    ("terms.html", "Terms of service", ROOT / "docs/legal/terms-of-service.md"),
    ("changelog.html", "Changelog", ROOT / "CHANGELOG.md"),
    ("applied.html", "Thanks for applying", SITE / "pages/applied.md"),
    ("apply-failed.html", "That did not go through", SITE / "pages/apply-failed.md"),
    ("download-soon.html", "Download not available", SITE / "pages/download-soon.md"),
    ("404.html", "Page not found", SITE / "pages/404.md"),
]

# Links between the Markdown files point at their pages instead.
PAGE_FOR_SOURCE = {source.name: page for page, _, source in PAGES}


def inline(text):
    text = html.escape(text, quote=False)
    text = re.sub(r"\*\*(.+?)\*\*", r"<strong>\1</strong>", text)
    text = re.sub(r"`(.+?)`", r"<code>\1</code>", text)

    def link(match):
        label, target = match.group(1), match.group(2)
        target = PAGE_FOR_SOURCE.get(target, target)
        return f'<a href="{html.escape(target)}">{label}</a>'

    text = re.sub(r"\[([^\]]+)\]\(([^)]+)\)", link, text)
    # "[Unreleased]" in a heading is a reference without a target.
    return re.sub(r"\[([^\]]+)\]", r"\1", text)


def markdown(text):
    """The Markdown the pages use: headings, paragraphs, lists and inline marks."""
    blocks = []
    paragraph = []
    items = []

    def flush():
        if paragraph:
            blocks.append(f"<p>{inline(' '.join(paragraph))}</p>")
            paragraph.clear()
        if items:
            blocks.append("<ul>" + "".join(f"<li>{inline(item)}</li>" for item in items) + "</ul>")
            items.clear()

    for line in text.splitlines():
        stripped = line.strip()
        heading = re.match(r"(#{1,3}) (.+)", stripped)
        if not stripped:
            flush()
        elif heading:
            flush()
            level = len(heading.group(1))
            blocks.append(f"<h{level}>{inline(heading.group(2))}</h{level}>")
        elif stripped.startswith("- "):
            if paragraph:
                flush()
            items.append(stripped[2:])
        elif items and line.startswith(" "):
            items[-1] += " " + stripped
        else:
            paragraph.append(stripped)
    flush()
    return "\n".join(blocks)


def page(template, title, body):
    head_end = template.index("</head>")
    header = template[template.index("<header>"):template.index("</header>") + len("</header>")]
    footer = template[template.index("<footer>"):template.index("</footer>") + len("</footer>")]
    head = re.sub(r"<title>.*?</title>", f"<title>{html.escape(title)}, Vaultime</title>", template[:head_end])
    # Anchors of the landing page live on the landing page.
    header = header.replace('href="#', 'href="/#')
    return (
        f"{head}</head>\n<body>\n  {header}\n\n"
        f'  <main class="page">\n    <div class="wrap prose">\n{body}\n    </div>\n  </main>\n\n'
        f"  {footer}\n</body>\n</html>\n"
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
    shutil.copyfile(SITE / "index.html", OUT / "index.html")
    for asset in ("site.css", "site.js"):
        shutil.copyfile(SITE / asset, OUT / asset)

    template = (SITE / "index.html").read_text(encoding="utf8")
    for name, title, source in PAGES:
        body = markdown(source.read_text(encoding="utf8"))
        (OUT / name).write_text(page(template, title, body), encoding="utf8", newline="\n")
    print("wrote", ", ".join(sorted(path.name for path in OUT.iterdir())))


if __name__ == "__main__":
    main()
