---
created_on: 2026-10-14 18:22
last_modified: 2026-10-14 22:40
status: current
---

# GWS Cache CLI

`gws-cache` manages a local Gmail and Calendar cache through the internal `crates/gws-cache` package in this repository's Cargo workspace. Use Rust, Cargo, just and the installed `gws` for runtime synchronization.

## Commands

- Run: `just run cache status`.
- Agent mode: `just run-ai cache status`.
- Test: `just test`.
- Format and lint: `just lint`.
- Full local gate: `just check`.
- Optimized executable: `just build`, producing `bin/gws-cache`.

## Contracts

- Keep CLI parsing native to Clap. Validate arguments and cross-flag windows before opening the cache; load classification rules only for incoming mail.
- Use the internal library's concrete SQLite, classification, sync and export APIs. Preserve overlapping topic membership, composite calendar identity, sparse cancellation and atomic cursor commits.
- Keep timeout per subprocess. Nonpositive Go durations disable the deadline while SIGINT/SIGTERM cancellation remains active. Preserve Unix group termination and child reaping.
- Keep JSON stdout a single valid document, including empty arrays. Emit warnings and the single returned error through stderr. Any failed topic in an all-topic sync must make the command fail after reporting outcomes.
- Route printed paths through `cli-command-kit`; retain original paths for filesystem and subprocess operations. Preserve actual cached contents in JSON/export data.
- Generate shell completion into memory before writing to fallible output.
- Maintain `skills/gws-cache/SKILL.md` in the same change as commands, arguments, flags, defaults, environment, output and side effects; update metadata and run its live interface check. Keep one canonical usage file; the executable prints its compiled installation pointer.
- Any runtime-result change requires a corresponding behavioral test change and at least 90% line coverage. Keep red/green and source-disable/restore evidence under `.tmp/`.
- Use isolated local subprocesses and SQLite fixtures for verification. Never read user credentials, cache or mail, or perform live Google requests as a test shortcut.
- Scope formatters to this package and preserve ownership of other packages. Keep standing rules here and progress/evidence in separate documents.
