# Mark

Mark (`mark.sylphx.com`) turns one URL into a README image: banners, badges,
typing text, tech icons, GitHub cards. It is used by dropping a URL into a
README, so the product is its URLs: they must render fast, safely, forever.
Goal and boundaries: [docs/vision.md](docs/vision.md). Capabilities and code
paths: [docs/capabilities.md](docs/capabilities.md). Decision of record:
[ADR-0005](docs/adr/ADR-0005-readme-visuals-toolkit.md). Company standards:
<https://github.com/SylphxAI/owner/tree/main/standards>.

## Hard lines

- Every URL once public on `mark.sylphx.com` keeps answering `200 image/svg+xml`,
  because it sits in other people's READMEs. Add a `tests/snapshots/legacy-*.url`
  case when you touch a public route and never delete one. `contract-*` cases
  are URL shapes other products emit (repomap's `/badge/agent--ready-N%2F100-color`);
  change one only in the same release as its consumer.
- All user text is escaped and SVG attribute values come only from validated
  tokens or static strings, since responses are embedded on third-party pages.
- Static routes are pure functions of the URL (no clock, no upstream, immutable
  cache); only the live capability reads upstream or a clock, bounded by
  timeouts, cache, stale-on-error and a `200` fallback card. Never require a
  user token; a server `GITHUB_TOKEN` is optional capacity.
- Rendering never fails: unknown input normalizes to documented defaults.
- Stateless SVG only on the hot path: no headless browser, no AI generation.
- Brand assets are generated from the masters in `brand/`
  ([usage sheet](brand/README.md)); `static/favicon.svg` and `static/og.png` are
  copies, so change the master and run the pinned shared generator ([usage](brand/README.md)).

## Judged by

CI (`.github/workflows/ci.yml`) is the gate: `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings`, `cargo test --locked` (includes
`public_contract` and `visual_snapshots`), `scripts/check-source-hygiene.py`,
`check-duplication.py`, `check-config-parity.py`, `check-module-budget.py`,
`bash scripts/check-owned-runner-profiles.sh`,
`python3 scripts/check-bench-workflow.py` (offline, inert cargo stub), and the
pinned shared brand check ([usage](brand/README.md)).
After a reviewed visual change, refresh with
`UPDATE_SNAPSHOTS=1 cargo test --test visual_snapshots`.

Performance is judged by the Bench workflow (`.github/workflows/bench.yml`) and
`scripts/measure-live.sh`. Deploy is proved by `/health.revision`, which needs
the git revision embedded in the image (`SYLPHX_GIT_COMMIT_SHA` /
`SYLPHX_GIT_SHA`, then `SOURCE_COMMIT` / `GIT_SHA`; `mark --version` fails the
image build without one). Configuration: `.env.example`.

Full matrices, render sweeps and benchmarks run on CI, not the shared desk host.
