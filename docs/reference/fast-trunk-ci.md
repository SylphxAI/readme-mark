# Fast Trunk CI

## Authority split

| Concern | Owner |
| --- | --- |
| Work / claim / review | none — Git history + this repository CI are the durable record (Enact is retired) |
| Source history | Git |
| Source correctness | This repository CI (`source-ci/pass`) |
| Production artifact build | Sylphx Platform (once) |
| Deploy / health / rollback | Sylphx Platform |

## Paths

- **Internal agents:** small-batch non-force direct-trunk to default branch.
- **External contributors:** Pull Request presubmit feedback.
- **Merge Queue:** on (`merge_group` trigger wired; concurrency cancels superseded runs).

## CI scope

Blocking: lint/typecheck, affected tests, schema/migration safety, narrow security,
non-v7 identifiers on added lines.

Not in source CI: production Docker/release image builds, disposable ship binaries for ordinary tips.
