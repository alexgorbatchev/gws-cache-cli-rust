---
name: gws-cache
description: Use when synchronizing Gmail threads or Google Calendar events into the local gws-cache SQLite cache, managing tracked topics, or searching and exporting the cached records offline.
author: alexgorbatchev
metadata:
  created_on: 2026-10-14 18:22
  last_modified: 2026-10-14 18:36
  status: current
---

Set `AGENT=1`, `true`, or `yes` for compact key:value records and status prefixes. This guide describes the local Rust implementation; its binary and skill publication are pending.

## Global interface

`gws-cache` shows help. `gws-cache topic`, `gws-cache message`, `gws-cache event` and `gws-cache cache` require a verb. Flags inherited by every command can appear before or after subcommands.

| Flag | Type | Default | Contract |
| :--- | :--- | :--- | :--- |
| `--db` | path | unset | `$XDG_CACHE_HOME/gws-cache/cache.db`, otherwise the platform cache directory with `gws-cache/cache.db`, then `~/.cache/gws-cache/cache.db`, then `./cache.db`. Nonempty XDG values take priority. |
| `--timeout` | duration | `2m` | Per `gws` subprocess, including pipe drainage. Go durations accept composite/fractional values and ns/us/µs/μs/ms/s/m/h. Zero and negative values disable the deadline; cancellation remains active. |
| `--version` | bool | `false` | Root only: raw `0.1.0` and one newline, without opening the cache. |
| `--help`, `-h` | bool | `false` | Help without cache or network access. |

A window value accepts `all` (or an empty value), UTC `YYYY-MM-DD`, or a nonnegative integer with h/d/w/m/y; a month is 30 days and a year is 365 days. Relative lookbacks use current UTC time. Calendar future windows add the duration; absolute dates remain fixed boundaries. An explicit window bypasses saved incremental state for the affected sync.

## Results, files and errors

Domain results go to stdout, warnings and one final error to stderr. Human results use ASCII tables and `[OK]`, `[WARN]`, `[ERR]`; agent records are tab-separated key:value fields. Text records and statuses flatten embedded line breaks/tabs and remove other control characters. JSON results preserve cached field contents and remain a single valid JSON document, including `[]` for empty searches. Displayed filesystem paths fold home to `~`.

All domain commands open the SQLite cache, creating missing parent directories and the database. Unix newly created parent directories are private (0700) and the database starts 0600. Existing permissions remain unchanged. The cache stores Gmail snippets, email addresses, classifications and calendar details; complete MIME bodies are not downloaded. Opening an existing Go cache performs a transactional one-way migration. Use a backup before switching implementations; the Go application cannot read the Rust schema.

Sync commands execute the installed `gws` with native Gmail/Calendar arguments and JSON parameters, inheriting its authentication environment. They read cloud data and update the local cache; offline topic/search/export/status commands make no requests. Mail sync alone loads additional sender/snippet regexes from `$XDG_CONFIG_HOME/gws-cache/classifier.json`, otherwise the platform config directory, `~/.config/gws-cache/classifier.json`, or `./classifier.json`. A missing file uses built-ins; malformed JSON/regexes fail mail sync. Keys are `sender_patterns` and `snippet_patterns`, both string arrays appended to defaults.

Exit 0 means successful execution, including a bounded partial thread sync whose result reports remaining work. Exit 1 means validation, file, rules, subprocess, timeout or output failure, including any failed topic in `--all`. Exit 130 means local cancellation. SIGINT and Unix SIGTERM cancel requests and kill/reap their Unix subprocess group; Windows terminates the direct child. Sync failures can leave earlier successfully cached threads/topics; inspect search/status before deciding what to rerun. Calendar results and cursors commit atomically. HTTP 410 clears an expired calendar before refetching; if refetching fails, that calendar remains empty.

## `gws-cache topic add <TOPIC>`

Creates a tracked topic and its initial query. `<TOPIC>` contains text and uniquely identifies the topic. `--name` (string, default empty) uses the slug when empty. `--query` (string, default empty) uses the display name when empty. Existing slugs fail. Prints a tracking status.

## `gws-cache topic list`

Lists topics by slug, with `slug`, `name`, `search`, `last_synced`; unsynced topics show `never`. Agent timestamps are RFC3339; human timestamps are UTC YYYY-MM-DD HH:MM. Empty results print `No topics tracked yet.`

## `gws-cache topic rm <TOPIC>`

Alias: `gws-cache topic remove`. Removes the topic and its memberships/queries. Cached mail and calendar records remain available. Missing topics fail. Prints a removal status.

## `gws-cache message sync [TOPIC]`

Exactly one target is required: `[TOPIC]`, `--all`, or `--query`. A new topic target is automatically registered using its slug as display name/query.

| Flag | Type | Default | Contract |
| :--- | :--- | :--- | :--- |
| `--all` | bool | `false` | Sync tracked topics in slug order; ordinary failures do not stop later topics. |
| `--query` | string | unset | Nonempty ad-hoc Gmail search, without creating a topic. |
| `--force-full` | bool | `false` | Refetch matching threads even when their history is already cached. |
| `--since` | value | `4w` | Cold/full lookback. Explicit values replace the saved watermark; warm defaults use a 24-hour overlap. |
| `--max-threads` | uint32 | `25` | Positive maximum uncached fetch attempts per topic/query. Failed detail requests consume budget and remain in the backlog. |
| `--format` | string | `table` | `table` or `json`. |

JSON returns one sync object for a topic/query and an array for `--all`, including an empty array. Fields: target, threads_listed, threads_fetched, threads_remaining, messages_ingested, max_history_id, optional warnings and error. Table mode reports remaining work before the success line; an empty `--all` prints `No topics tracked yet.` Warnings use stderr. Repeating a bounded sync drains the backlog. Any topic error in `--all` gives exit 1 after reporting all outcomes. Unchanged threads can acquire membership in additional topics without redownload.

## `gws-cache message search [WORDS]`

Searches literal text in cached sender, subject and snippet. `[WORDS]` is one optional string; quote multiword searches. `--topic` (string, default empty/unset) filters membership. `--since` (value, default `14d`) sets the lookback. `--human-only` (bool, default `false`) selects human email; `--human-only=false` includes automated mail. `--format` (string, default `table`) accepts table/json. Rows are newest first, with date/from/subject/snippet. JSON returns complete cached message records, including the many-to-many `topic_ids` array.

## `gws-cache message export <TOPIC>`

Exports a chronological topic timeline. `--human-only` (bool, default `true`) selects human email; use `--human-only=false` for all mail. `--since` (value, default `4w`) sets the lookback. `--format` (string, default `markdown`) accepts markdown/json. Markdown decodes entities and flattens snippet newlines. JSON returns an array containing id/thread_id/date_iso/date_display/from/subject/snippet/is_automated. Empty exports retain the timeline heading or return `[]`. Missing topics fail.

## `gws-cache event sync`

Fetches the selected calendar using single-event expansion and cancelled records. `--calendar-id` (string, default `primary`) selects a calendar. `--past` and `--future` (value, defaults `4w` and `4w`) set bounds; an end at or before the start fails before cache access. `--force-full` (bool, default `false`) replaces that calendar's cached scope. Explicit `--past`/`--future` also request a fresh window. Otherwise a saved cursor fetches changes; only typed HTTP 410 expires it. `--format` (string, default `table`) accepts table/json. JSON fields: calendar_id/events_fetched/events_ingested/events_deleted. Sparse cancellations preserve known event details; unknown cancellations have nullable times. IDs are unique within each calendar.

## `gws-cache event search [WORDS]`

Searches literal cached summary/description/location/organizer text, oldest first. `--calendar-id` (string, default `primary`) selects a calendar. `--topic` (string, default empty/unset) filters the topic matched at sync. `--since` (value, default `4w`) sets the lookback; use all to include unknown-time cancellations. `--format` (string, default `table`) accepts table/json. Table fields: date/time/summary/status/location. All-day dates retain their civil date and an empty time; unknown dates show `unknown`. Agent timed-event dates use RFC3339, human dates YYYY-MM-DD. JSON returns complete cached event records, with nullable timestamps and `all_day`.

## `gws-cache cache status`

Prints location/topics/conversations/messages/calendar_events. Counts cover the entire cache, including mail whose former topic was removed.

## `gws-cache completion <SHELL>`

Prints a shell completion script for bash/zsh/fish/powershell/elvish. No cache or API access.

## `gws-cache skill`

Prints the compiled Skills CLI installation command and source repository URL identically in both modes, without file or network access. Accepts no extra arguments. Rust skill publication at that URL is pending.

## Help commands

`gws-cache help [COMMAND]...` prints help for the specified command path. Generated forms include `gws-cache help topic`, `gws-cache help topic add`, `gws-cache help topic list`, `gws-cache help topic rm`, `gws-cache help message`, `gws-cache help message sync`, `gws-cache help message search`, `gws-cache help message export`, `gws-cache help event`, `gws-cache help event sync`, `gws-cache help event search`, `gws-cache help cache`, `gws-cache help cache status`, `gws-cache help completion`, `gws-cache help skill`, and `gws-cache help help`. `--help` is available on all command paths. Agent help begins with the skill alert.

Subject-local generated help forms are `gws-cache topic help`, `gws-cache topic help add`, `gws-cache topic help list`, `gws-cache topic help rm`, `gws-cache topic help help`; `gws-cache message help`, `gws-cache message help sync`, `gws-cache message help search`, `gws-cache message help export`, `gws-cache message help help`; `gws-cache event help`, `gws-cache event help sync`, `gws-cache event help search`, `gws-cache event help help`; and `gws-cache cache help`, `gws-cache cache help status`, `gws-cache cache help help`.

## Workflows

```sh
AGENT=1 gws-cache topic add work --name Work --query 'label:work'
AGENT=1 gws-cache message sync work --max-threads 25 --format json
AGENT=1 gws-cache message search 'planning' --topic work --since all --human-only --format json
AGENT=1 gws-cache message export work --since all --format markdown
AGENT=1 gws-cache event sync --past 30d --future 8w --format json
AGENT=1 gws-cache event search --since all --format json
AGENT=1 gws-cache cache status
```
