#!/usr/bin/env python3
"""Offline Bench status regression: run the real step with an inert cargo stub.

Run: python3 scripts/check-bench-workflow.py
No Rust compilation, network access or benchmark measurement is performed.
"""

from __future__ import annotations

import os
from pathlib import Path
import re
import subprocess
import tempfile
import textwrap
import unittest

ROOT = Path(__file__).resolve().parent.parent


class BenchWorkflow(unittest.TestCase):
    def setUp(self) -> None:
        workflow = (ROOT / ".github/workflows/bench.yml").read_text()
        step = workflow.split("      - name: Render latency (release, in-process)\n", 1)[1]
        self.assertRegex(step, r"(?m)^        shell: bash$")
        block = re.search(r"(?m)^        run: \|\n((?:          .*\n|\n)+)", step)
        self.assertIsNotNone(block, "benchmark step must have an inline run block")
        self.script = textwrap.dedent(block.group(1))

    def run_step(self, output: str, status: int, *, grep_error: bool = False,
                 summary_error: bool = False) -> tuple[subprocess.CompletedProcess, str]:
        with tempfile.TemporaryDirectory(prefix="mark-bench-status-") as directory:
            root = Path(directory)
            cargo = root / "cargo"
            cargo.write_text('#!/bin/bash\nprintf "%s" "$STUB_OUTPUT"\nexit "$STUB_STATUS"\n')
            cargo.chmod(0o755)
            if grep_error:
                grep = root / "grep"
                grep.write_text("#!/bin/bash\nexit 2\n")
                grep.chmod(0o755)
            summary = root / "summary"
            if summary_error:
                summary.mkdir()
            else:
                summary.write_text("existing summary\n")
            env = {**os.environ, "PATH": f"{root}:{os.environ['PATH']}",
                   "STUB_OUTPUT": output, "STUB_STATUS": str(status),
                   "GITHUB_STEP_SUMMARY": str(summary), "TMPDIR": str(root)}
            # Explicit shell: bash in Actions invokes bash --noprofile --norc -e -o pipefail.
            result = subprocess.run(
                ["bash", "--noprofile", "--norc", "-e", "-o", "pipefail", "-c", self.script],
                env=env, cwd=root, capture_output=True, text=True, check=False,
            )
            self.assertEqual(list(root.glob("tmp.*")), [], "temporary output must be cleaned up")
            return result, "" if summary_error else summary.read_text()

    def test_failed_producer_does_not_publish_partial_table(self) -> None:
        result, summary = self.run_step("diagnostic\n| partial table |\n", 23)
        self.assertEqual(result.returncode, 23)
        self.assertIn("diagnostic", result.stdout)
        self.assertEqual(summary, "existing summary\n")

    def test_success_publishes_only_table_rows(self) -> None:
        result, summary = self.run_step("diagnostic\n| table |\n", 0)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(summary, "existing summary\n| table |\n")

    def test_success_without_table_is_intentionally_harmless(self) -> None:
        result, summary = self.run_step("no table rows\n", 0)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(summary, "existing summary\n")

    def test_grep_error_fails(self) -> None:
        result, summary = self.run_step("| table |\n", 0, grep_error=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(summary, "existing summary\n")

    def test_summary_write_error_fails(self) -> None:
        result, _ = self.run_step("| table |\n", 0, summary_error=True)
        self.assertNotEqual(result.returncode, 0)


if __name__ == "__main__":
    unittest.main()
