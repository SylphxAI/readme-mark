#!/usr/bin/env python3
"""Config and vocabulary parity gate.

Two kinds of deliberately repeated declaration are checked mechanically:

1. **Defaults** — the service port, bind host, credit default, and public base
   URL are declared in the process defaults, the local env template, the
   container image, and the Apps service spec. A silent divergence is a
   deploy-time surprise, so the values are checked against each other.
2. **Form vocabulary** — the server's `MarkForm::parse` and the studio page's
   JS `parseForm` mirror the same form ids (the browser must decide a form
   without a round trip). Divergence would make the composer accept ids the
   render does not, or vice versa.

Run: `python3 scripts/check-config-parity.py`
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def dotenv(path: Path) -> dict[str, str]:
    values: dict[str, str] = {}
    for line in path.read_text().splitlines():
        line = line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        key, value = line.split("=", 1)
        values[key.strip()] = value.strip()
    return values


def dockerfile_env(path: Path) -> dict[str, str]:
    text = path.read_text().replace("\\\n", " ")
    values: dict[str, str] = {}
    for line in text.splitlines():
        if not line.startswith("ENV "):
            continue
        for key, value in re.findall(r"([A-Z_][A-Z0-9_]*)=([^\s]+)", line[4:]):
            values[key] = value.strip('"')
    return values


def sylphx_env(path: Path) -> dict[str, str]:
    text = path.read_text()
    values: dict[str, str] = {}
    for match in re.finditer(r"\[env\.(?P<key>[A-Z_][A-Z0-9_]*)\]\s*\nvalue\s*=\s*\"(?P<value>[^\"]*)\"", text):
        values[match.group("key")] = match.group("value")
    # PORT belongs to the `web` service (sidecars may declare their own).
    web = re.search(
        r'\[\[services\]\](?:(?!\[\[services\]\]).)*?name\s*=\s*"web"(?:(?!\[\[services\]\]).)*?'
        r"^port\s*=\s*(\d+)",
        text,
        re.M | re.S,
    )
    if web:
        values["PORT"] = web.group(1)
    return values


def normalise(key: str, value: str) -> str:
    """Booleans compare by meaning: `false`/`true` are `0`/`1` for the flag."""
    if key != "DEFAULT_CREDIT":
        return value
    return {"false": "0", "true": "1", "no": "0", "yes": "1"}.get(value.lower(), value)


def process_defaults(path: Path) -> dict[str, str]:
    text = path.read_text()
    values: dict[str, str] = {}
    port = re.search(r'var\("PORT"\).*?unwrap_or\((\d+)\)', text, re.S)
    if port:
        values["PORT"] = port.group(1)
    host = re.search(r'var\("HOST"\)\.unwrap_or_else\(\|_\| "([^"]+)"', text)
    if host:
        values["HOST"] = host.group(1)
    credit = re.search(r'var\("DEFAULT_CREDIT"\).*?unwrap_or\((\w+)\)', text, re.S)
    if credit:
        values["DEFAULT_CREDIT"] = "0" if credit.group(1) == "false" else "1"
    return values


FORMS = ("hero", "pill", "strip", "profile", "identity", "deploy")


def server_forms(path: Path) -> set[str]:
    """Ids `MarkForm::parse` accepts: its explicit branches plus the hero default."""
    text = path.read_text()
    parse = text[text.index("pub fn parse(raw") : text.index("/// Hero geometry")]
    return set(re.findall(r'Some\("([a-z]+)"\)', parse)) | {"hero"}


def studio_forms(path: Path) -> set[str]:
    text = path.read_text()
    start = text.index("function parseForm(raw){")
    body = text[start : text.index("function decodeText", start)]
    found = set(re.findall(r's === "([a-z]+)"', body))
    fallback = re.search(r'return "([a-z]+)";\n\}', body)
    if fallback:
        found.add(fallback.group(1))
    return found


def form_vocabulary(workspace: Path) -> list[str]:
    server = server_forms(workspace / "src" / "capabilities" / "mark" / "domain" / "spec.rs")
    studio = studio_forms(workspace / "templates" / "index.html")
    findings: list[str] = []
    if server != studio:
        findings.append(
            "form vocabulary: server "
            f"{sorted(server)} != studio {sorted(studio)} "
            f"(server-only {sorted(server - studio)}, studio-only {sorted(studio - server)})"
        )
    missing = set(FORMS) - server
    if missing:
        findings.append(f"form vocabulary: expected ids missing from the server: {sorted(missing)}")
    return findings


def image_inputs(workspace: Path) -> list[str]:
    """Every compile-time input directory reaches the image build context.

    CI builds with the whole checkout, the platform builds the Dockerfile: an
    `include_str!` of a directory the Dockerfile never copies passes CI and
    fails the production image build.
    """
    docker = (workspace / "Dockerfile").read_text()
    copied = set(re.findall(r"^COPY\s+(?!--from)(\S+)", docker, re.M))
    copied = {c.rstrip("/").split("/")[0] for c in copied}
    watch = (workspace / "sylphx.toml").read_text()
    findings: list[str] = []
    for rs in sorted((workspace / "src").rglob("*.rs")):
        for rel in re.findall(r'include_(?:str|bytes)!\("([^"]+)"\)', rs.read_text()):
            target = (rs.parent / rel).resolve().relative_to(workspace.resolve())
            top = target.parts[0]
            if top not in copied:
                findings.append(f"image inputs: {rs.relative_to(workspace)} includes {target}, but the Dockerfile never copies {top}/")
            if f'"{top}/**"' not in watch:
                findings.append(f"image inputs: sylphx.toml watch_paths lacks \"{top}/**\" (a change there would not redeploy)")
    return findings


def main() -> int:
    sources = {
        "process defaults": process_defaults(ROOT / "src" / "bootstrap.rs"),
        "Dockerfile": dockerfile_env(ROOT / "Dockerfile"),
        ".env.example": dotenv(ROOT / ".env.example"),
        "sylphx.toml": sylphx_env(ROOT / "sylphx.toml"),
    }
    required = {
        "PORT": ["process defaults", "Dockerfile", ".env.example", "sylphx.toml"],
        "HOST": ["process defaults", "Dockerfile", ".env.example"],
        "DEFAULT_CREDIT": ["process defaults", "Dockerfile", ".env.example", "sylphx.toml"],
        "PUBLIC_BASE_URL": [".env.example", "sylphx.toml"],
    }
    findings: list[str] = []
    for key, homes in required.items():
        seen: dict[str, list[str]] = {}
        for home in homes:
            raw = sources[home].get(key)
            value = normalise(key, raw) if raw is not None else None
            if value is None:
                findings.append(f"{key}: not declared in {home}")
                continue
            seen.setdefault(value, []).append(home)
        if len(seen) > 1:
            detail = "; ".join(f"{value} ({', '.join(homes)})" for value, homes in seen.items())
            findings.append(f"{key}: defaults disagree: {detail}")

    findings += form_vocabulary(ROOT)
    findings += image_inputs(ROOT)

    if findings:
        for finding in findings:
            print(f"FAIL {finding}", file=sys.stderr)
        print("config parity contract failed", file=sys.stderr)
        return 1
    summary = ", ".join(f"{key}={sources[homes[0]].get(key)}" for key, homes in required.items())
    print(
        f"OK: config defaults agree across {len(sources)} sources ({summary}); "
        f"server and studio form vocabularies match ({len(FORMS)} ids)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
