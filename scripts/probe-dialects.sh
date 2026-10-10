#!/usr/bin/env bash
# Usage: scripts/probe-dialects.sh [origin]; requires curl and Python 3 only.
# Reuses tests/corpus/readme-urls.json, prints every case and dialect summary,
# and exits nonzero for any transport/status/type/geometry/text/fallback failure.
set -euo pipefail
exec python3 -I "$(dirname "${BASH_SOURCE[0]}")/probe-dialects.py" "$@"
