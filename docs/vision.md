# Mark vision

## Goal

Mark (repository `readme-mark`) makes README images from one URL: animated
banners, shields-compatible badges, typing text, tech icons, profile cards and
live GitHub stats cards. One free host, no token, no signup, always up. The
capabilities and their code are in [`capabilities.md`](capabilities.md); the
decision of record is [ADR-0005](adr/ADR-0005-readme-visuals-toolkit.md).

Switching is a host change. Mark reads the URL dialects people already have in
their READMEs (shields, skill-icons, readme-typing-svg, capsule-render,
github-readme-stats) and translates each into one render kernel. The canonical
host is `https://mark.sylphx.com`, and every URL once public there keeps
working.

## For whom

- README authors who want a polished profile or project README without
  learning six tools and without broken images when a free host runs out of
  quota.
- Maintainers who need project badges (version, stars, scores) and banners
  that match their brand.

## Principles

- Every default looks good; a bare URL renders a polished image.
- Unknown input normalizes; an upstream failure renders a calm fallback card
  with `200`, never a broken image.
- Static routes are pure functions of their URL and cache immutably. Only live
  routes read upstream, under the ADR-0005 network contract.
- User text is escaped, paint is validated tokens, and SVG responses carry
  `script-src 'none'`.

## Boundaries

- No accounts, saved marks, uploads, PNG or AI generation on the hot path.
- No user token required for anything; a server token only adds capacity.
- No personal or company names in theme or palette ids (brand icons a user
  picks are content).
- Every URL ever public on `mark.sylphx.com` keeps answering `200 image/svg+xml`.

## Target metrics

- In-process render p50 in microseconds (badge 8 µs, banner 20 µs; the Bench
  workflow re-measures on every change to `main`).
- Edge-cached `.svg` URLs answer in about 30 ms wait time (`cf-cache-status:
  HIT`), measured with `scripts/measure-live.sh`.
- A URL copied from shields, skill-icons, readme-typing-svg, capsule-render or
  github-readme-stats with only the host changed renders the equivalent image.

## Hosting

Mark is an ordinary Sylphx Apps tenant; this repository owns `sylphx.toml`.
Origin cache headers are this product's; edge caching of `.svg` URLs follows
Cloudflare's extension rule.
