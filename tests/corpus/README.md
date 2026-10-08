# README dialect corpus

`readme-urls.json` records 50 distinct image URLs from real upstream READMEs:
18 shields badges, 4 skill-icons grids, 2 typing images, 16 capsule banners and
10 GitHub stats cards. Each `source` pins the README commit and line where the
URL appeared. Markdown backslash escapes before `&` are removed; browser
fragments stay in the recorded URL but are not sent in HTTP requests.

`cargo test --locked --test dialect_corpus` sends each original path and query
through Mark's router with only the host changed. Every case must answer
`200 image/svg+xml` and contain its recorded geometry and text. The test checks
uniqueness and dialect counts, and reports missing expectations with their
source URLs. It runs automatically in the existing CI `cargo test --locked`.

All rendering is offline. Stats for the corpus's `anuraghazra` subject use the
existing synthetic counters with an explicit profile name in `FixtureUpstream`;
they are not measurements of that person's account. Row text assertions reject
missing-user or unavailable fallback cards. Static cases assert dimensions,
icon labels, typing lines, or banner text, rather than storing full SVGs.
These are structural host-swap contracts, not pixel comparisons against the
upstream services or a claim that every optional parameter is supported.

To add a case, copy an image URL from a README, retain a commit-pinned source
link, and record expected geometry and text independently of a snapshot update.
Update the expected counts in `dialect_corpus.rs`. Never fetch upstream services
at test time or regenerate expectations from the response under test.
