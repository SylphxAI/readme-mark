# Mark — local agent notes

Doctrine and Mission Control are retired historical lineage and must not
be loaded as current instruction authority.

This file is local commands/hazards only.

## Local commands

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test
cargo test --test public_contract
python3 scripts/check-source-hygiene.py
python3 scripts/check-duplication.py
python3 scripts/check-config-parity.py
python3 scripts/check-module-budget.py
python3 scripts/check-owned-runner-profiles.sh
python3 brand/build.py --check
cargo run
cargo build --release
```

Env: see `.env.example` (`PORT`, `HOST`, `PUBLIC_BASE_URL`, `DEFAULT_CREDIT`, `RUST_LOG`;
optional `GITHUB_TOKEN`/`GITHUB_TOKENS` for live cards).

## Hazards

- Decision of record is ADR-0005 (`docs/adr/`): README-visuals toolkit,
  drop-in dialects, live GitHub data under a network contract. ADR-0003/0004
  are history where they conflict.
- **URLs are forever.** Anything once public on `mark.sylphx.com` must keep
  answering `200 image/svg+xml`. Add a `tests/snapshots/legacy-*.url` case
  when you touch a public route; never delete one. `contract-*` cases are URL
  shapes other products emit (repomap's `/badge/agent--ready-N%2F100-color`):
  changing them needs the consumer released in the same crossing.
- Static routes are pure functions of the URL (no clock, no upstream, immutable
  cache). Only the live capability may read upstream or a clock, bounded by
  timeouts, cache, stale-on-error, and a `200` fallback card.
- Never require a user token. A server token is optional capacity.
- Stateless SVG only on the hot path — no headless browser or AI generation.
- Soft watermark via `credit` stays opt-in.
- Brand assets: the masters live in `brand/` (usage sheet `brand/README.md`);
  `static/favicon.svg` and `static/og.png` are generated copies, so change the
  master and run `python3 brand/build.py` instead of editing them.
- SVG attribute values come only from validated tokens or static strings; all
  user text is escaped.
- Deploy identity: the image embeds the git revision (`SYLPHX_GIT_COMMIT_SHA` /
  `SYLPHX_GIT_SHA`, then `SOURCE_COMMIT` / `GIT_SHA`); `mark --version` fails
  the image build without one. `/health.revision` is deploy proof.
- Rendering never fails: unknown input normalizes to documented defaults.
- Visual changes: review, then `UPDATE_SNAPSHOTS=1 cargo test --test visual_snapshots`.
- Heavy work (full matrices, big render sweeps, benchmarks) runs on CI, not the
  shared desk host.
