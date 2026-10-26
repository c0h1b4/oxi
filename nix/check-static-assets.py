"""Check static-export asset URLs against the prefix fixed at build time."""

from html.parser import HTMLParser
from pathlib import Path
import sys
from urllib.parse import unquote, urlsplit


class AssetParser(HTMLParser):
    def __init__(self):
        super().__init__()
        self.urls = []

    def handle_starttag(self, tag, attrs):
        for key, value in attrs:
            if key in ("src", "href") and value and "/_next/" in value:
                self.urls.append(value)


def check(export_dir, base_path):
    expected_prefix = base_path + "/_next/"
    assets = 0
    for page in export_dir.rglob("*.html"):
        parser = AssetParser()
        parser.feed(page.read_text())
        for url in parser.urls:
            parsed = urlsplit(url)
            assert not parsed.scheme and not parsed.netloc, f"{page}: external asset {url}"
            assert parsed.path.startswith(expected_prefix), f"{page}: wrong asset prefix {url}"
            relative = unquote(parsed.path[len(base_path) :]).lstrip("/")
            assert (export_dir / relative).is_file(), f"{page}: missing asset {relative}"
            assets += 1
    fonts = list((export_dir / "_next/static/media").glob("*.ttf"))
    assert fonts, f"{export_dir}: no self-hosted font files found"
    for stylesheet in export_dir.rglob("*.css"):
        css = stylesheet.read_text()
        assert "fonts.googleapis.com" not in css, f"{stylesheet}: Google Fonts CSS URL"
        assert "fonts.gstatic.com" not in css, f"{stylesheet}: Google Fonts asset URL"
    assert assets, f"{export_dir}: no Next.js assets found in exported HTML"
    print(f"Checked {assets} static asset references for base path {base_path!r}")


if __name__ == "__main__":
    check(Path(sys.argv[1]), sys.argv[2])
