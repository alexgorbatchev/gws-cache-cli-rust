`gws-cache` gives Rust applications a searchable local copy of Gmail threads and Google Calendar events. It shares the installed Google Workspace CLI's authentication and supports saved topics, incremental synchronization and timeline export.

# What It Does

- **Saved searches:** Associates threads with every matching topic and combines queries without duplicating messages.
- **Bounded synchronization:** Limits attempted thread downloads and retains failed downloads in the backlog.
- **Calendar caching:** Keeps separate event identities for each calendar, including all-day dates and cancellations.
- **Offline access:** Searches cached records and exports chronological Markdown or JSON timelines.

# How It Works

1. Open a cache file and register topics with Gmail queries.
2. Synchronize a topic, an ad-hoc query or a calendar through authenticated `gws`.
3. Search saved records offline or export a topic's timeline.

# How it Really Works

1. Opening a cache performs blocking filesystem and SQLite operations. Each store owns one connection; synchronization borrows it exclusively. Nothing runs at import time. New Unix cache files use mode `0600` and newly created parent directories use `0700`; existing permissions are retained.
2. Mail sync reads listing pages into memory and deduplicates thread IDs. Failed downloads consume budget and hold the date watermark. Completed threads commit atomically. Warm topics search from their watermark minus one day unless an explicit window or full sync overrides it. Messages absent from a later response remain cached. Snippets supply `body_plain`; complete MIME bodies and attachments are not fetched.
3. Calendar sync saves fetched records and their final cursor together. Delta requests omit date bounds. Typed HTTP 410 clears that calendar before fetching a replacement; other errors retain it. Explicit windows and full sync replace the previous scope after successful conversion. Sparse cancellations preserve known details; unknown cancellations may have no timestamps. All-day records retain a civil date through `all_day`. Ambiguous local times without an explicit offset fail.
4. Each `gws` invocation has its own deadline, including pipe drainage; a zero duration disables that deadline while preserving cancellation. Cancellation or timeout kills the Unix process group and reaps the direct child; other platforms terminate the direct child. Authentication belongs to `gws`, and subprocesses inherit the caller's environment. Offline operations launch no subprocess and make no network requests.
5. Typed errors retain SQLite, JSON, I/O, cancellation and structured Google failures. Search treats `%` and `_` literally. Timeline export defaults to human messages from four weeks and emits `[]` for an empty JSON result.

# Prerequisites

- An authenticated [Google Workspace CLI](https://github.com/googleworkspace/cli) for synchronization; offline reads need only an existing cache.
- A Tokio asynchronous runtime for sync calls. SQLite is bundled.

# Installation

This unpublished implementation crate belongs to the `gws-cache-cli` repository. From a consumer beside that repository:

```sh
cargo add gws-cache --path ../gws-cache-cli/crates/gws-cache
```

# Quick Start

```rust
use gws_cache::{MessageFilter, Store};
use std::path::Path;

# std::fs::create_dir_all(".tmp")?;
# let fixture = tempfile::Builder::new().prefix("readme-").tempdir_in(".tmp")?;
let cache_path = Path::new("cache.db");
# let cache_path = fixture.path().join("cache.db");
let mut cache = Store::open(cache_path.as_ref())?;
cache.create_topic("work", "Work", "label:work")?;
let messages = cache.messages(&MessageFilter {
    topic: Some("work".into()),
    human_only: true,
    ..Default::default()
})?;
assert!(messages.is_empty());
# Ok::<(), gws_cache::Error>(())
```

# API

| Entry points | Purpose |
| :--- | :--- |
| `Store::open`, `topic`, `topics`, `create_topic`, `delete_topic`, `add_query`, `queries` | Open the cache and manage saved topics. |
| `Store::messages`, `events`, `event`, `stats` | Read records with literal text, topic and date filters. |
| `Client::new`, `list_threads`, `get_thread`, `calendar_events` | Invoke a selected executable with a per-request deadline and cancellation token; a zero duration disables the deadline. |
| `sync_topic`, `sync_query`, `sync_all_topics` | Synchronize mail with `MailOptions` and a caller-supplied clock. Missing topics auto-register. All-topic sync continues ordinary failures and returns per-topic error fields. |
| `sync_calendar` | Synchronize with `CalendarOptions`, typed failures and atomic cursor updates. |
| `Classifier::new`, `Classifier::load`, `convert_thread` | Classify and validate native Gmail payloads. Additional JSON rules append to built-in sender patterns. |
| `export_topic` | Return Markdown or formatted JSON using `ExportOptions`. |
| `Window` | Parse `all`, UTC dates, or nonnegative integers with `h`, `d`, `w`, `m`, `y`; months mean 30 days, years 365 days. |

# Configuration

| Option | Default | Meaning |
| :--- | :--- | :--- |
| `MailOptions.max_threads` | `25` | Positive budget of attempted uncached downloads. |
| `MailOptions.since`, `since_explicit` | `4w`, `false` | Window and whether it overrides the warm-cache watermark. |
| `MailOptions.force_full` | `false` | Re-fetch threads and bypass the watermark. |
| `CalendarOptions.calendar_id` | `primary` | Calendar to synchronize. |
| `CalendarOptions.past`, `future` | `4w`, `4w` | Full-sync bounds. |
| `CalendarOptions.window_explicit`, `force_full` | `false` | Replace the stored scope instead of using its cursor. |
| `ExportOptions.human_only`, `since`, `format` | `true`, `4w`, Markdown | Timeline filtering and output. |

# Compatibility

Opening an original Go cache performs a transactional, one-way migration, preserving records and decoding the Go SQLite driver's timestamps. The resulting cache supports multiple topic memberships and calendar-scoped event IDs. Message JSON exposes `topic_ids` rather than singular `topic_id`. Keep a copy if the cache still needs to be used by the Go application. Future schemas and invalid legacy relationships fail without committing a migration.

Verification covers Linux subprocesses, SQLite and local fixtures. Live Google accounts and macOS/Windows subprocess behavior remain unverified. [DEPENDENCIES.md](DEPENDENCIES.md) records source provenance and dependency choices.

# License

MIT — [LICENSE](LICENSE).
