# ADR-0005: Mark becomes the free README-visuals toolkit (readme-mark)

- Status: accepted
- Date: 2026-09-25
- Authority: the owner's open-source star program (goal: GitHub stars) granted
  this repository full autonomy, including overriding earlier vision and
  non-goals where they block that goal. Amends ADR-0003 (one grammar, no live
  data) and ADR-0004 (neutral catalog). Supersedes the "Not doing" and
  "Non-goals" lists they fed into `docs/vision.md`.

## Context

Mark renders good SVG, but almost nobody can find it or switch to it:

- **The name is unsearchable.** "mark" collides with markdown, marker, and
  every watermark tool. Nobody searching "readme banner" or "readme badge"
  lands here.
- **It covers only part of the market.** People building a README reach for
  six tools: shields.io (badges, ~26k stars), anuraghazra/github-readme-stats
  (stats cards, ~70k), kyechan99/capsule-render (animated banners, ~5k),
  DenverCoder1/readme-typing-svg (typing text), tandpfun/skill-icons (tech
  icons, ~9k), and the trophy and streak cards. Mark matched the banner and a
  subset of the badge. The largest category, live GitHub stats, was a
  deliberate non-goal.
- **Switching costs a rewrite.** Each of those tools has its own URL dialect
  that already sits in hundreds of thousands of READMEs. Mark only spoke its
  own grammar, so moving over meant rewriting every URL.
- **The incumbents' main pain is availability.** Users of those tools mostly
  complain about the public instances: github-readme-stats cards failing on
  rate limits and paused hosting, capsule-render and the typing SVG going
  down with their free hosting, and self-hosting being the only fix offered.
  A fast, cached host that stays up is a real advantage.

## Decision

1. **Position.** "Beautiful README images from one URL": banners, badges,
   typing text, tech icons, and GitHub stats cards in one free host with no
   token and no signup. The goal is to be the best free README-visuals
   toolkit.
2. **Name.** The repository becomes `SylphxAI/readme-mark`, which is
   keyword-rich and keeps the product's name. The product is still "Mark" in
   prose. `readme-mark` has no significant collision on GitHub and is free on
   npm. GitHub redirects the old repository URL.
3. **Existing URLs keep working forever.** `https://mark.sylphx.com` stays the
   canonical host. Every URL that was public before this ADR must keep
   answering `200 image/svg+xml`. The `legacy-*` snapshot cases in
   `tests/snapshots/` enforce this. Their bytes may improve, but their status
   and type may not change. No new host is introduced now. A future vanity
   host can only be added alongside this one.
4. **Drop-in dialects.** Mark accepts the URL dialects of the tools people
   already use, so switching only means changing the host:
   shields static badges (`/badge/...`, `/static/v1`), skill-icons
   (`/icons?i=`), readme-typing-svg (`/?lines=`, `/typing`), capsule-render
   (`/api?type=`), and github-readme-stats (`/api?username=`,
   `/api/top-langs`, `/api/pin`). Each dialect is a parser at the interface
   edge that translates into the render kernel. There is still one render
   authority. What changes from ADR-0003 is that there is no longer one
   accepted spelling. The capsule-render typography knobs ADR-0003 retired
   (`fontSize`, `fontColor`, alignment, `section`, `reversal`, `stroke`,
   `textBg`, `rotate`) come back only inside the capsule dialect module.
5. **Live GitHub data is a capability again (`MARK-LIVE`).** It replaces
   `MARK-STATS dead`, under this network contract:
   - Live data runs only on live routes. Every other route stays a pure
     function of its URL, with immutable caching and no clock.
   - Upstream calls are bounded: a timeout on every call, an in-memory TTL
     cache with request coalescing, and stale-on-error serving.
   - Nobody needs a token. An operator may give the server a GitHub token
     through a platform secret to raise the rate limit. Without one, the
     service uses unauthenticated public endpoints and leans on caching.
   - A failure still renders a card. When upstream fails and nothing is
     cached, the route answers `200` with a calm "temporarily unavailable"
     card and a short cache, never a broken image.
   - Responses cache for hours, not forever:
     `s-maxage` plus `stale-while-revalidate` and `stale-if-error`.
   Only the live capability reads the clock, for TTLs.
6. **Brand icons are content.** Simple Icons (CC0 path data) supplies
   thousands of brand glyphs for badge logos and tech-icon tiles. A user
   choosing `logo=rust` is content, just like `text=`. The rule from
   ADR-0004 is unchanged: theme and palette ids stay neutral.
7. **Edge cacheability is part of the product.** Every image route also
   answers with a `.svg` suffix (for example `/badge/a-b-c.svg` or
   `/api/v1/mark/hero.svg?...`). Cloudflare caches `.svg` by extension, so
   those URLs get an edge `HIT` without a platform cache rule. Render p50
   and cacheability numbers are published in the README.

## Consequences

- `docs/vision.md`, `docs/capabilities.md`, and `AGENTS.md`
  hazards are rewritten to match.
- `MARK-STATS` becomes `rename-to:MARK-LIVE`. The new identities are
  `MARK-DIALECTS`, `MARK-ICONS`, `MARK-TYPING`, and `MARK-LIVE`.
- The structural gates keep working. The dialect and live modules are
  allowed exactly the exceptions stated above, and nothing more.
- Visual regressions are caught by byte snapshots
  (`tests/visual_snapshots.rs`).

## Amendment (2026-09-25, owner)

- **Clean cut on pre-launch URLs.** Mark was never promoted, and the only
  embeds were our own. Routes retired before this ADR (`/api/v1/banner`,
  `/api/v1/deploy`, and the other legacy capability routes) stay retired.
  Every internal README and site that used them was migrated to
  `/api/v1/mark/*` in the same program: 24 repositories plus the
  `SylphxAI/.github` org profile. From this release on, decision 3 applies
  in full: a URL that works today keeps working.
- **Name and host.** The repository is `SylphxAI/readme-mark`. The host
  stays `https://mark.sylphx.com`: it is free, short, matches the name, and
  already sits behind the edge cache. A dedicated domain could be added
  later alongside it.
- **Live cards are a priority.** Stats, streak, top languages, repo pins,
  and trophies are free for everyone, with no user token. A server-side
  GitHub token in the platform secret `GITHUB_TOKEN` raises capacity when
  set. Without it, the anonymous path (REST plus the public contributions
  calendar) runs behind the cache.
