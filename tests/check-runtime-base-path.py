"""Smoke-test one immutable exported frontend at several runtime prefixes.

Usage: python3 tests/check-runtime-base-path.py backend/target/debug/oxi-email-server frontend/out
Uses only Python's standard library. Browser navigation is a separate E2E check.
"""

from contextlib import contextmanager
from html.parser import HTMLParser
from pathlib import Path
import hashlib
import json
import os
import signal
import socket
import subprocess
import sys
import tempfile
from urllib.error import HTTPError
from urllib.parse import urlsplit
from urllib.request import HTTPRedirectHandler, build_opener

MARKER = "/__OXI_BASE_PATH__"


class NoRedirect(HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None


class Assets(HTMLParser):
    def __init__(self):
        super().__init__()
        self.urls = []

    def handle_starttag(self, tag, attrs):
        for key, value in attrs:
            if key in ("src", "href") and value and value.startswith("/"):
                self.urls.append(value)


@contextmanager
def server(binary, export, prefix, shutdown=signal.SIGTERM):
    with tempfile.TemporaryDirectory(prefix="oxi-runtime-test-") as data:
        with socket.socket() as port_socket:
            port_socket.bind(("127.0.0.1", 0))
            port = port_socket.getsockname()[1]
        temporary_assets = Path(data) / "tmp"
        temporary_assets.mkdir()
        env = dict(os.environ, HOST="127.0.0.1", PORT=str(port), STATIC_DIR=str(export),
                   DATA_DIR=data, TMPDIR=str(temporary_assets), ENVIRONMENT="production", RUST_LOG="info")
        if prefix is None:
            env.pop("BASE_PATH", None)
        else:
            env["BASE_PATH"] = prefix
        process = subprocess.Popen([str(binary)], env=env, stdout=subprocess.PIPE,
                                   stderr=subprocess.STDOUT, text=True)
        try:
            for line in process.stdout:
                if "oxi-email server starting" in line:
                    break
            else:
                raise AssertionError(f"server exited before startup: {process.wait()}")
            yield f"http://127.0.0.1:{port}"
        finally:
            process.send_signal(shutdown)
            try:
                process.communicate(timeout=10)
            except subprocess.TimeoutExpired:
                process.kill()
                process.communicate()
                raise
            assert process.returncode == 0, f"shutdown failed: {process.returncode}"
            assert not list(temporary_assets.iterdir()), "runtime frontend leaked after shutdown"


def check(binary, export):
    originals = {path.relative_to(export): hashlib.sha256(path.read_bytes()).digest()
                 for path in export.rglob("*") if path.is_file()}
    assert MARKER in (export / "index.html").read_text(), "Build with NEXT_PUBLIC_BASE_PATH=" + MARKER
    opener = build_opener(NoRedirect)
    for configured in (None, "", "/", "/oxi", "/mail", "/apps/mail/"):
        prefix = (configured or "").rstrip("/")
        with server(binary, export, configured) as origin:
            def request(path):
                try:
                    response = opener.open(origin + path, timeout=10)
                except HTTPError as error:
                    response = error
                return response.status, response.headers, response.read()

            for page in ("/", "/login/"):
                status, _, body = request(prefix + page)
                assert status == 200, (configured, page, status)
                assert MARKER.encode() not in body
                parser = Assets()
                parser.feed(body.decode())
                assert parser.urls, "No assets found"
                for url in parser.urls:
                    assert urlsplit(url).path.startswith(prefix + "/"), url
                    assert request(url)[0] == 200, url
            status, headers, _ = request(prefix + "/login?next=mail")
            assert status == 307
            assert headers["Location"] == prefix + "/login/?next=mail"
            assert request(prefix + "/api/health")[0] == 200
            assert json.loads(request(prefix + "/api/auth/accounts")[2])["accounts"] == []
            assert request(prefix + "/api/auth/session")[0] == 401
            # Check every generated text asset, including JS router/chunk loader and RSC payloads.
            replacements = 0
            for relative in originals:
                if relative.suffix not in (".html", ".js", ".css", ".txt", ".json", ".svg", ".map"):
                    continue
                template = (export / relative).read_text()
                if MARKER not in template:
                    continue
                status, _, body = request(prefix + "/" + relative.as_posix())
                assert status == 200, relative
                assert body == template.replace(MARKER, prefix).encode(), relative
                replacements += 1
            assert replacements > 5, "Expected HTML, JS, and RSC templates"
            if prefix:
                for path in ("/_next/missing.js", "/api/health", "/login/"):
                    assert request(path)[0] == 404, path
        print(f"Runtime prefix {configured!r}: pages, assets, redirects, API, and {replacements} templates OK")
    with server(binary, export, "/mail", shutdown=signal.SIGINT):
        pass  # The server context verifies SIGINT cleanup as well as SIGTERM.
    for invalid in ("mail", "/apps//mail", "/apps/../mail", "/mail?x=1", " /mail", "/mail "):
        result = subprocess.run([str(binary)], env=dict(os.environ, BASE_PATH=invalid),
                                capture_output=True, text=True, timeout=10)
        assert result.returncode != 0, invalid
        assert "invalid BASE_PATH" in result.stderr, (invalid, result.stderr)
    for relative, digest in originals.items():
        assert hashlib.sha256((export / relative).read_bytes()).digest() == digest, relative
    print("Source export unchanged across all starts")


if __name__ == "__main__":
    check(Path(sys.argv[1]).resolve(), Path(sys.argv[2]).resolve())
