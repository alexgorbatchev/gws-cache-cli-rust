#![cfg(unix)]
use gws_cache::{Client, Error, model::CalendarParams};
use std::{
    os::unix::fs::PermissionsExt,
    time::{Duration, Instant},
};
use tokio_util::sync::CancellationToken;

// Serialize executable fixture publication and execution across tests.
static EXECUTABLE_FIXTURES: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn executable(temp: &tempfile::TempDir, script: &str) -> std::path::PathBuf {
    let path = temp.path().join("gws");
    let staged = temp.path().join("gws.next");
    std::fs::write(&staged, format!("#!/bin/sh\n{script}\n")).unwrap();
    std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o700)).unwrap();
    std::fs::rename(staged, &path).unwrap();
    path
}

#[tokio::test]
async fn follows_empty_pages_and_preserves_typed_workspace_errors() {
    let _fixture_lock = EXECUTABLE_FIXTURES.lock().await;
    let temp = tempfile::tempdir().unwrap();
    let path = executable(
        &temp,
        "case \"$*\" in\n*next*) printf '%s\\n' '{\"threads\":[{\"id\":\"t\",\"historyId\":\"99\"}]}' ;;\n*) printf '%s\\n' '{\"threads\":[],\"nextPageToken\":\"next\"}' ;;\nesac",
    );
    let client = Client::new(path.clone(), Duration::from_secs(2)).unwrap();
    let cancel = CancellationToken::new();
    assert_eq!(
        client.list_threads("subject:hello", &cancel).await.unwrap()[0].id,
        "t"
    );
    executable(&temp, "printf '%s\\n' '{\"id\":\"t\",\"messages\":[]}'");
    assert_eq!(client.get_thread("t", &cancel).await.unwrap().id, "t");
    executable(
        &temp,
        "printf '%s\\n' '{\"error\":{\"code\":410,\"message\":\"Sync token expired\",\"reason\":\"gone\"}}'; exit 1",
    );
    assert!(matches!(
        client
            .calendar_events(CalendarParams::default(), &cancel)
            .await,
        Err(Error::Workspace {
            code: Some(410),
            exit: Some(1),
            ..
        })
    ));
    executable(
        &temp,
        "printf '%s\\n' '{\"error\":{\"code\":403,\"message\":\"Forbidden\"}}' >&2; exit 1",
    );
    assert!(matches!(
        client.list_threads("", &cancel).await,
        Err(Error::Workspace {
            code: Some(403),
            ..
        })
    ));
    executable(&temp, "printf '%s\\n' 'discovery unavailable' >&2; exit 4");
    let err = client.list_threads("", &cancel).await.unwrap_err();
    assert!(matches!(
        &err,
        Error::Workspace {
            code: None,
            exit: Some(4),
            ..
        }
    ));
    assert!(err.to_string().contains("discovery unavailable"));
    executable(
        &temp,
        "printf '%s\\n' '{\"error\":{\"code\":401,\"message\":\"Authentication missing\"}}'",
    );
    assert!(matches!(
        client.list_threads("", &cancel).await,
        Err(Error::Workspace {
            code: Some(401),
            ..
        })
    ));
    executable(&temp, "printf '%s\\n' 'not json'");
    assert!(matches!(
        client.get_thread("t", &cancel).await,
        Err(Error::Json(_))
    ));
}

#[tokio::test]
async fn pagination_cycles_fail_and_calendar_keeps_the_final_cursor() {
    let _fixture_lock = EXECUTABLE_FIXTURES.lock().await;
    let temp = tempfile::tempdir().unwrap();
    let path = executable(
        &temp,
        "printf '%s\\n' '{\"threads\":[{\"id\":\"t\"}],\"nextPageToken\":\"repeat\"}'",
    );
    let client = Client::new(path, Duration::from_secs(2)).unwrap();
    let cancel = CancellationToken::new();
    assert!(matches!(
        client.list_threads("", &cancel).await,
        Err(Error::Pagination(_))
    ));
    executable(
        &temp,
        "case \"$*\" in\n*next*) printf '%s\\n' '{\"items\":[{\"id\":\"e\"}],\"nextSyncToken\":\"cursor\"}' ;;\n*) printf '%s\\n' '{\"items\":[],\"nextPageToken\":\"next\"}' ;;\nesac",
    );
    let page = client
        .calendar_events(CalendarParams::default(), &cancel)
        .await
        .unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.next_sync_token, "cursor");
    executable(
        &temp,
        "printf '%s\\n' '{\"items\":[],\"nextPageToken\":\"repeat\"}'",
    );
    assert!(matches!(
        client
            .calendar_events(CalendarParams::default(), &cancel)
            .await,
        Err(Error::Pagination(_))
    ));
}

#[tokio::test]
async fn deadline_and_cancellation_terminate_inherited_pipe_holders() {
    let _fixture_lock = EXECUTABLE_FIXTURES.lock().await;
    let temp = tempfile::tempdir().unwrap();
    let script = "sleep 60 &\nprintf '%s\\n' '{\"threads\":[]}'\nwait";
    let path = executable(&temp, script);
    let client = Client::new(path.clone(), Duration::from_millis(80)).unwrap();
    let start = Instant::now();
    assert!(matches!(
        client.list_threads("", &CancellationToken::new()).await,
        Err(Error::Timeout)
    ));
    assert!(start.elapsed() < Duration::from_secs(3));
    let client = Client::new(path, Duration::from_secs(20)).unwrap();
    let cancel = CancellationToken::new();
    let trigger = cancel.clone();
    let task = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        trigger.cancel();
    });
    assert!(matches!(
        client.list_threads("", &cancel).await,
        Err(Error::Cancelled)
    ));
    task.await.unwrap();
    executable(
        &temp,
        "sleep 60 &\nprintf '%s\\n' '{\"threads\":[]}'\nexit 0",
    );
    let client = Client::new(temp.path().join("gws"), Duration::from_millis(80)).unwrap();
    assert!(matches!(
        client.list_threads("", &CancellationToken::new()).await,
        Err(Error::Timeout)
    ));
    let missing = Client::new(temp.path().join("missing"), Duration::from_secs(1)).unwrap();
    assert!(matches!(
        missing.list_threads("", &CancellationToken::new()).await,
        Err(Error::Io(_))
    ));
    assert!(matches!(
        client.list_threads("", &cancel).await,
        Err(Error::Cancelled)
    ));
}

#[tokio::test]
async fn zero_deadline_still_waits_for_output_and_remains_cancellable() {
    let _fixture_lock = EXECUTABLE_FIXTURES.lock().await;
    let temp = tempfile::tempdir().unwrap();
    let path = executable(&temp, "sleep 0.05; printf '%s\\n' '{\"threads\":[]}'");
    let client = Client::new(path.clone(), Duration::ZERO).unwrap();
    assert!(
        client
            .list_threads("", &CancellationToken::new())
            .await
            .unwrap()
            .is_empty()
    );
    executable(&temp, "sleep 60");
    let cancel = CancellationToken::new();
    let trigger = cancel.clone();
    let task = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        trigger.cancel();
    });
    assert!(matches!(
        client.list_threads("", &cancel).await,
        Err(Error::Cancelled)
    ));
    task.await.unwrap();
}
