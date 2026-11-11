---
version: alpha
name: Mark
description: Ink, proof paper and one highlighter stroke. A quiet print-shop studio where the user's README image is the only loud thing on screen.
colors:
  ink: "#0B0C10"
  paper: "#F7F6F2"
  highlighter: "#D4F53C"
  on-highlighter: "#0B0C10"
  surface: "#F7F6F2"
  surface-raised: "#FFFFFF"
  surface-sunken: "#EEEDE7"
  on-surface: "#0B0C10"
  on-surface-muted: "#56585F"
  outline: "#D9D8D1"
  primary: "#0B0C10"
  on-primary: "#FFFFFF"
  primary-hover: "#2A2C33"
  error: "#B42318"
  surface-dark: "#0B0C10"
  surface-raised-dark: "#15171C"
  surface-sunken-dark: "#07080B"
  on-surface-dark: "#EDECE6"
  on-surface-muted-dark: "#9B9EA7"
  outline-dark: "#2A2D35"
  primary-dark: "#F7F6F2"
  on-primary-dark: "#0B0C10"
  primary-hover-dark: "#FFFFFF"
  error-dark: "#FF8A7A"
  stage-light: "#FFFFFF"
  stage-dark: "#0D1117"
typography:
  display: { fontFamily: Instrument Sans, fontSize: 56px, fontWeight: 600, lineHeight: 1.02, letterSpacing: -0.035em }
  headline-lg: { fontFamily: Instrument Sans, fontSize: 36px, fontWeight: 600, lineHeight: 1.1, letterSpacing: -0.025em }
  headline-md: { fontFamily: Instrument Sans, fontSize: 24px, fontWeight: 600, lineHeight: 1.2, letterSpacing: -0.015em }
  title: { fontFamily: Instrument Sans, fontSize: 18px, fontWeight: 600, lineHeight: 1.3 }
  body-lg: { fontFamily: Instrument Sans, fontSize: 17px, fontWeight: 400, lineHeight: 1.55 }
  body-md: { fontFamily: Instrument Sans, fontSize: 15px, fontWeight: 400, lineHeight: 1.5 }
  body-sm: { fontFamily: Instrument Sans, fontSize: 13px, fontWeight: 400, lineHeight: 1.45 }
  label-md: { fontFamily: Instrument Sans, fontSize: 14px, fontWeight: 600, lineHeight: 1 }
  url: { fontFamily: JetBrains Mono, fontSize: 14px, fontWeight: 500, lineHeight: 1.5, fontFeature: '"calt" 0' }
  code-sm: { fontFamily: JetBrains Mono, fontSize: 12px, fontWeight: 400, lineHeight: 1.5 }
  label-caps: { fontFamily: JetBrains Mono, fontSize: 11px, fontWeight: 600, lineHeight: 1, letterSpacing: 0.08em }
rounded:
  none: 0px
  sm: 4px
  md: 8px
  lg: 12px
  full: 9999px
spacing:
  unit: 4px
  xs: 4px
  sm: 8px
  md: 16px
  lg: 24px
  xl: 40px
  xxl: 72px
  gutter: 24px
  margin: 24px
  max-width: 1200px
  control-height: 40px
components:
  button-primary: { backgroundColor: "{colors.primary}", textColor: "{colors.on-primary}", typography: "{typography.label-md}", rounded: "{rounded.md}", height: 40px, padding: 16px }
  button-primary-hover: { backgroundColor: "{colors.primary-hover}", textColor: "{colors.on-primary}" }
  button-primary-dark: { backgroundColor: "{colors.primary-dark}", textColor: "{colors.on-primary-dark}" }
  button-secondary: { backgroundColor: "{colors.surface-raised}", textColor: "{colors.on-surface}", typography: "{typography.label-md}", rounded: "{rounded.md}", height: 40px, padding: 16px }
  button-secondary-dark: { backgroundColor: "{colors.surface-raised-dark}", textColor: "{colors.on-surface-dark}" }
  input: { backgroundColor: "{colors.surface-raised}", textColor: "{colors.on-surface}", typography: "{typography.body-md}", rounded: "{rounded.md}", height: 40px, padding: 12px }
  input-dark: { backgroundColor: "{colors.surface-raised-dark}", textColor: "{colors.on-surface-dark}" }
  input-error: { backgroundColor: "{colors.surface-raised}", textColor: "{colors.error}" }
  card: { backgroundColor: "{colors.surface-raised}", textColor: "{colors.on-surface}", rounded: "{rounded.lg}", padding: 24px }
  card-dark: { backgroundColor: "{colors.surface-raised-dark}", textColor: "{colors.on-surface-dark}" }
  nav: { backgroundColor: "{colors.surface}", textColor: "{colors.on-surface}", typography: "{typography.label-md}", height: 56px, padding: 24px }
  nav-dark: { backgroundColor: "{colors.surface-dark}", textColor: "{colors.on-surface-dark}" }
  url-bar: { backgroundColor: "{colors.ink}", textColor: "{colors.on-surface-dark}", typography: "{typography.url}", rounded: "{rounded.md}", height: 48px, padding: 16px }
  preview-stage: { backgroundColor: "{colors.stage-light}", rounded: "{rounded.lg}", padding: 40px }
  preview-stage-dark: { backgroundColor: "{colors.stage-dark}", rounded: "{rounded.lg}", padding: 40px }
  highlight-mark: { backgroundColor: "{colors.highlighter}", textColor: "{colors.on-highlighter}", rounded: "{rounded.sm}", padding: 4px }
  kind-tab-active: { backgroundColor: "{colors.ink}", textColor: "{colors.paper}", typography: "{typography.label-md}", rounded: "{rounded.full}", height: 32px, padding: 14px }
  kind-tab: { backgroundColor: "{colors.surface}", textColor: "{colors.on-surface-muted}", typography: "{typography.label-md}", rounded: "{rounded.full}", height: 32px, padding: 14px }
  button-primary-hover-dark: { backgroundColor: "{colors.primary-hover-dark}", textColor: "{colors.on-primary-dark}" }
  input-error-dark: { backgroundColor: "{colors.surface-raised-dark}", textColor: "{colors.error-dark}" }
  kind-tab-dark: { backgroundColor: "{colors.surface-dark}", textColor: "{colors.on-surface-muted-dark}" }
  code-block: { backgroundColor: "{colors.surface-sunken}", textColor: "{colors.on-surface}", typography: "{typography.code-sm}", rounded: "{rounded.md}", padding: 16px }
  code-block-dark: { backgroundColor: "{colors.surface-sunken-dark}", textColor: "{colors.on-surface-dark}" }
  divider: { backgroundColor: "{colors.outline}", height: 1px }
  divider-dark: { backgroundColor: "{colors.outline-dark}", height: 1px }
---

# Mark

## Overview

Mark turns one URL into a README image: banners, badges, typing text, tech
icons and GitHub stats cards. Free, no token, no signup. The audience is
developers and open-source maintainers polishing a profile or project README.
They arrive with a job, compose an image in the studio, copy one line and
leave. The product is that line.

The identity is a **print shop**: black ink, warm proof paper, and one
highlighter stroke, the gesture of marking the line that matters. The studio
is a proofing table. It stays monochrome so the user's own image, in whatever
theme they picked, is the only colour on screen, and the copyable URL is the
most important object on every page.

- **Name:** "Mark", one word, capital M. Endorsed "Mark by Sylphx" in the
  footer and on social cards; never "Sylphx Mark" in prose. Sylphx Limited
  is the legal operator line, not a brand element.
- **Logo:** the existing M stroke (rounded caps and joins) on an ink tile,
  from `brand/svg/`. There is no wordmark; "Mark" is set in Instrument Sans
  600 next to the tile. Never redraw or recolour the M.
- **Locale:** English only. CJK appears only as user content inside rendered
  images.
- **Voice:** terse and dev-to-dev. Lead with what you get, then show the
  URL. Use concrete nouns and numbers that are real ("3,400+ icons", "change
  the host"). No hype words, no exclamation marks, no emoji in UI copy.
  Example: "Keep your shields URL. Change the host."
- **Social posts and marketing images:** show real images Mark rendered,
  placed in a realistic README frame (GitHub light or dark) on a paper or ink
  field. The URL that made the image appears below it in JetBrains Mono, with
  a highlighter swipe behind the one parameter that changed. Layouts are flat
  and grid-aligned, with generous margins. No AI-generated illustration,
  mascots, 3D renders, stock photos of laptops, glassmorphism or
  purple-to-blue gradients.

Renderer themes, shields colours and Simple Icons colours are content the
user chooses, never Mark's own chrome, pages or posts.

## Colors

Two neutrals carry everything; one accent marks.

- **Ink (#0B0C10):** the logo tile, all text in light mode, primary buttons,
  the URL bar and the dark-mode page surface.
- **Proof Paper (#F7F6F2):** the light page surface. Warm enough to read as
  paper next to the pure white (#FFFFFF) raised cards and preview stage.
- **Highlighter (#D4F53C):** a fluorescent yellow-green used only as a fill
  behind ink text (#0B0C10): the key word in a headline, the copied state,
  the changed URL parameter, the current selection. Never as text, an icon
  colour or a large background, and at most one highlight per view.
- **Neutrals:** sunken #EEEDE7 for control wells and code blocks, outline
  #D9D8D1 for decorative 1px borders and dividers (never the only edge of a
  control), muted text #56585F.
- **Error (#B42318, dark #FF8A7A):** validation messages only. Rendering never
  fails, so error is rare; it is never decorative.
- **Dark mode:** surface #0B0C10, raised #15171C, sunken #07080B, text
  #EDECE6, muted #9B9EA7, outline #2A2D35. The primary button inverts to
  paper #F7F6F2 with ink text; hover goes to #FFFFFF. Light hover is #2A2C33.
  Follow `prefers-color-scheme` and offer a toggle.
- **Stage (#FFFFFF / #0D1117):** the preview stage reproduces GitHub's light
  and dark README backgrounds so users judge their image where it will live.
  These two values are for the stage only.

## Typography

**Instrument Sans** (SIL OFL) for all interface and marketing text: a
compact grotesque with a slightly narrow, engineered feel that sets
"Mark" cleanly at display sizes. **JetBrains Mono** (SIL OFL) for every URL,
parameter, code sample and the caps micro-labels. Disable contextual
ligatures in URLs (`"calt" 0`) so `--`, `->` and `==` copy as typed.

Fallback stacks: `"Instrument Sans", ui-sans-serif, system-ui, -apple-system,
"Segoe UI", Roboto, Helvetica, Arial, sans-serif` and `"JetBrains Mono",
ui-monospace, SFMono-Regular, Menlo, Consolas, monospace`. Self-host both as
WOFF2 with `font-display: swap`. Rendered SVG images keep their own
system stacks; these fonts style Mark's site and posts, not the images.

- **Display / headline:** weight 600 with tight negative tracking. Sentence
  case. One highlight-mark per headline at most.
- **Body:** 15px in the studio, 17px on landing and docs prose; max 68ch.
- **URL:** 14px mono, weight 500. A URL is never truncated mid-parameter;
  wrap at `&` and `?`.
- **Label-caps:** 11px mono uppercase for section markers ("BANNER",
  "BADGE") and metadata. Never for sentences.

## Layout

A 12-column grid, max width 1200px, 24px gutters and margins, on a 4px base
unit. The studio is two panes: controls (4 columns) on the left, the preview
stage and URL bar (8 columns) on the right, with the URL bar pinned under the
stage so the output is always visible. Below 900px the panes stack: stage
first, URL bar, then controls. Landing and docs pages use a single 8-column
reading column with full-width gallery bands. Vertical rhythm uses 24px within
a section and 72px between sections. Controls are 40px tall (44px touch
targets on mobile).

## Elevation & Depth

Flat, like paper on a table. Hierarchy comes from tone (sunken, surface,
raised) and 1px outlines, not shadows. Two exceptions: the preview stage
carries one soft shadow (`0 1px 2px rgba(11,12,16,0.06), 0 16px 40px
-24px rgba(11,12,16,0.24)`) so the user's image sits forward, and menus and
toasts use the same shadow. In dark mode shadows disappear; raised surfaces
#15171C on #0B0C10 with outline #2A2D35 do the work. No blur or glass.

## Shapes

Crisp with a small, consistent radius. Controls, inputs, the URL bar and code
blocks: 8px. Cards and the preview stage: 12px. Kind tabs and filter chips
are full pills, so they read as choices rather than actions. The highlight
mark is a 4px-radius block that runs slightly past the word (4px padding)
like a real marker stroke. Never mix sharp and rounded corners in one view.
The logo tile keeps its master geometry.

## Components

- **button-primary:** ink (#0B0C10) with white text, 40px, 8px radius. One
  per view: "Copy Markdown". After copying it turns highlighter (#D4F53C) with
  ink text and reads "Copied" for 1.5s.
- **button-secondary:** white raised fill, ink text, 1px outline #D9D8D1.
  For "Copy HTML", "Download SVG" and other secondary actions.
- **input:** white fill, 1px border in muted #56585F (#9B9EA7 in dark
  mode) so the field edge clears 3:1, ink text, muted placeholder. Focus is a
  2px ink ring (paper #F7F6F2 in dark mode), never a glow. Errors use the
  error colour for the outline and message.
- **card:** white raised fill, 1px outline, 12px radius, 24px padding. Gallery
  cards show a rendered image on top and its URL in code-sm beneath.
- **nav:** 56px bar on the page surface with a bottom outline. The left side
  holds the logo tile and "Mark"; the right side holds Studio, Docs, GitHub;
  the active link is ink, others muted.
- **url-bar:** the defining component. An ink strip (sunken #07080B with a
  #2A2D35 outline in dark mode), with the live URL in JetBrains Mono #EDECE6, the parameter just changed wrapped in a
  highlight mark, and the copy button inline on the right. It is the first
  thing to update when any control changes.
- **preview-stage:** the user's image centred on a GitHub-light (#FFFFFF) or
  GitHub-dark (#0D1117) field with a two-way toggle, 40px padding and a real
  README column width (888px max) so scale is honest.
- **code-block / divider:** sunken fill, code-sm, 8px radius; dividers are
  1px outline.
- **kind-tab:** pills for Banner, Badge, Typing, Icons, Stats. Active is ink
  with paper text; inactive is muted text on the surface.

## Do's and Don'ts

- Do let the user's image be the only colourful element; Mark's chrome stays
  ink and paper.
- Do show the URL next to every image, in mono, copyable.
- Do use the highlighter once per view, as a fill behind ink text.
- Don't style Mark's site or posts with renderer themes, gradients or the
  old indigo accent.
- Don't put highlighter text on paper or highlighter fills behind muted text;
  both fail contrast.
- Don't redraw the M, add a wordmark image, or place "by Sylphx" inside the
  logo lockup; it sits in the footer or caption.
- Don't use AI illustrations, mascots, emoji or exclamation marks in brand
  surfaces.
- Do keep WCAG AA in both modes: 4.5:1 for text, 3:1 for input borders,
  focus rings and icons that carry meaning.
