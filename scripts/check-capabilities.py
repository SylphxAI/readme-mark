#!/usr/bin/env python3
"""Check capability code/test evidence and the explicit HTTP route inventory.

This checks evidence references, not test outcomes: CI runs the referenced Rust
suite and Python gates. Routes use the router's literal Axum paths, so adding a
route requires assigning it to a capability (including pages and operations).
"""

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def tokens(cell: str) -> list[str]:
    return re.findall(r"`([^`]+)`", cell)


def check(root: Path) -> list[str]:
    findings = []
    text = (root / "docs/capabilities.md").read_text()
    header = "| ID | Capability | Status | Code | Depends on | Tests | Routes |"
    if header not in text:
        return ["missing capability table"]
    rows = text.split(header, 1)[1].strip().splitlines()[1:]
    documented = set()
    count = 0
    ci = (root / ".github/workflows/ci.yml").read_text()
    for line in rows:
        if not line.startswith("|"):
            break
        cells = [cell.strip() for cell in line.strip("|").split("|")]
        if len(cells) != 7:
            findings.append(f"malformed capability row: {line}")
            continue
        identifier, _, status, code, _, tests, routes = cells
        count += 1
        paths = tokens(code)
        if not paths:
            findings.append(f"{identifier}: missing code path")
        for path in paths:
            if not (root / path).is_file():
                findings.append(f"{identifier}: missing code path: {path}")
        evidence = tokens(tests)
        if status == "supported" and not evidence:
            findings.append(f"{identifier}: supported status needs test evidence")
        for reference in evidence:
            path, separator, name = reference.partition("::")
            file = root / path
            if not file.is_file():
                findings.append(f"{identifier}: missing test file: {path}")
                continue
            if separator and path.endswith(".rs"):
                source = re.sub(r"//[^\n]*", "", file.read_text())
                pattern = (
                    r"#\[(?:tokio::)?test\]\s*(?:#\[[^\]]+\]\s*)*"
                    r"(?:async\s+)?fn\s+" + re.escape(name) + r"\s*\("
                )
                if not re.search(pattern, source):
                    findings.append(f"{identifier}: missing Rust test: {reference}")
            elif not separator and path.endswith(".py"):
                if not re.search(r"run:\s*python3\s+" + re.escape(path) + r"\s*(?:\n|$)", ci):
                    findings.append(f"{identifier}: test gate is not run in CI: {path}")
            else:
                findings.append(f"{identifier}: invalid test reference: {reference}")
        documented.update(tokens(routes))
    if not count:
        findings.append("empty capability table")
    # Route declarations live in this composition root; ignore line comments.
    router = re.sub(r"//[^\n]*", "", (root / "src/interfaces/http/mod.rs").read_text())
    actual = set(re.findall(r'\.route\s*\(\s*"([^"]+)"', router))
    for route in sorted(actual - documented):
        findings.append(f"undocumented route: {route}")
    for route in sorted(documented - actual):
        findings.append(f"documented route does not exist: {route}")
    return findings


def main() -> int:
    findings = check(ROOT)
    if findings:
        for finding in findings:
            print(f"FAIL {finding}", file=sys.stderr)
        return 1
    print("OK: capability code paths, test references and HTTP routes agree")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
