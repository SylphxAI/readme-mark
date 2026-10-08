#!/usr/bin/env python3
"""Probe the pinned README corpus by changing only the host (curl, no token)."""

import argparse
import json
from pathlib import Path
import subprocess
import sys
from urllib.parse import urlsplit
import xml.etree.ElementTree as ET

CORPUS = Path(__file__).resolve().parents[1] / "tests/corpus/readme-urls.json"


def target_url(base, upstream):
    """Keep the raw path/query, including encoded characters; omit fragments."""
    parsed = urlsplit(upstream)
    return base.rstrip("/") + (parsed.path or "/") + (
        "?" + parsed.query if parsed.query else ""
    )


def fetch(url):
    # --disable must be first: a local curlrc must not add credentials or retries.
    result = subprocess.run(
        ["curl", "--disable", "--silent", "--show-error", "--globoff",
         "--connect-timeout", "3", "--max-time", "8", "--max-filesize", "2097152",
         "--write-out", "\n%{http_code}\n%{content_type}", url],
        capture_output=True, timeout=10, check=False,
    )
    if result.returncode:
        raise ValueError(f"curl exited {result.returncode}")
    body, status, content_type = result.stdout.decode("utf-8").rsplit("\n", 2)
    return body, status, content_type


def check(case, response):
    body, status, content_type = response
    failures = []
    if status != "200":
        failures.append(f"HTTP {status}, expected 200")
    if content_type.split(";", 1)[0].strip().lower() != "image/svg+xml":
        failures.append(f"content-type {content_type!r}, expected image/svg+xml")
    try:
        if ET.fromstring(body).tag != "{http://www.w3.org/2000/svg}svg":
            failures.append("root is not an SVG")
    except ET.ParseError:
        failures.append("invalid SVG XML")
    for expected in case["geometry"] + case["text"]:
        if expected not in body:
            failures.append(f"missing {expected!r}")
    if "temporarily unavailable" in body.lower():
        failures.append("upstream fallback card")
    return failures


def probe(cases, base, fetcher=fetch, output=sys.stdout):
    totals = {}
    for case in cases:
        dialect = case["dialect"]
        passed, count = totals.get(dialect, (0, 0))
        url = target_url(base, case["url"])
        try:
            failures = check(case, fetcher(url))
        except (ValueError, OSError, subprocess.TimeoutExpired) as error:
            failures = [str(error)]
        totals[dialect] = (passed + (not failures), count + 1)
        print(f"{'FAIL' if failures else 'PASS'} {dialect} {url}", file=output)
        for failure in failures:
            print(f"  {failure}", file=output)
    for dialect, (passed, count) in sorted(totals.items()):
        print(f"{'PASS' if passed == count else 'FAIL'} {dialect}: {passed}/{count}",
              file=output)
    return 0 if totals and all(p == n for p, n in totals.values()) else 1


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("base", nargs="?", default="https://mark.sylphx.com")
    args = parser.parse_args()
    base = urlsplit(args.base)
    if (base.scheme not in ("http", "https") or not base.hostname
            or base.username is not None or base.password is not None
            or base.path not in ("", "/") or base.query or base.fragment):
        parser.error("base must be an HTTP(S) origin without credentials, path or query")
    return probe(json.loads(CORPUS.read_text()), args.base)


if __name__ == "__main__":
    sys.exit(main())
