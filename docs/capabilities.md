# Mark capabilities

Decision: [ADR-0005](adr/ADR-0005-readme-visuals-toolkit.md). Each capability row states what it does; its tests and the live host measure it.

| ID | Capability | Status | Code | Depends on |
| --- | --- | --- | --- | --- |
| MARK-GRAMMAR | Native render grammar: `GET /api/v1/mark/{form}` renders a deterministic SVG for every form in `/api/v1/catalog`; unknown input normalizes (unknown form → hero, unknown art → `waving`, unknown theme/layout/animation → default). | supported | `src/capabilities/mark/domain/spec.rs` | — |
| MARK-FOREVER | Public URLs never break: Every `tests/snapshots/legacy-*.url` answers `200 image/svg+xml` on the live host. | supported | `tests/visual_snapshots.rs` | MARK-GRAMMAR |
| MARK-BADGE | Shields-compatible static badge: `/badge/{label}-{message}-{color}` (and `{message}-{color}`) and `/static/v1?label&message&color` honor shields escaping (`--`, `__`, `_`), `style` (`flat` `flat-square` `plastic` `for-the-badge` `social`), `logo`, `logoColor`, `labelColor`, `color` (shields names, hex, CSS names, `rgb()`/`hsl()`), and the `.svg` suffix, with badge-maker geometry and Verdana widths; `/api/v1/mark/score?label&value&max` renders a graded score pill. | supported | `src/capabilities/mark/domain/shields.rs` | MARK-GRAMMAR, MARK-ICONS |
| MARK-DIALECTS | Drop-in URL dialects: A URL written for shields, skill-icons, readme-typing-svg, capsule-render, or github-readme-stats renders the equivalent image when only the host changes. | supported | `src/capabilities/mark/interfaces/dialects/mod.rs` | MARK-GRAMMAR |
| MARK-ICONS | Brand icon set: Simple Icons slugs (thousands) plus short aliases render as badge logos and as tech-icon tiles. Every skill-icons id resolves (`/icons?i=` with skillicons.dev geometry, `theme`/`t`, `perline`, `i=all`), and the strip form paints any Simple Icons slug or title. | supported | `src/capabilities/mark/domain/icons.rs` | — |
| MARK-TYPING | Typing-text SVG: `/typing?lines=…` renders an animated typing SVG that works inside `<img>`. | supported | `src/capabilities/mark/domain/typing.rs` | MARK-GRAMMAR |
| MARK-LIVE | Live GitHub data cards and badges: Stats, top-languages, streak, repo, and star-history cards plus dynamic badges (GitHub, Actions workflow status, npm, pub.dev, Packagist, Bundlephobia, Chrome Web Store) render from public upstream data with no user token, a bounded cached upstream, stale-on-error, and a `200` fallback card. | supported | `src/capabilities/live/mod.rs` | MARK-GRAMMAR |
| MARK-SVG | Valid SVG + XSS-safe paint: SVG is well-formed, user text is escaped, non-canonical paint falls back, and responses carry `Content-Security-Policy: script-src 'none'` plus `X-Content-Type-Options: nosniff`. | supported | `src/capabilities/mark/domain/svg.rs` | MARK-GRAMMAR |
| MARK-CDN | Cacheable responses: Static routes send immutable long cache + strong ETag + `304`; live routes send hour-scale `s-maxage` with `stale-while-revalidate`/`stale-if-error`; every image route also answers with a `.svg` suffix for edge caching. | supported | `src/interfaces/http/response.rs` | MARK-GRAMMAR |
| MARK-CATALOG | Public vocabulary: `/api/v1/catalog` publishes forms, art, layouts, themes, icons, fonts, and limits that the render honors. Theme ids stay neutral. | supported | `src/interfaces/http/catalog.rs` | MARK-GRAMMAR |
| MARK-STUDIO | Composer at `/`: `/` offers live preview, presets, copy URL / markdown / HTML, and recovers state from a pasted URL. | supported | `src/interfaces/http/studio.rs` | MARK-CATALOG |
| MARK-HOST | Canonical host: `https://mark.sylphx.com` serves the product. | supported | `sylphx.toml` | MARK-GRAMMAR |
| MARK-PROFILE | Text-driven profile card: `/api/v1/mark/profile?text&desc` renders name and tagline from the URL; `identity` maps here. | supported | `src/capabilities/mark/application/profile.rs` | MARK-GRAMMAR |
| MARK-DEPLOY | Conversion mark: `/api/v1/mark/deploy?service=…` renders the "deployed on Sylphx" pill. | supported | `src/capabilities/mark/application/deploy.rs` | MARK-GRAMMAR |

## Operations

- Public probe: anonymous `GET https://mark.sylphx.com/badge/build-passing-brightgreen` and one URL per dialect return SVG. `/health.revision` is deploy proof, not product proof.
- The service has no database and no persistence. Live routes read public GitHub and npm APIs through a bounded in-memory cache.
