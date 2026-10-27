---
created_on: 2026-10-14 18:08
last_modified: 2026-10-14 22:40
status: current
---

# GWS Cache library

Run `just test`, `just lint`, `just coverage`, `just check`, or `just build` from this internal workspace package in the `gws-cache-cli` repository. Inherit `../../AGENTS.md` and read applicable skills before edits. Any runtime change requires a corresponding behavioral test-file change and at least 90% line coverage; `scripts/` is excluded.

- Validate complete threads before their transaction; never commit a history marker before all messages. Threads may belong to several topics.
- Keep Calendar identities composite `(calendar_id, id)`. Commit fetched records and the final cursor atomically. Only typed HTTP 410 expires a cursor; other failures preserve it. Handle sparse cancellations without inventing timestamps.
- Keep deadlines per invocation, including pipe drainage. Preserve Unix process-group cancellation and direct-child reaping.
- Keep migrations transactional and validate foreign keys before committing. Reject future schemas. Preserve the Go timestamp and relationship migration fixtures.
- Exercise subprocesses through real local executables and persistence through native SQLite. Keep evidence under `.tmp/`. Never read unrelated credentials or make live Google calls in automated checks.
- Preserve typed failures and partial-run accounting. Never advance a watermark past failed or deferred downloads.
- Keep task history outside this file; update standing guidance in place. Never publish releases, tags, packages or production deployments without explicit user authorization.
