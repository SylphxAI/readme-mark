#!/usr/bin/env python3
"""Mutation regressions for the capability evidence gate (no Rust build)."""

import importlib.util
import tempfile
import unittest
from pathlib import Path

spec = importlib.util.spec_from_file_location(
    "check_capabilities", Path(__file__).with_name("check-capabilities.py")
)
guard = importlib.util.module_from_spec(spec)
spec.loader.exec_module(guard)


class CapabilityGuardTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        for directory in ("docs", "src/interfaces/http", "tests", ".github/workflows"):
            (self.root / directory).mkdir(parents=True)
        self.docs = self.root / "docs/capabilities.md"
        self.docs.write_text(
            "| ID | Capability | Status | Code | Depends on | Tests | Routes |\n"
            "| --- | --- | --- | --- | --- | --- | --- |\n"
            "| MARK-TEST | Test | supported | `src/code.rs` | — | "
            "`tests/contract.rs::renders` | `/badge/{*tail}` |\n"
        )
        (self.root / "src/code.rs").write_text("// Implementation\n")
        (self.root / "tests/contract.rs").write_text(
            "#[tokio::test]\nasync fn renders() { assert!(true); }\n"
        )
        (self.root / "src/interfaces/http/mod.rs").write_text(
            'Router::new().route(\n "/badge/{*tail}", get(handler))\n'
        )
        (self.root / ".github/workflows/ci.yml").write_text(
            "run: cargo test --locked\n"
        )

    def replace(self, old, new):
        self.docs.write_text(self.docs.read_text().replace(old, new))

    def test_valid_evidence(self):
        self.assertEqual(guard.check(self.root), [])

    def test_missing_code_path(self):
        self.replace("src/code.rs", "src/missing.rs")
        self.assertIn("missing code path", "\n".join(guard.check(self.root)))

    def test_unlisted_route_family(self):
        router = self.root / "src/interfaces/http/mod.rs"
        router.write_text(router.read_text() + '.route("/new/{id}", get(handler))')
        self.assertIn("undocumented route: /new/{id}", guard.check(self.root))

    def test_deleted_route(self):
        self.replace("/badge/{*tail}", "/removed")
        self.assertIn("documented route does not exist: /removed", guard.check(self.root))

    def test_supported_without_test(self):
        self.replace("`tests/contract.rs::renders`", "—")
        self.assertIn("supported status needs test evidence", "\n".join(guard.check(self.root)))

    def test_missing_test_file(self):
        (self.root / "tests/contract.rs").unlink()
        self.assertIn("missing test file", "\n".join(guard.check(self.root)))

    def test_helper_is_not_test_evidence(self):
        (self.root / "tests/contract.rs").write_text("fn renders() {}\n")
        self.assertIn("missing Rust test", "\n".join(guard.check(self.root)))

    def test_python_gate_must_run_in_ci(self):
        (self.root / "scripts").mkdir()
        (self.root / "scripts/check-host.py").write_text("raise SystemExit(0)\n")
        self.replace("tests/contract.rs::renders", "scripts/check-host.py")
        self.assertIn("test gate is not run in CI", "\n".join(guard.check(self.root)))
        (self.root / ".github/workflows/ci.yml").write_text(
            "run: python3 scripts/check-host.py\n"
        )
        self.assertEqual(guard.check(self.root), [])

    def test_empty_inventory(self):
        self.docs.write_text("# Capabilities\n")
        self.assertIn("missing capability table", guard.check(self.root))


if __name__ == "__main__":
    unittest.main()
