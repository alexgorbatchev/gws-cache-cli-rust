---
created_on: 2026-10-14 18:22
last_modified: 2026-10-14 22:40
status: current
---

# Dependency replacements

Source: `alexgorbatchev/gws-cache-cli`, MIT copyright 2026 Alex Gorbatchev. Its reusable implementation lives in the internal `crates/gws-cache` package in this repository's Cargo workspace.

| Go dependency/primitive | Native Rust replacement | Purpose |
| :--- | :--- | :--- |
| Cobra/pflag | Clap 4 and clap_complete | Native parsing, typed validation, command tree and shell completion. |
| cobra-help-tree | `cli-command-kit` | Shared human/agent help, installation pointer and home folding. |
| Table rendering | comfy-table 7 | Human terminal tables; agent records use direct formatted writes. |
| time.Duration | go-duration 0.2 / std::time::Duration | Parse Go duration syntax; nonpositive values disable request deadlines. |
| XDG/os.UserConfigDir | dirs 6 and standard environment access | Platform cache/config/home directories, retaining explicit XDG priority. |
| context/signal | Tokio 1 / tokio-util CancellationToken | Native asynchronous SIGINT/SIGTERM cancellation. |
| encoding/json, time | Serde/serde_json, Chrono | Native JSON and UTC timestamps. |
| modernc SQLite, regex, mail parsing, exec | internal `crates/gws-cache` | rusqlite, regex, mailparse, native Tokio subprocesses and Unix process groups; see its dependency notes. |

The executable `gws` remains an external runtime dependency for online sync and owns authentication. No Go runtime or authentication shim is included. Native crate versions and checksums are retained in the workspace's root `Cargo.lock`.
