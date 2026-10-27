#![cfg(unix)]
use serde_json::{Value, json};
use std::{
    os::unix::fs::PermissionsExt,
    path::Path,
    process::{Command, Output},
};

fn fixture() -> tempfile::TempDir {
    std::fs::create_dir_all(".tmp").unwrap();
    tempfile::Builder::new()
        .prefix("process-")
        .tempdir_in(".tmp")
        .unwrap()
}
fn executable(dir: &Path, script: &str) {
    let path = dir.join("gws.next");
    std::fs::write(&path, format!("#!/bin/sh\n{script}\n")).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
    std::fs::rename(path, dir.join("gws")).unwrap();
}
fn command(dir: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_gws-cache"));
    let mut paths = vec![dir.to_path_buf()];
    paths.extend(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    ));
    command
        .env("PATH", std::env::join_paths(paths).unwrap())
        .env("XDG_CONFIG_HOME", dir.join("config"))
        .env("AGENT", "1")
        .args(["--db", dir.join("cache.db").to_str().unwrap()])
        .args(args);
    command
}
fn run(dir: &Path, args: &[&str]) -> Output {
    command(dir, args).output().unwrap()
}
fn okay(output: Output) -> String {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}
fn json_output(dir: &Path, args: &[&str]) -> Value {
    serde_json::from_str(&okay(run(dir, args))).unwrap()
}
fn fixture_script() -> String {
    let detail = json!({"id":"thread","historyId":"100","messages":[{"id":"message","internalDate":"1780300800000","snippet":"Some &amp; details\nnext","payload":{"headers":[{"name":"From","value":"Alice <alice@example.org>"},{"name":"Subject","value":"Work details"}]}},{"id":"auto","internalDate":"1780300800001","snippet":"receipt","payload":{"headers":[{"name":"From","value":"noreply@example.org"}]}}]});
    format!(
        "printf '%s\\n' \"$*\" >> \"$XDG_CONFIG_HOME/../calls\"\ncase \"$*\" in\n*'threads list'*) printf '%s\\n' '{{\"threads\":[{{\"id\":\"thread\",\"historyId\":\"100\"}}]}}' ;;\n*'threads get'*) printf '%s\\n' '{detail}' ;;\n*'calendar events list'*) printf '%s\\n' '{{\"items\":[{{\"id\":\"event\",\"summary\":\"Work day\",\"status\":\"confirmed\",\"location\":\"Office\",\"start\":{{\"date\":\"2026-06-01\"}},\"end\":{{\"date\":\"2026-06-02\"}}}},{{\"id\":\"cancelled\",\"status\":\"cancelled\"}}],\"nextSyncToken\":\"cursor\"}}' ;;\nesac"
    )
}

#[test]
fn gmail_sync_search_export_and_incremental_flags_use_real_subprocesses() {
    let temp = fixture();
    let dir = temp.path().canonicalize().unwrap();
    std::fs::create_dir_all(dir.join("config")).unwrap();
    executable(&dir, &fixture_script());
    okay(run(&dir, &["topic", "add", "work", "--name", "Work"]));
    let result = json_output(
        &dir,
        &[
            "message",
            "sync",
            "work",
            "--since",
            "all",
            "--format",
            "json",
            "--timeout",
            "0",
        ],
    );
    assert_eq!(result["messages_ingested"], 2);
    assert_eq!(
        json_output(
            &dir,
            &[
                "message",
                "search",
                "--since",
                "all",
                "--human-only",
                "--format",
                "json"
            ]
        )
        .as_array()
        .unwrap()
        .len(),
        1
    );
    let table = okay(run(
        &dir,
        &["message", "search", "--since", "all", "--human-only=false"],
    ));
    assert!(
        table.contains("from:Alice") && table.contains("Work details") && table.contains("noreply")
    );
    let mut human = command(&dir, &["message", "search", "--since", "all"]);
    human.env("AGENT", "0");
    assert!(okay(human.output().unwrap()).contains("| date"));
    let markdown = okay(run(&dir, &["message", "export", "work", "--since", "all"]));
    assert!(markdown.contains("Some & details next"));
    assert!(!markdown.contains("receipt"));
    assert_eq!(
        json_output(
            &dir,
            &[
                "message",
                "export",
                "work",
                "--since",
                "all",
                "--format",
                "json",
                "--human-only=false"
            ]
        )
        .as_array()
        .unwrap()
        .len(),
        2
    );
    assert_eq!(
        json_output(&dir, &["message", "sync", "work", "--format", "json"])["threads_fetched"],
        0
    );
    assert_eq!(
        json_output(
            &dir,
            &[
                "message",
                "sync",
                "work",
                "--force-full",
                "--since",
                "all",
                "--format",
                "json"
            ]
        )["threads_fetched"],
        1
    );
    assert_eq!(
        json_output(
            &dir,
            &[
                "message",
                "sync",
                "--query",
                "is:unread",
                "--format",
                "json"
            ]
        )["threads_fetched"],
        0
    );
    assert_eq!(
        json_output(&dir, &["message", "sync", "--all", "--format", "json"])
            .as_array()
            .unwrap()
            .len(),
        1
    );
    okay(run(&dir, &["message", "sync", "--all"]));
    assert!(okay(run(&dir, &["topic", "list"])).contains("last_synced:2026"));
    let mut human = command(&dir, &["topic", "list"]);
    human.env("AGENT", "0");
    okay(human.output().unwrap());
    assert!(okay(run(&dir, &["cache", "status"])).contains("messages:2"));
    let calls = std::fs::read_to_string(dir.join("calls")).unwrap();
    assert!(calls.contains("after:") && calls.contains("--format json"));
    okay(run(&dir, &["topic", "rm", "work"]));
    assert_eq!(
        json_output(
            &dir,
            &["message", "search", "--since", "all", "--format", "json"]
        )
        .as_array()
        .unwrap()
        .len(),
        2
    );
}

#[test]
fn calendar_explicit_windows_replace_a_warm_cursor_and_preserve_all_day_dates() {
    let temp = fixture();
    let dir = temp.path().canonicalize().unwrap();
    std::fs::create_dir_all(dir.join("config")).unwrap();
    executable(&dir, &fixture_script());
    assert_eq!(
        json_output(
            &dir,
            &[
                "event", "sync", "--past", "all", "--future", "all", "--format", "json"
            ]
        )["events_deleted"],
        1
    );
    okay(run(&dir, &["event", "sync"]));
    let calls = std::fs::read_to_string(dir.join("calls")).unwrap();
    assert!(calls.lines().last().unwrap().contains("syncToken"));
    okay(run(&dir, &["event", "sync", "--past", "30d"]));
    let calls = std::fs::read_to_string(dir.join("calls")).unwrap();
    let last = calls.lines().last().unwrap();
    assert!(last.contains("timeMin") && !last.contains("syncToken"));
    let table = okay(run(&dir, &["event", "search", "--since", "all"]));
    assert!(table.contains("date:2026-06-01\ttime:\tsummary:Work day"));
    assert!(table.contains("date:unknown"));
    assert_eq!(
        json_output(
            &dir,
            &["event", "search", "--since", "all", "--format", "json"]
        )
        .as_array()
        .unwrap()
        .len(),
        2
    );
    let mut human = command(&dir, &["event", "search", "--since", "all"]);
    human.env("AGENT", "0");
    assert!(okay(human.output().unwrap()).contains("Work day"));
    okay(run(
        &dir,
        &["event", "sync", "--force-full", "--calendar-id", "other"],
    ));
}

#[test]
fn partial_thread_failures_and_failed_topics_keep_json_clean_and_return_failure() {
    let temp = fixture();
    let dir = temp.path().canonicalize().unwrap();
    std::fs::create_dir_all(dir.join("config")).unwrap();
    executable(
        &dir,
        "case \"$*\" in\n*'threads list'*) printf '%s\\n' '{\"threads\":[{\"id\":\"a\"},{\"id\":\"b\"}]}' ;;\n*) printf '%s\\n' 'detail unavailable' >&2; exit 1 ;;\nesac",
    );
    let result = run(
        &dir,
        &[
            "message",
            "sync",
            "work",
            "--max-threads",
            "1",
            "--format",
            "json",
        ],
    );
    assert!(result.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&result.stdout).unwrap()["threads_remaining"],
        2
    );
    assert!(String::from_utf8_lossy(&result.stderr).contains("WARN:"));
    let table = okay(run(
        &dir,
        &["message", "sync", "work", "--max-threads", "1"],
    ));
    assert!(table.contains("OK:"));
    executable(
        &dir,
        "printf '%s\\n' '{\"error\":{\"code\":403,\"message\":\"Denied\"}}'; exit 1",
    );
    for format in ["table", "json"] {
        let result = run(&dir, &["message", "sync", "--all", "--format", format]);
        assert_eq!(result.status.code(), Some(1));
        let errors = String::from_utf8_lossy(&result.stderr);
        assert_eq!(errors.matches("ERR:").count(), 1);
        assert!(!errors.contains("Usage:"));
        if format == "json" {
            assert!(
                serde_json::from_slice::<Value>(&result.stdout).unwrap()[0]["error"]
                    .as_str()
                    .unwrap()
                    .contains("Denied")
            );
        }
    }
}

#[test]
fn classifier_errors_only_affect_incoming_mail_and_empty_all_is_a_json_array() {
    let temp = fixture();
    let dir = temp.path().canonicalize().unwrap();
    std::fs::create_dir_all(dir.join("config/gws-cache")).unwrap();
    executable(&dir, "printf '%s\\n' '{\"threads\":[]}'");
    assert_eq!(
        json_output(&dir, &["message", "sync", "--all", "--format", "json"]),
        json!([])
    );
    assert!(okay(run(&dir, &["message", "sync", "--all"])).contains("No topics tracked yet."));
    std::fs::write(dir.join("config/gws-cache/classifier.json"), "bad JSON").unwrap();
    let result = run(&dir, &["message", "sync", "--query", "is:unread"]);
    assert_eq!(result.status.code(), Some(1));
    assert_eq!(
        String::from_utf8_lossy(&result.stderr)
            .matches("ERR:")
            .count(),
        1
    );
    okay(run(&dir, &["cache", "status"]));
    okay(run(&dir, &["message", "search"]));
    let result = run(&dir, &["message", "export", "missing"]);
    assert_eq!(result.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&result.stderr).contains("topic not found"));
}

#[test]
fn deadlines_signals_metadata_and_offline_validation_have_native_exit_behavior() {
    let temp = fixture();
    let dir = temp.path().canonicalize().unwrap();
    std::fs::create_dir_all(dir.join("config")).unwrap();
    executable(&dir, "sleep 60");
    let result = run(
        &dir,
        &["message", "sync", "--query", "x", "--timeout", "20ms"],
    );
    assert_eq!(result.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&result.stderr).contains("timed out"));
    for signal in ["-INT", "-TERM"] {
        let child = command(&dir, &["message", "sync", "--query", "x", "--timeout", "0"])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(150));
        assert!(
            Command::new("kill")
                .args([signal, &child.id().to_string()])
                .status()
                .unwrap()
                .success()
        );
        let result = child.wait_with_output().unwrap();
        assert_eq!(result.status.code(), Some(130), "{signal}");
        assert_eq!(
            String::from_utf8_lossy(&result.stderr)
                .matches("ERR:")
                .count(),
            1
        );
    }
    assert_eq!(okay(run(&dir, &["--version"])), "0.1.0\n");
    assert!(okay(run(&dir, &["skill"])).contains("npx skills add"));
    for shell in ["bash", "zsh", "fish", "powershell", "elvish"] {
        assert!(!okay(run(&dir, &["completion", shell])).is_empty());
    }
    let no_cache = fixture();
    let mut help = command(no_cache.path(), &["--help"]);
    help.env("COLUMNS", "40").env("AGENT", "0");
    let help = okay(help.output().unwrap());
    assert!(
        help.lines().all(|line| line.chars().count() <= 40),
        "{help}"
    );
    let result = run(
        no_cache.path(),
        &[
            "event",
            "sync",
            "--past",
            "2026-12-01",
            "--future",
            "2026-01-01",
        ],
    );
    assert_eq!(result.status.code(), Some(1));
    assert!(!no_cache.path().join("cache.db").exists());
    let result = run(
        no_cache.path(),
        &["cache", "status", "--db", no_cache.path().to_str().unwrap()],
    );
    assert_eq!(result.status.code(), Some(1));
}
