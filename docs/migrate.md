# Migrate README images to Mark

Switch an existing image to Mark by replacing its host with `mark.sylphx.com`.
Keep `https://`, the path, query, percent escapes and any fragment unchanged.
No account, signup or user token is needed. Change the image URL, not the
surrounding link to your project, profile or package.

These examples come from the [pinned README parity corpus](../tests/corpus/README.md).
Each Before URL is recorded in [readme-urls.json](../tests/corpus/readme-urls.json),
with its original README source. Each After URL differs only in its host.
Click an After URL to preview the SVG before replacing your README image.

## Before and after, by dialect

### shields.io — badges

| Before | After |
| --- | --- |
| `https://img.shields.io/badge/react-%2320232a.svg?style=for-the-badge&logo=react&logoColor=%2361DAFB` | <https://mark.sylphx.com/badge/react-%2320232a.svg?style=for-the-badge&logo=react&logoColor=%2361DAFB> |
| `https://img.shields.io/badge/rust-%23000000.svg?style=for-the-badge&logo=rust&logoColor=white` | <https://mark.sylphx.com/badge/rust-%23000000.svg?style=for-the-badge&logo=rust&logoColor=white> |

### skill-icons — tech icon grids

| Before | After |
| --- | --- |
| `https://skillicons.dev/icons?i=js,html,css,wasm` | <https://mark.sylphx.com/icons?i=js,html,css,wasm> |
| `https://skillicons.dev/icons?i=java,kotlin,nodejs,figma&theme=light` | <https://mark.sylphx.com/icons?i=java,kotlin,nodejs,figma&theme=light> |

### readme-typing-svg — typing lines

| Before | After |
| --- | --- |
| `https://readme-typing-svg.demolab.com/?lines=First+line+of+text;Second+line+of+text` | <https://mark.sylphx.com/?lines=First+line+of+text;Second+line+of+text> |

### capsule-render — banners

| Before | After |
| --- | --- |
| `https://capsule-render.vercel.app/api?type=waving&color=auto&height=300&section=header&text=capsule%20render&fontSize=90&animation=fadeIn&fontAlignY=38&desc=Decorate%20GitHub%20Profile%20or%20any%20Repo%20like%20me!&descAlignY=51&descAlign=62` | <https://mark.sylphx.com/api?type=waving&color=auto&height=300&section=header&text=capsule%20render&fontSize=90&animation=fadeIn&fontAlignY=38&desc=Decorate%20GitHub%20Profile%20or%20any%20Repo%20like%20me!&descAlignY=51&descAlign=62> |
| `https://capsule-render.vercel.app/api?type=rounded&color=timeAuto&text=Rounded%20with%20stroke&fontAlignY=50&fontSize=40&height=200&stroke=000000&strokeWidth=2` | <https://mark.sylphx.com/api?type=rounded&color=timeAuto&text=Rounded%20with%20stroke&fontAlignY=50&fontSize=40&height=200&stroke=000000&strokeWidth=2> |

### github-readme-stats — live GitHub cards

| Before | After |
| --- | --- |
| `https://github-readme-stats.vercel.app/api?username=anuraghazra` | <https://mark.sylphx.com/api?username=anuraghazra> |
| `https://github-readme-stats.vercel.app/api?username=anuraghazra&show_icons=true&theme=radical` | <https://mark.sylphx.com/api?username=anuraghazra&show_icons=true&theme=radical> |
| `https://github-readme-stats.vercel.app/api?username=anuraghazra&show_icons=true&theme=dark#gh-dark-mode-only` | <https://mark.sylphx.com/api?username=anuraghazra&show_icons=true&theme=dark#gh-dark-mode-only> |

## Paste into a README

```md
![GitHub stats](https://mark.sylphx.com/api?username=anuraghazra)
```

Keep your own username, text, labels and other parameters when migrating your
images. Preserve `+` and `%20` as written; do not decode `%23` into `#`, because
that changes a color parameter into a browser fragment. Keep `#gh-dark-mode-only`
and `#gh-light-mode-only` on images that select GitHub's viewer color scheme;
fragments are not sent to Mark's server.

## What compatibility means

The corpus test exercises every recorded path and query through Mark's router,
asserting `200 image/svg+xml` plus expected geometry and text. The migration
table is checked against that same corpus. This is a structural compatibility
contract, not a pixel-identical copy of another service or support for every
optional parameter. Mark uses its own rendering defaults, fonts and themes;
unknown values normalize to defaults. Static images depend only on their URL,
so capsule's `timeAuto` does not make Mark change colors with the clock.

GitHub cards read public data, cache it and can show stale data or a
"temporarily unavailable" fallback when the upstream is unavailable. A valid
SVG response alone does not prove fresh GitHub data. For supported dynamic
badge paths, card routes and options, see the [README reference](../README.md#reference)
and [dynamic badges](../README.md#github-stats--live-cached-no-token).

## Verify or undo a switch

Open each replacement URL and check the text, layout and theme before committing
your README. If the result is not what you want, restore the original host from
your README diff; no account or data migration needs undoing. An unsupported
shields path returns an `unsupported` badge rather than a broken image. Keep
that badge on its original host until its data source is supported.

The [studio](https://mark.sylphx.com) also accepts an existing image URL for editing.
See [ADR-0005](adr/ADR-0005-readme-visuals-toolkit.md) for the host-only dialect
contract and the guarantee that published Mark image URLs keep working.
