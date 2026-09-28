# Mark brand

`brand/` is the source of truth for Mark's mark, icons, colours and type:
surfaces copy from it and never redraw it, and every generated file is
rebuilt from the masters with `python3 brand/build.py` (needs pillow,
resvg-py and numpy; `python3 brand/build.py --check` verifies the hashes and
the surface copies with the standard library alone).

## Name

- Running text: **Mark** — capital M, one word. Not "MARK", not "the Mark
  service". `docs/vision.md` ("Mark (repository `readme-mark`) is the free
  README-visuals toolkit") and `PROJECT.md` ("# Mark (readme-mark)") are the
  source.
- Repository: `SylphxAI/readme-mark`. Canonical host: `https://mark.sylphx.com`
  (ADR-0005 decision 3: the host never changes and every URL that was public
  there stays valid).
- Deployed-service name: the API index and the server's own startup lines say
  **Sylphx Mark** (`src/interfaces/http/catalog.rs`, `src/bootstrap.rs`).
  Prose, the repository and the host say Mark.
- Operator line: "Mark is run by Sylphx Limited, registered in England and
  Wales, company number 16438428. Registered office: 128 City Road, London
  EC1V 2NX." (`static/index.html` footer, PR #87). Sylphx Limited is the
  operator and is never part of the brand.
- Local-script names: none. The product ships English only and every public
  URL is ASCII.
- Capitals in the logo: none. The logo is the M alone; the product has no
  wordmark, so no casing lives in an image — "Mark" is always set in the
  surrounding UI's type.

## Files

| Need | File |
|---|---|
| The mark as an icon (tile + M) | `svg/mark-app-icon.svg` |
| The M alone, light background | `svg/mark-symbol.svg` |
| The M alone, dark background | `svg/mark-symbol-on-dark.svg` |
| One-colour reproduction (print, engraving, stamps) | `svg/mark-symbol-black.svg`, `svg/mark-symbol-white.svg` |
| Maskable icon and Apple touch icon | `svg/mark-maskable.svg` |
| Browser tab | `favicon/favicon.svg` (32 px grid), `favicon/favicon.ico` (16, 32, 48) |
| Home screen, store listing, web app | `app-icon/icon-{192,512,1024}.png`, `app-icon/apple-touch-icon-180.png` |
| Android maskable | `app-icon/icon-maskable-{192,512}.png` |
| Social card, 1200×630 | `og/mark-og.png` |
| Colours and type as code | `tokens.json` (generated `tokens.css`) |

## Colours

| Token | Hex | Use |
|---|---|---|
| `ink` | `#0B0C10` | the logo tile; text in the light theme; page surface in the dark theme |
| `ink-dark` | `#EDEFF4` | text in the dark theme |
| `on-dark` | `#FFFFFF` | the M stroke on the tile |
| `paper` | `#FAFAFA` | page surface in the light theme |
| `paper-dark` | `#0B0C10` | page surface in the dark theme (same value as `ink`) |
| `accent` | `#5B63F5` | light-theme accent |
| `accent-dark` | `#7C83FF` | dark-theme accent |

The badge, banner, score and stats renderers carry their own palettes — the
eight themes, the shields colour names, the automatic score grading and the
3,400+ Simple Icons brand colours. Those are product output, not brand.

## Type

| Role | Stack |
|---|---|
| Studio UI | `ui-sans-serif, system-ui, -apple-system, "Segoe UI", Inter, Roboto, Helvetica, Arial, sans-serif` (`font.sans`) |
| Code and URL fields | `ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, monospace` (`font.mono`) |

No font file ships and no webfont is fetched: ADR-0004 decision 3 settles on
system stacks, so there is nothing to license or host. Rendered images carry
their own stacks (`src/capabilities/mark/domain/text.rs`, `font=sans|mono`),
which are product output.

## Small sizes

16 and 32 px are drawn on the pixel grid, not scaled. `build.py` renders the
app-icon master at 8×, snaps every sample to the nearest colour in
`icon.palette` (`#0B0C10`, `#FFFFFF`), and a pixel stays empty when fewer than
`snap_threshold` (0.5) of its samples are filled.

- `favicon/grid-16.txt` and `favicon/grid-32.txt` are those grids as text,
  with the palette in the header. Hand edits are kept: `build.py` redraws from
  the grid file, and only `python3 brand/build.py --resnap` throws them away.
- `favicon/favicon.svg` is the 32 px grid as rectangles; the studio serves it
  at `/favicon.svg`.
- 48 px and up use the vector master.

## Clear space and minimum size

Not yet specified. The mark has only ever existed as the studio favicon
(added 2026-09-26), and no document in the repository states clear space or a
minimum size. What the files do fix: the masters are tight to their artwork
(the tile fills the canvas, the symbol's viewBox is tight to the stroke), so
clear space is added where the mark is placed, not inside the file, and the
tile's corner radius is 25% of the canvas (rx 8 of 32).

## Do / Don't

The repository has no logo style guide; these are the rules it does record.

Do:

- Put the tile (`svg/mark-app-icon.svg`) wherever the mark stands alone, and
  the symbol where the ground is already the tile colour.
- Take colours from `tokens.json`. The studio page uses these exact values.
- Keep rendered images neutral: themes, icon glyphs and named colours carry no
  personal or company names (ADR-0004 decision 1) — identity in an image is
  the user's own `text`/`desc`.
- Keep the watermark opt-in (`DEFAULT_CREDIT=0` in `sylphx.toml`). The credit
  and the deploy mark are the only Sylphx-branded surfaces, and both are
  product-level, never theme-level (ADR-0004 decision 1).

Don't:

- Don't add a wordmark or a name to the logo. The product name is set in type
  beside the mark, never drawn into it.
- Don't recolour. One-colour work uses `svg/mark-symbol-black.svg` or
  `-white.svg` rather than a new colour.
- Don't scale one axis only, or redraw the M: the tile is square and the
  symbol's viewBox is tight, so a distorted copy no longer matches the
  masters.
- Don't break a public URL to change the brand. IDs, routes and hosts that
  were once public keep working (ADR-0005 decision 3).

## Surfaces

| Surface file (URL) | Brand file |
|---|---|
| `static/favicon.svg` (`/favicon.svg`, linked from `static/index.html` and `static/404.html`) | `favicon/favicon.svg` |
| `static/og.png` (`/og.png`, the `og:image` and `twitter:image` in `static/index.html`) | `og/mark-og.png` |

Both are copies; `python3 brand/build.py` writes them and `--check` fails if
they drift.

Surfaces still to move:

- `static/index.html` — the studio's inline `<style>` hard-codes the brand
  hex of the tokens above: `--bg:#0B0C10` / `#FAFAFA`, `--fg:#EDEFF4` /
  `#0B0C10`, `--accent:#7C83FF` / `#5B63F5`, plus the two
  `<meta name="theme-color">` values in the head. Nothing serves
  `brand/tokens.css` (the CSS is inline in the page, and `brand/` is outside
  the Docker build context), so a `var(--brand-color-…)` wiring needs a served
  stylesheet URL that does not exist yet.
- `src/interfaces/http/studio.rs` — the fallback page painted when
  `static/index.html` is missing uses GitHub's colours (`#0d1117`,
  `#e6edf3`), not the brand. It is a fallback, not a brand surface.
- `README.md` — the hero, badges, typing line, icon strips and stat cards are
  all URLs served by Mark itself, so there is nothing to copy.

## Provenance

| File | Where it came from |
|---|---|
| `svg/mark-app-icon.svg` | moved from `static/favicon.svg`; added in `1106f9e` (2026-09-26, PR #82) and unchanged |
| `svg/mark-symbol*.svg`, `svg/mark-maskable.svg` | derived from `svg/mark-app-icon.svg` when the brand home landed (2026-09-28): the M stroke alone with a tight viewBox (`7.7 9.2 16.6 14.1`), and the same tile with the corner radius dropped so it fills the canvas. Geometry and stroke width unchanged |
| `og/mark-og.png` | moved from `static/og.png`; added in `1106f9e` (2026-09-26, PR #82). **No source found**: the commit adds the PNG alone, with no script, HTML or SVG that draws it (`git log --stat 1106f9e -- static/og.png`; `git grep og.png` finds only the `og:image` metas). It stays a raster master, and the gap stands |
| `tokens.json` | the studio's theme variables in `static/index.html` and the logo master, read when the brand home landed (2026-09-28) |
| `build.py` | copy of the shared brand-kit generator, unmodified |
| `favicon/*`, `app-icon/*`, `tokens.css` | generated by `build.py` |

Every file's SHA-256 is in `provenance.json`, and `python3 brand/build.py
--check` verifies the hashes and that each surface is a byte copy of its
brand file.

## Trademark

Not registered. Owner decision owner#781: no trademark filings before the product earns money. Use ™ at most, never ®.

## Similarity check

<!-- similarity: filled in by review -->
