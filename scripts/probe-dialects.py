#!/usr/bin/env python3
"""Probe the pinned README corpus by changing only the host (curl, no token)."""

import argparse
from datetime import datetime, timezone
import hashlib
import io
import os
import re
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
        if expected not in body and expected not in case.get("live_optional_text", []):
            failures.append(f"missing {expected!r}")
    if "temporarily unavailable" in body.lower():
        failures.append("upstream fallback card")
    return failures


def probe(cases, base, fetcher=fetch, output=sys.stdout, records=None):
    totals = {}
    for index, case in enumerate(cases, 1):
        dialect = case["dialect"]
        passed, count = totals.get(dialect, (0, 0))
        url = target_url(base, case["url"])
        response = None
        try:
            response = fetcher(url)
            failures = check(case, response)
        except (ValueError, OSError, subprocess.TimeoutExpired) as error:
            failures = [str(error)]
        omitted = [text for text in case.get("live_optional_text", [])
                   if response is not None and text not in response[0]]
        if records is not None:
            records.append({
                "case": index, "dialect": dialect, "url": url,
                "original_url": case["url"], "source": case.get("source"),
                "verdict": "fail" if failures else "pass", "failures": failures,
                "optional_omissions": omitted,
                "http_status": response[1] if response is not None else None,
                "content_type": response[2] if response is not None else None,
            })
        totals[dialect] = (passed + (not failures), count + 1)
        print(f"{'FAIL' if failures else 'PASS'} {dialect} {url}", file=output)
        for failure in failures:
            print(f"  {failure}", file=output)
        for expected in omitted:
            print(f"  OPTIONAL upstream metric omitted: {expected}", file=output)
    for dialect, (passed, count) in sorted(totals.items()):
        print(f"{'PASS' if passed == count else 'FAIL'} {dialect}: {passed}/{count}",
              file=output)
    return 0 if totals and all(p == n for p, n in totals.values()) else 1


def timestamp():
    return datetime.now(timezone.utc).isoformat()


def provenance():
    """Identify the checked-in instrument and corpus, not just the trigger SHA."""
    root = CORPUS.parents[2]

    def git(*args):
        return subprocess.check_output(
            ["git", "-C", str(root), *args], timeout=5, text=True,
            stderr=subprocess.PIPE,
        ).strip()

    git("diff", "--exit-code", "HEAD", "--", "scripts/probe-dialects.py",
        "tests/corpus/readme-urls.json")
    return {
        "source_revision": git("rev-parse", "HEAD"),
        "corpus_revision": git("log", "-1", "--format=%H", "--",
                               "tests/corpus/readme-urls.json"),
        "corpus_blob": git("rev-parse", "HEAD:tests/corpus/readme-urls.json"),
        "corpus_sha256": hashlib.sha256(CORPUS.read_bytes()).hexdigest(),
    }


def health_revision(base, fetcher):
    body, status, content_type = fetcher(base.rstrip("/") + "/health")
    if status != "200" or content_type.split(";", 1)[0].strip().lower() != "application/json":
        raise ValueError(f"health HTTP {status}, content-type {content_type!r}")
    health = json.loads(body)
    revision = health.get("revision")
    if health.get("ok") is not True or not isinstance(revision, str) or not re.fullmatch(r"[0-9a-f]{40}", revision):
        raise ValueError("health is not ok or lacks a full serving git revision")
    return revision


def record_run(cases, base, fetcher=fetch, identify=provenance, expected_revision=None):
    """One bounded observation: no waiting, retry, replay or production writes."""
    result = {
        "schema_version": 1, "started_at": timestamp(), "origin": base,
        "trigger": os.environ.get("GITHUB_EVENT_NAME", "manual"),
        "run_url": (f"https://github.com/{os.environ['GITHUB_REPOSITORY']}/actions/runs/"
                    f"{os.environ['GITHUB_RUN_ID']}" if os.environ.get("GITHUB_RUN_ID") else None),
        "expected_serving_revision": expected_revision,
        "serving_revision": None, "serving_revision_after": None,
        "source_revision": None, "corpus_revision": None, "corpus_blob": None,
        "corpus_sha256": None, "errors": [], "cases": [], "dialects": {},
    }
    try:
        result.update(identify())
        if any(not result.get(key) for key in
               ("source_revision", "corpus_revision", "corpus_blob", "corpus_sha256")):
            raise ValueError("incomplete source/corpus provenance")
    except Exception as error:
        result["errors"].append(f"instrument provenance: {type(error).__name__}: {error}")
    try:
        result["serving_revision"] = health_revision(base, fetcher)
        if expected_revision and result["serving_revision"] != expected_revision:
            result["errors"].append("serving revision does not match completed deployment")
    except Exception as error:
        result["errors"].append(f"health before: {type(error).__name__}: {error}")
    output = io.StringIO()
    try:
        probe(cases, base, fetcher, output, result["cases"])
    except Exception as error:
        result["errors"].append(f"instrument: {type(error).__name__}: {error}")
    try:
        result["serving_revision_after"] = health_revision(base, fetcher)
        if result["serving_revision"] != result["serving_revision_after"]:
            result["errors"].append("serving revision changed during probe")
    except Exception as error:
        result["errors"].append(f"health after: {type(error).__name__}: {error}")
    for case in result["cases"]:
        counts = result["dialects"].setdefault(case["dialect"], {"passed": 0, "total": 0})
        counts["total"] += 1
        counts["passed"] += case["verdict"] == "pass"
    result["expected_cases"] = len(cases)
    if not cases or len(result["cases"]) != len(cases):
        result["errors"].append("empty or incomplete corpus observation")
    result["verdict"] = ("unknown" if result["errors"] else
                         "fail" if any(c["verdict"] != "pass" for c in result["cases"]) else "pass")
    result["finished_at"] = timestamp()
    return result, output.getvalue()


def write_result(result, log, directory):
    directory.mkdir(parents=True, exist_ok=True)
    (directory / "result.json").write_text(json.dumps(result, indent=2) + "\n")
    lines = [f"# Production compatibility: {result['verdict'].upper()}", "",
             f"Started: {result['started_at']}  ", f"Finished: {result['finished_at']}", ""]
    for key in ("origin", "trigger", "run_url", "serving_revision", "serving_revision_after",
                "expected_serving_revision", "source_revision", "corpus_revision",
                "corpus_blob", "corpus_sha256", "expected_cases"):
        lines.append(f"- {key}: `{result[key]}`")
    lines.extend(["", "## Instrument errors", "", *result["errors"], "",
                  "## Per-dialect counts", "", "| Dialect | Passed | Total |",
                  "| --- | ---: | ---: |"])
    for dialect, counts in sorted(result["dialects"].items()):
        lines.append(f"| {dialect} | {counts['passed']} | {counts['total']} |")
    lines.extend(["", "## Every corpus case", "", "```text", log, "```", "",
                  "Case sources, original URLs, response metadata, failures and optional omissions "
                  "are also in result.json. No requests are retried."])
    (directory / "summary.md").write_text("\n".join(lines) + "\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("base", nargs="?", default="https://mark.sylphx.com")
    parser.add_argument("--result-dir", type=Path, help="record JSON and a durable run summary")
    parser.add_argument("--expected-revision", help="serving SHA from a completed production deploy")
    args = parser.parse_args()
    base = urlsplit(args.base)
    if (base.scheme not in ("http", "https") or not base.hostname
            or base.username is not None or base.password is not None
            or base.path not in ("", "/") or base.query or base.fragment):
        parser.error("base must be an HTTP(S) origin without credentials, path or query")
    cases = json.loads(CORPUS.read_text())
    if not args.result_dir:
        return probe(cases, args.base)
    result, log = record_run(cases, args.base, expected_revision=args.expected_revision)
    write_result(result, log, args.result_dir)
    print(log, end="")
    print(f"Recorded verdict: {result['verdict']}")
    return 0 if result["verdict"] == "pass" else 1


if __name__ == "__main__":
    sys.exit(main())
