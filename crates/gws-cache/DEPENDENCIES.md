---
created_on: 2026-10-14 18:08
last_modified: 2026-10-14 18:08
status: current
---

# Dependencies and provenance

Source behavior and MIT attribution come from [alexgorbatchev/gws-cache-cli](https://github.com/alexgorbatchev/gws-cache-cli). Reusable cache, classifier, sync and export operations are separated from its CLI. The Go schema fixture is retained verbatim for native SQLite migration checks.

| Source functionality | Rust integration |
| :--- | :--- |
| `modernc.org/sqlite`, `database/sql` | `rusqlite` 0.40.2, bundled native SQLite transactions, optional rows and Chrono decoding. Default features disabled; bundled and Chrono explicitly enabled. |
| Time arithmetic and zones | `chrono` 0.4.45 checked UTC arithmetic and `chrono-tz` 0.10.4 native IANA local-time resolution. |
| `net/mail`, MIME words | `mailparse` 0.18 native header address parser, single-address extraction and date parser. |
| Regex and JSON | `regex` 1.13.1, `serde`, `serde_json`; Unicode word boundaries use the Rust engine. |
| HTML entities | `html-escape` 0.3 native decoding before timeline newline flattening. |
| `os/exec`, contexts | Tokio process I/O, timers, cancellation tokens and native Unix groups/signals through `nix`. |
| Wrapped errors | Native `thiserror` typed errors preserving their sources. |

The [Google Workspace CLI](https://github.com/googleworkspace/cli) is itself Rust. Its actual `error.rs` verifies JSON errors on stdout containing HTTP codes/messages/reasons and distinct process exit codes. Calls use native `--params` and `--format json`; successful exits containing error envelopes also fail.

Verified protocol references:

- [Gmail discovery](https://gmail.googleapis.com/$discovery/rest?version=v1): history IDs are decimal `uint64` strings, compared numerically.
- [Gmail filtering](https://developers.google.com/workspace/gmail/api/guides/filtering): epoch-second `after:` avoids date-string timezone interpretation.
- [Calendar events](https://developers.google.com/workspace/calendar/api/v3/reference/events): calendar-scoped IDs and sparse cancellations.
- [Calendar listing](https://developers.google.com/workspace/calendar/api/v3/reference/events/list): date bounds are excluded with cursors; HTTP 410 requires fresh synchronization.
- [RFC 3834](https://www.rfc-editor.org/rfc/rfc3834): `Auto-Submitted` keyword and optional parameters.

The pinned Go driver's `modernc.org/sqlite` 1.55.0 source verifies its `time.Time.String()` encoding. Migration decodes numeric offsets, zone abbreviations and monotonic suffixes before native UTC storage. Schema migration is one-way, without a Go API facade. Direct crates and transitive versions are pinned in `Cargo.lock`; no crate is vendored.
