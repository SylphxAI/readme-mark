# Production README compatibility verdict

[Production compatibility runs](https://github.com/SylphxAI/readme-mark/actions/workflows/production-compatibility.yml)
record an independent observation of `https://mark.sylphx.com`, not a merge gate,
a deployment command, an availability SLO, or a pixel comparison.

## Automatic execution and reading a result

- After the existing release writer completes its `sylphx/deploy` check with
  `success` / `Live`, the workflow runs. It accepts only the `sylphx-ai` app's
  production check, not `sylphx/preview`, source CI, build completion or a push.
  The check's SHA must match the public `/health.revision`; otherwise the
  observation is **UNKNOWN**, never green for an older release. The check may
  belong to a merge-queue suite, so its branch name is not a production filter.
- Each day at **06:23 UTC**, GitHub schedules the same observation (normal
  Actions scheduling delays apply). The workflow file on the default branch
  is the schedule's configuration; the Actions list shows actual executions.
- **Run workflow** is the ordinary manual trigger for an immediate observation,
  including the first run. There is no need to wait for the daily schedule.

Open a run's **corpus** job summary for the verdict, UTC timestamps, serving
revision before and after, checked-out source revision, last corpus change
revision, exact corpus blob and SHA-256, all case URLs, per-dialect counts and
optional metric omissions. The `production-compatibility-…` artifact contains
`result.json` (including each case's pinned README source, original URL,
response metadata and failures) and `summary.md`, retained for **90 days**.
The summary and job conclusion remain ordinary repository Actions records;
no implementing session or local filesystem is needed to read them. Keep
GitHub's existing workflow-failure notifications enabled to receive failures.
No new notification sender, ticket writer or alert pipeline is installed.

## One corpus, one live checker

The workflow calls `scripts/probe-dialects.sh`, extending the existing
`scripts/probe-dialects.py`; its sole input is the unchanged
[`tests/corpus/readme-urls.json`](../tests/corpus/readme-urls.json). Today that
is all **50** pinned URLs: shields **18**, skill-icons **4**, typing **2**,
capsule-render **16**, GitHub stats **10**. Only the host changes. The original
encoded path/query is preserved and browser fragments are not sent.

Each request must return 200, SVG media type, well-formed namespaced SVG XML,
and the existing corpus's required geometry/text. A temporarily-unavailable
fallback fails even if it is a 200 SVG. Optional live metrics remain explicitly
listed when omitted; their omission cannot excuse missing required content.
The existing Rust offline contract still requires the fixture's optional rows.

An ordinary run makes **52 anonymous GETs maximum**: the 50 corpus cases and
one `/health` observation on either side. Each curl request has a 3-second
connect limit, 8-second total limit, 2 MiB body limit and 10-second process
limit. Curl config files, credentials, redirects and retries are not enabled.
The job is capped at 12 minutes. A failed case does not suppress later cases.
No waiting for rollout, automatic replay, production fault injection, live
state mutation or always-on monitor is involved.

**FAIL** means at least one observed case failed. **UNKNOWN** means the
instrument, source/corpus provenance, health revision, deployment match or
completeness could not be established (including a release changing during
the run). Both are nonzero and produce a failed Actions job; only a complete
successful observation is **PASS**. Publication runs even on failure. If the
instrument cannot write its files, the summary explicitly says UNKNOWN and
artifact publication fails rather than presenting a healthy empty result.

## Offline verification and boundaries

```sh
python3 -I -m unittest discover -s scripts -p 'test_probe_dialects.py' -v
```

These deterministic CI tests read back recorded failures for timeout, non-200,
wrong media type, malformed XML, non-SVG XML, missing geometry/required text
and fallback content. They assert one call per case, and verify health/read
errors, unexpected instrument errors, deployment mismatches and mid-run
revision changes cannot pass. No test faults production; source CI stays
network-independent. To record an observation manually outside Actions:

```sh
scripts/probe-dialects.sh https://mark.sylphx.com --result-dir /tmp/mark-compatibility
```

This adds no serving changes or corpus copy. The existing release/migration
writer and history are untouched. [Issue events and the image SLO](quality-signals.md)
retain their existing writers and semantics: this stricter compatibility
observation does not redefine image success, file issues or declare a second
SLO. Investigate a failed run through the normal release and issue records.
