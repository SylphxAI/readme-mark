# ADR-0003: The one grammar — north-star end state for Mark

- Status: accepted
- Date: 2026-08-09
- Authority: product repository decision implementing binding `engineering-standard`; supersedes the capability list in ADR-0001 and the surface clause in ADR-0002.

## Context

ADR-0001 defined six capabilities (banner, badge, github_card, icon_row,
brand_kit, deploy_mark). ADR-0002 delivered the clean-break hardening
(security grammar, bounded inputs, canonical surface, deploy identity).

The north-star review found the product was still **five dialects of one
sentence**: banner/badge/icon_row/brand_kit/deploy_mark are all "URL params →
a small branded SVG" from one chromatic kernel, split into separate bounded
contexts that cannot compose. Meanwhile github_card — the only network/state/
secret system — was the weakest art (fixed palette, no chromatic system) and
the only surface with failure modes, staleness, and rate limits; it fought
specialist hosts on their turf while poisoning the reliability moat
("a Mark that can fail because an upstream 429'd is a Mark that can fail").
Time-sampled fills (`timeAuto`/`timeGradient`) were the only remaining
non-URL input, breaking "same URL → same mark, forever".

## Decision

The north-star end state: **one concept (the Mark), one grammar, one surface,
one kernel, zero state.**

1. **One capability `mark`.** `src/capabilities/mark/` owns the whole product:
   - `domain/` — `spec.rs` (MarkSpec: form × art × paint × geometry × text ×
     motion), `catalog.rs` (the vocabulary + limits), `pill.rs`, `icons.rs`,
     `motion.rs`, `shapes.rs` (the 42 art types), and the kernel
     (`color.rs`, `theme.rs`, `svg.rs` — moved out of the retired `src/shared`).
   - `application/` — pure renderers per form (`hero`, `pill`, `strip`,
     `identity`, `deploy`) + one dispatcher.
   - `interfaces/` — the one HTTP surface.
2. **One grammar:** `mark = form × art × paint × geometry × text × motion`.
   Forms: `hero` (flagship), `pill` (atomic status), `strip` (tech identity),
   `identity` (fleet brand), `deploy` (conversion). Art is a property of hero
   and identity; motion composes at text level onto pill/identity and at row
   level onto strip; width scales identity. The cross product is the product.
3. **One surface:** `GET /api/v1/mark/{form}` + shields-style
   `/badge/{label}-{message}-{color}` (pill shorthand). All legacy capability
   routes are deleted and 404.
4. **Retire live data:** `github_card` (stats/org/repo), the HTTP adapter,
   caches, token handling, and error cards are deleted. `reqwest` and `moka`
   dependencies are removed. Specialist hosts own data.
5. **Retire the clock:** `timeAuto`/`timeGradient`/`clock_seed`/
   `current_time_seed` are deleted; `chrono` is removed. `resolve_fill` is
   deterministic. Same URL renders the same SVG forever.
6. **Totality:** rendering never fails. Unknown form → hero, unknown art →
   `aurora`, invalid colors → fallback paint. The error-SVG path is deleted.
7. **Studio as the grammar:** `templates/index.html` is rewritten palette-first
   and form-first around the one grammar (302 lines, was 1110), consuming the
   same `/api/v1/catalog` vocabulary the API exposes.
8. **Config:** `AppState` is `{default_credit, public_base}` — no upstream, no
   secrets. `GITHUB_TOKEN` removed from `.env.example`, `sylphx.toml`,
   README, AGENTS.md.

## Consequences

- Breaking: every legacy path and capability route 404s; the studio, README,
  and all examples use the single grammar; GitHub cards no longer exist.
- The reliability moat becomes absolute: no upstream, no clock, no state, no
  failure modes — every Mark URL is immutable, cacheable, and brand-bearing.
- Regression guards: architecture boundary tests (single capability, no
  clock/upstream deps, retired paths absent, catalog↔shapes parity, single
  surface), smoke tests for all five forms and all 42 art types, composition
  tests (pill + motion, identity + art + width), determinism tests,
  injection/escaping/limits contracts, HTTP 404 contracts for every retired
  route.

## Evidence

- `cargo test --locked` — 54 tests green
- `cargo clippy --all-targets -- -D warnings`
- Live verification: revision readback matches the landed SHA; all five forms
  serve on `/api/v1/mark`; every retired route 404s; injection inert; same
  URL returns byte-identical SVG; limits and CSP enforced.
