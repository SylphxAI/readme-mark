#!/usr/bin/env python3
"""Offline regressions for the live corpus probe; no network or credentials."""

import importlib.util
import io
from pathlib import Path
import subprocess
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location(
    "probe_dialects", Path(__file__).with_name("probe-dialects.py")
)
probe = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(probe)
CASE = {"dialect": "shields", "url": "https://img.shields.io/badge/a-b-blue",
        "geometry": ['height="28"'], "text": ["REACT"]}
SVG = '<svg xmlns="http://www.w3.org/2000/svg" height="28"><text>REACT</text></svg>'
GOOD = (SVG, "200", "image/svg+xml; charset=utf-8")


class ProbeTests(unittest.TestCase):
    def test_host_only_swap(self):
        self.assertEqual(
            probe.target_url("https://mark.sylphx.com/", "https://upstream/?lines=A%20B;C&color=%23FFF#gh-light-mode-only"),
            "https://mark.sylphx.com/?lines=A%20B;C&color=%23FFF",
        )

    def test_valid_response(self):
        self.assertEqual(probe.check(CASE, GOOD), [])

    def test_bad_status_type_xml_geometry_text_and_fallback(self):
        responses = [
            (SVG, "302", "image/svg+xml"),
            (SVG, "200", "text/html"),
            ("<svg", "200", "image/svg+xml"),
            ("<html/>", "200", "image/svg+xml"),
            (SVG.replace('height="28"', 'height="29"'), "200", "image/svg+xml"),
            (SVG.replace("REACT", "OTHER"), "200", "image/svg+xml"),
            (SVG.replace("REACT", "REACT temporarily unavailable"), "200", "image/svg+xml"),
        ]
        for response in responses:
            with self.subTest(response=response):
                self.assertTrue(probe.check(CASE, response))

    def test_failure_continues_and_reports_each_dialect(self):
        output = io.StringIO()
        cases = [CASE, dict(CASE, dialect="skill-icons")]
        with patch.object(probe, "fetch", side_effect=[OSError("offline"), GOOD]) as fetch:
            self.assertEqual(probe.probe(cases, "https://mark.sylphx.com", fetch, output), 1)
            self.assertEqual(fetch.call_count, 2)
        self.assertIn("FAIL shields: 0/1", output.getvalue())
        self.assertIn("PASS skill-icons: 1/1", output.getvalue())

    def test_all_pass_and_empty_fails(self):
        self.assertEqual(probe.probe([CASE], "https://mark.sylphx.com", lambda _: GOOD, io.StringIO()), 0)
        self.assertEqual(probe.probe([], "https://mark.sylphx.com", output=io.StringIO()), 1)

    def test_curl_is_bounded_no_config_no_redirects(self):
        raw = (SVG + "\n200\nimage/svg+xml").encode()
        with patch.object(probe.subprocess, "run", return_value=subprocess.CompletedProcess([], 0, raw)) as run:
            self.assertEqual(probe.fetch("https://mark.sylphx.com/"), (SVG, "200", "image/svg+xml"))
        args = run.call_args.args[0]
        self.assertEqual(args[:2], ["curl", "--disable"])
        self.assertIn("--max-time", args)
        self.assertIn("--max-filesize", args)
        self.assertNotIn("--location", args)
        self.assertNotIn("--user", args)
        self.assertEqual(run.call_args.kwargs["timeout"], 10)

    def test_curl_error_timeout_and_invalid_encoding_fail(self):
        with patch.object(probe.subprocess, "run", return_value=subprocess.CompletedProcess([], 28)):
            with self.assertRaises(ValueError):
                probe.fetch("https://mark.sylphx.com/")
        for error in [subprocess.TimeoutExpired("curl", 10), UnicodeDecodeError("utf8", b"\xff", 0, 1, "bad")]:
            with self.subTest(error=error):
                self.assertEqual(probe.probe([CASE], "https://mark.sylphx.com", lambda _: (_ for _ in ()).throw(error), io.StringIO()), 1)


if __name__ == "__main__":
    unittest.main()
