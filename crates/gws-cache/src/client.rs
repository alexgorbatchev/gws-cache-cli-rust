use crate::{
    Error, Result,
    model::{CalendarParams, EventPage, ThreadDetail, ThreadPage, ThreadSummary},
};
use serde_json::{Value, json};
use std::{collections::HashSet, path::PathBuf, process::Stdio, time::Duration};
use tokio::{io::AsyncReadExt, process::Command};
use tokio_util::sync::CancellationToken;

/// Uses the installed Google Workspace CLI's authentication and native JSON
/// interface. A nonzero deadline applies to each invocation, including pipe
/// drainage; zero leaves the invocation bounded only by cancellation.
pub struct Client {
    program: PathBuf,
    timeout: Duration,
}
impl Client {
    pub fn new(program: PathBuf, timeout: Duration) -> Result<Self> {
        Ok(Self { program, timeout })
    }
    pub async fn list_threads(
        &self,
        query: &str,
        cancel: &CancellationToken,
    ) -> Result<Vec<ThreadSummary>> {
        let mut combined = Vec::new();
        let mut token = String::new();
        let mut seen = HashSet::new();
        for _ in 0..200 {
            let mut params = json!({"userId":"me","q":query});
            if !token.is_empty() {
                params["pageToken"] = token.clone().into();
            }
            let page: ThreadPage = serde_json::from_slice(
                &self
                    .run(&["gmail", "users", "threads", "list"], &params, cancel)
                    .await?,
            )?;
            combined.extend(page.threads);
            if page.next_page_token.is_empty() {
                return Ok(combined);
            }
            token = page.next_page_token;
            if !seen.insert(token.clone()) {
                return Err(Error::Pagination("repeated Gmail page token".into()));
            }
        }
        Err(Error::Pagination("Gmail exceeded 200 pages".into()))
    }
    pub async fn get_thread(&self, id: &str, cancel: &CancellationToken) -> Result<ThreadDetail> {
        Ok(serde_json::from_slice(
            &self
                .run(
                    &["gmail", "users", "threads", "get"],
                    &json!({"userId":"me","id":id}),
                    cancel,
                )
                .await?,
        )?)
    }
    pub async fn calendar_events(
        &self,
        mut params: CalendarParams,
        cancel: &CancellationToken,
    ) -> Result<EventPage> {
        if params
            .sync_token
            .as_ref()
            .is_some_and(|token| !token.is_empty())
            && (params.time_min.is_some() || params.time_max.is_some())
        {
            return Err(Error::Invalid(
                "Calendar syncToken cannot be combined with timeMin or timeMax".into(),
            ));
        }
        let mut combined = EventPage::default();
        let mut seen = HashSet::new();
        params.single_events = true;
        params.show_deleted = true;
        if params.calendar_id.is_empty() {
            params.calendar_id = "primary".into();
        }
        for _ in 0..200 {
            let page: EventPage = serde_json::from_slice(
                &self
                    .run(
                        &["calendar", "events", "list"],
                        &serde_json::to_value(&params)?,
                        cancel,
                    )
                    .await?,
            )?;
            combined.items.extend(page.items);
            if page.next_page_token.is_empty() {
                combined.next_sync_token = page.next_sync_token;
                return Ok(combined);
            }
            if !seen.insert(page.next_page_token.clone()) {
                return Err(Error::Pagination("repeated Calendar page token".into()));
            }
            params.page_token = Some(page.next_page_token);
        }
        Err(Error::Pagination("Calendar exceeded 200 pages".into()))
    }
    async fn run(
        &self,
        resource: &[&str],
        params: &Value,
        cancel: &CancellationToken,
    ) -> Result<Vec<u8>> {
        if cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        let mut command = Command::new(&self.program);
        command
            .args(resource)
            .arg("--params")
            .arg(serde_json::to_string(params)?)
            .args(["--format", "json"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        #[cfg(unix)]
        command.process_group(0);
        let mut child = command.spawn()?;
        let pid = child.id();
        let mut stdout = child.stdout.take().expect("piped stdout");
        let mut stderr = child.stderr.take().expect("piped stderr");
        let mut output = Vec::new();
        let mut diagnostics = Vec::new();
        let result = {
            let capture = async {
                let (status, out, err) = tokio::join!(
                    child.wait(),
                    stdout.read_to_end(&mut output),
                    stderr.read_to_end(&mut diagnostics)
                );
                out?;
                err?;
                Ok::<_, Error>(status?)
            };
            let deadline = async {
                if self.timeout.is_zero() {
                    std::future::pending::<()>().await;
                } else {
                    tokio::time::sleep(self.timeout).await;
                }
            };
            tokio::select! {
                biased;
                () = cancel.cancelled() => Err(Error::Cancelled),
                () = deadline => Err(Error::Timeout),
                result = capture => result,
            }
        };
        let status = match result {
            Ok(status) => status,
            Err(error) => {
                #[cfg(unix)]
                if let Some(pid) = pid.and_then(|p| i32::try_from(p).ok()) {
                    let _ = nix::sys::signal::killpg(
                        nix::unistd::Pid::from_raw(pid),
                        nix::sys::signal::Signal::SIGKILL,
                    );
                }
                #[cfg(not(unix))]
                let _ = pid;
                let _ = child.start_kill();
                let _ = tokio::time::timeout(Duration::from_secs(2), child.wait()).await;
                return Err(error);
            }
        };
        let structured = [&output, &diagnostics].into_iter().find_map(|bytes| {
            serde_json::from_slice::<Value>(bytes)
                .ok()
                .and_then(|v| v.get("error").cloned())
        });
        if !status.success() || structured.is_some() {
            let code = structured
                .as_ref()
                .and_then(|e| e.get("code"))
                .and_then(Value::as_u64)
                .and_then(|n| u16::try_from(n).ok());
            let message = structured
                .as_ref()
                .and_then(|e| e.get("message"))
                .and_then(Value::as_str)
                .map(String::from)
                .unwrap_or_else(|| String::from_utf8_lossy(&diagnostics).trim().to_owned());
            return Err(Error::Workspace {
                exit: status.code(),
                code,
                message,
            });
        }
        Ok(output)
    }
}
