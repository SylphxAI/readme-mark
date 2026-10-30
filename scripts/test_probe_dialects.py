#!/usr/bin/env python3
"""Offline regressions for the live corpus probe; no network or credentials."""

import importlib.util
import io
import json
from pathlib import Path
import subprocess
import tempfile
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

    def test_unknown_live_metrics_are_reported_not_invented(self):
        case = dict(CASE, text=["REACT", "Total Stars Earned"],
                    live_optional_text=["Total Stars Earned"])
        output = io.StringIO()
        self.assertEqual(probe.probe([case], "https://mark.sylphx.com", lambda _: GOOD, output), 0)
        self.assertIn("OPTIONAL upstream metric omitted: Total Stars Earned", output.getvalue())
        self.assertTrue(probe.check(case, (SVG.replace("REACT", "OTHER"), "200", "image/svg+xml")))
        self.assertTrue(probe.check(case, (SVG.replace('height="28"', ''), "200", "image/svg+xml")))

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
        self.assertNotIn("--retry", args)
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


REVISION = "a" * 40
HEALTH = (json.dumps({"ok": True, "revision": REVISION}), "200", "application/json")
PROVENANCE = {"source_revision": "b" * 40, "corpus_revision": "c" * 40,
              "corpus_blob": "d" * 40, "corpus_sha256": "e" * 64}


class RecordedVerdictTests(unittest.TestCase):
    def record(self, response=GOOD, health=HEALTH, identify=lambda: PROVENANCE):
        def fetch(url):
            if url.endswith("/health"):
                return health
            if isinstance(response, Exception):
                raise response
            return response
        with patch.object(probe, "fetch", side_effect=fetch) as fetcher:
            result, log = probe.record_run([CASE], "https://mark.sylphx.com", fetcher, identify)
            # Two distinct revision observations, exactly one request per case.
            self.assertEqual(fetcher.call_count, 3)
        with tempfile.TemporaryDirectory() as tmp:
            probe.write_result(result, log, Path(tmp))
            self.assertEqual(json.loads((Path(tmp) / "result.json").read_text()), result)
            self.assertIn(result["verdict"].upper(), (Path(tmp) / "summary.md").read_text())
        return result

    def test_each_negative_is_a_persisted_failure_without_retry(self):
        for response in [
            subprocess.TimeoutExpired("curl", 10),
            (SVG, "503", "image/svg+xml"),
            (SVG, "200", "text/html"),
            ("<svg", "200", "image/svg+xml"),
            ("<html/>", "200", "image/svg+xml"),
            (SVG.replace("REACT", "OTHER"), "200", "image/svg+xml"),
            (SVG.replace('height="28"', 'height="29"'), "200", "image/svg+xml"),
            (SVG.replace("REACT", "REACT temporarily unavailable"), "200", "image/svg+xml"),
        ]:
            with self.subTest(response=response):
                result = self.record(response)
                self.assertEqual(result["verdict"], "fail")
                self.assertEqual(result["cases"][0]["verdict"], "fail")
                self.assertEqual(result["dialects"], {"shields": {"passed": 0, "total": 1}})

    def test_pass_records_provenance_timestamps_and_counts(self):
        result = self.record()
        self.assertEqual(result["verdict"], "pass")
        self.assertEqual(result["serving_revision"], REVISION)
        self.assertEqual(result["serving_revision_after"], REVISION)
        for key, value in PROVENANCE.items():
            self.assertEqual(result[key], value)
        self.assertLessEqual(result["started_at"], result["finished_at"])
        self.assertEqual(result["dialects"]["shields"], {"passed": 1, "total": 1})

    def test_revision_and_instrument_errors_are_unknown(self):
        for health in [("{}", "200", "application/json"),
                       ("not json", "200", "application/json"),
                       (HEALTH[0], "500", "application/json"),
                       (HEALTH[0], "200", "text/html")]:
            with self.subTest(health=health):
                result = self.record(health=health)
                self.assertEqual(result["verdict"], "unknown")
                self.assertEqual(len(result["cases"]), 1)
        def broken():
            raise OSError("git unavailable")
        self.assertEqual(self.record(identify=broken)["verdict"], "unknown")
        self.assertEqual(self.record(identify=lambda: {})["verdict"], "unknown")
        self.assertEqual(self.record(RuntimeError("instrument bug"))["verdict"], "unknown")

    def test_health_transport_failure_still_attempts_every_case_once(self):
        with patch.object(probe, "fetch", side_effect=[OSError("offline"), GOOD, HEALTH]) as fetcher:
            result, _ = probe.record_run([CASE], "https://mark.sylphx.com", fetcher, lambda: PROVENANCE)
        self.assertEqual(fetcher.call_count, 3)
        self.assertEqual(result["verdict"], "unknown")
        self.assertEqual(len(result["cases"]), 1)

    def test_deploy_mismatch_or_mid_run_release_is_unknown(self):
        for expected, after in [("f" * 40, HEALTH),
                                (REVISION, (HEALTH[0].replace(REVISION, "f" * 40), "200", "application/json"))]:
            with patch.object(probe, "fetch", side_effect=[HEALTH, GOOD, after]) as fetcher:
                result, _ = probe.record_run([CASE], "https://mark.sylphx.com", fetcher,
                                             lambda: PROVENANCE, expected)
                self.assertEqual(result["verdict"], "unknown")
                self.assertEqual(fetcher.call_count, 3)

    def test_record_optional_omissions_and_all_dialects(self):
        cases = [dict(CASE, dialect=dialect, live_optional_text=["Total Stars Earned"])
                 for dialect in ("shields", "skill-icons")]
        with patch.object(probe, "fetch", side_effect=[HEALTH, GOOD, GOOD, HEALTH]) as fetcher:
            result, log = probe.record_run(cases, "https://mark.sylphx.com", fetcher, lambda: PROVENANCE)
        self.assertEqual(fetcher.call_count, 4)
        self.assertEqual(result["verdict"], "pass")
        self.assertEqual(len(result["cases"]), 2)
        self.assertIn("OPTIONAL upstream metric omitted", log)
        self.assertEqual(result["cases"][0]["optional_omissions"], ["Total Stars Earned"])

    def test_empty_corpus_never_passes(self):
        result, _ = probe.record_run([], "https://mark.sylphx.com", lambda _: HEALTH, lambda: PROVENANCE)
        self.assertEqual(result["verdict"], "unknown")


if __name__ == "__main__":
    unittest.main()
