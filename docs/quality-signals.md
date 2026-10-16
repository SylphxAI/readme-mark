# Mark quality signals

## Image-success journey declaration

Owner: Mark, repository `SylphxAI/readme-mark`. Public host:
`mark.sylphx.com`. Production tenant namespace:
`env-019f695ace307e279aca` (read from `edge_route_backend_info` on 2026-10-07;
re-read the host's route mapping if the environment moves).

| Field | Declaration |
| --- | --- |
| Journey | `mark.image` |
| Objective | 99% of finished origin image requests succeed |
| Success event | `mark.image.ok`, severity `info` |
| Failure event | `mark.image.failed`, severity `info` |
| SLI | Failed images / (successful + failed images), scoped to this tenant and journey |
| Generic consumer | Infra `product-journey-success`, grouped by `tenant_namespace` and `journey` |
| Alert route | `ops-intake`: Work items, never chat or ticket digest |

Mark adopts the generic consumer's 99% objective, four multiwindow burn rates
and 20-event minimum in each burn's long window. The consumer strips the
`.ok` / `.failed` suffix to discover `mark.image`; it needs no Mark-specific
rule or tenant selector in infra. Do not declare a second image SLO there.
This document owns the product declaration and runbook; the event-producing
code is the executable registration with the existing generic consumer.

Image routes emit one journey line per completed origin response, including
SVG aliases and the image dialects on `/` and `/api`. Studio, health, catalogue,
legal pages and static assets do not contribute image successes. A 5xx on an
image route is failed; a valid SVG or conditional 304 is successful. Upstream
fallback cards remain 200 images, so they report an issue but do not count as
broken images. Edge cache hits do not reach the origin and are not counted.

## Issue events

`mark.issue.turn_failed.<route>` reports a 5xx using only the matched route
without path parameters. `mark.issue.tool_failed.<subject>` reports an
unavailable live badge lookup; the subject is the code's fixed resource kind
(`repo`, `package`, `crate` or `image`), not a hostname or user input.

One occurrence is one structured line. Event names are bounded fingerprints
of at most 64 `[a-z0-9_.]` characters. Lines carry enumerated codes/status and
release only, never the query, path ids, upstream URL or user's text.
Infra's generic `ProductIssueReported` rule groups by namespace and event;
Work intake files one item and adds a repeat note when the alert's occurrence
count rises. No Mark-local issue tracker or grouping mechanism exists.

## Respond

For a burn with `journey=mark.image`, read
`{namespace="env-019f695ace307e279aca"} |= "mark.image.failed"` in Loki,
then `|= "mark.issue."` in the same namespace for the failing route/resource
fingerprints. Read the matching `ProductIssueReported` Work item and the
release recorded on the lines. Fix the product, not the generic alert rule.

A `tool_failed` means the live dependency was unavailable and Mark served its
fallback badge. Inspect that dependency and cache through Mark's normal
records. A `turn_failed` means the caller saw a 5xx. Health/studio failures
still report issues, but never inflate this image SLO denominator.

The generic SLO's missing-signal alert watches the fleet's journey counter
path, not a quiet Mark tenant. Below 20 image events the journey is measured
without a burn alert. Absence is not proof that this release emits signals:
read an origin request's log line before claiming adoption.

## Acceptance induction

On a deployed or isolated test instance, make one unavailable live badge
lookup twice within an hour. Both requests must still return a 200 SVG
fallback, emit the same `mark.issue.tool_failed.<subject>` fingerprint and
emit one `mark.image.ok` each. An isolated instance can use
`AppState::for_tests()` and `/github/stars/offline/repo`: `offline` is a
fixture timeout, not a production fault switch. A production induction must
use an actual controlled unavailable upstream; `?username=offline-user` does
not induce a timeout in production.

After the first occurrence is scraped/evaluated and read by Work intake,
record the new item id and alert fingerprint. Make the second occurrence,
wait for the rule's count to increase and intake to read it, then read back
exactly the same item plus one `occurred again` note. Intake throttles repeat
notes to one per fingerprint per hour. Do not attest production acceptance
from the fixture, a log alone or an unchanged release.
