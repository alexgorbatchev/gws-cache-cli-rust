Search and export Gmail and Google Calendar from a local SQLite cache.

# What It Does

- Tracks topics backed by Gmail queries and caches thread snippets.
- Synchronizes calendar events, including updates and cancellations.
- Searches and exports saved records offline in human or agent mode.

# How It Works

- Add a topic, then sync it through your authenticated `gws` installation.
- Repeat bounded mail syncs to collect remaining threads; unchanged histories reuse cached records.
- Search saved mail and events or export a chronological topic timeline.

# How it Really Works

- Every domain command opens the selected SQLite database, creating it when missing. The default is `$XDG_CACHE_HOME/gws-cache/cache.db` or the platform cache directory. This local cache contains email addresses, snippets and calendar details.
- Online commands execute `gws`; offline searches, topic management, exports and status do not request cloud data. Gmail full message bodies and attachments are not downloaded.
- Mail classification reads optional `gws-cache/classifier.json` under the XDG/platform config directory. Invalid custom rules fail mail sync while offline commands remain usable.
- Mail sync limits uncached fetch attempts to 25 per topic/query by default. Remaining work is explicit; failed downloads remain in the backlog. Topics can share the same cached thread.
- Calendar records and their cursor commit atomically. Explicit windows replace warm cursor scope; only HTTP 410 expires a cursor. A failed replacement after 410 leaves that calendar empty.
- Opening a Go database migrates it transactionally to the Rust schema. This is one-way: back up the cache before switching. Removing topics retains cached records.
- Human output uses tables; `AGENT=1` uses compact key:value records. Text records and statuses neutralize control characters; cached data and JSON retain their contents. JSON stdout remains one document, including empty arrays. Warnings/errors go to stderr. Exit 0 includes bounded partial thread sync, 1 reports failures, and 130 reports local cancellation.
- Nonpositive `--timeout` disables the deadline. Positive deadlines apply to each subprocess, including pipe drainage. Unix cancellation terminates the subprocess group; non-Unix cancellation stops the direct child.

# Installation

The Rust migration is local and has no published release. No Rust download artifact or installation URL is available yet.

# Setup

- [Google Workspace CLI](https://github.com/googleworkspace/cli) — install and authenticate `gws` for Gmail and Calendar sync. Offline operations require no Google credentials.

# Quick Start

```sh
gws-cache topic add work --name Work --query 'label:work'
AGENT=1 gws-cache topic list
gws-cache message sync work --format json
gws-cache message search planning --topic work --since all --format json
gws-cache message export work --since all
gws-cache event sync --past 30d --future 8w
```

Sample Output after adding the topic, as verified by the offline fixture:

```text
slug:work	name:Work	search:label:work	last_synced:never
```

# Options & Flags

| Flag | Short | Default | Description |
| :--- | :--- | :--- | :--- |
| `--db <path>` | — | Platform cache path | SQLite cache file; explicit XDG cache directory takes priority. |
| `--timeout <duration>` | — | `2m` | Per-request Go duration; nonpositive disables deadline. |
| `--version` | — | `false` | Print raw version and exit. |
| `--help` | `-h` | `false` | Print help without opening the cache. |

`gws-cache topic add`

| Flag | Short | Default | Description |
| :--- | :--- | :--- | :--- |
| `--name` | — | Empty | Display name, falling back to topic slug. |
| `--query` | — | Empty | Gmail query, falling back to display name. |

`gws-cache message sync`

| Flag | Short | Default | Description |
| :--- | :--- | :--- | :--- |
| `--all` | — | `false` | Sync every tracked topic. |
| `--query` | — | Unset | Ad-hoc query; choose this, a topic, or all. |
| `--force-full` | — | `false` | Refetch unchanged matching threads. |
| `--since` | — | `4w` | Lookback window; explicit values replace warm state. |
| `--max-threads` | — | `25` | Positive uncached fetch-attempt budget per topic/query. |
| `--format` | — | `table` | Table or JSON. |

`gws-cache message search`

| Flag | Short | Default | Description |
| :--- | :--- | :--- | :--- |
| `--topic` | — | Unset | Topic filter. |
| `--since` | — | `14d` | Lookback window. |
| `--human-only` | — | `false` | Select human email. |
| `--format` | — | `table` | Table or JSON. |

`gws-cache message export`

| Flag | Short | Default | Description |
| :--- | :--- | :--- | :--- |
| `--human-only` | — | `true` | Select human email; `=false` includes automated mail. |
| `--since` | — | `4w` | Lookback window. |
| `--format` | — | `markdown` | Markdown or JSON. |

`gws-cache event sync`

| Flag | Short | Default | Description |
| :--- | :--- | :--- | :--- |
| `--calendar-id` | — | `primary` | Calendar to cache. |
| `--past` | — | `4w` | Past window. |
| `--future` | — | `4w` | Future window. |
| `--force-full` | — | `false` | Replace cached scope. |
| `--format` | — | `table` | Table or JSON. |

`gws-cache event search`

| Flag | Short | Default | Description |
| :--- | :--- | :--- | :--- |
| `--calendar-id` | — | `primary` | Calendar filter. |
| `--topic` | — | Unset | Matched-topic filter. |
| `--since` | — | `4w` | Lookback window. |
| `--format` | — | `table` | Table or JSON. |

Window syntax is `all`, UTC `YYYY-MM-DD`, or integer h/d/w/m/y (30-day months, 365-day years). Optional booleans accept bare flags or `=true`/`=false`.

# Known Limits

Local SQLite and subprocess checks run on Linux. Live Google authentication, real user caches, and macOS/Windows runtime remain pending. The [usage skill](skills/gws-cache/SKILL.md) is canonical; its compiled GitHub installation pointer is awaiting publication. The [library](crates/gws-cache/README.md) documents migration and storage details.

# License

MIT License, copyright 2026 Alex Gorbatchev. See [LICENSE](LICENSE).
