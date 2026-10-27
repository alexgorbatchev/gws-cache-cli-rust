#![doc = include_str!("../README.md")]
mod calendar;
mod classifier;
mod client;
mod export;
mod mail;
mod message;
pub mod model;
mod store;
mod window;

pub use calendar::{CalendarOptions, CalendarSyncResult, sync_calendar};
pub use classifier::{Classification, Classifier, Rules};
pub use client::Client;
pub use export::{ExportFormat, ExportOptions, export_topic};
pub use mail::{MailOptions, sync_all_topics, sync_query, sync_topic};
pub use message::convert_thread;
pub use store::{EventFilter, MessageFilter, Store};
pub use window::Window;

/// A failure whose class is preserved independently of its diagnostic text.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Invalid(String),
    #[error("topic not found: {0}")]
    TopicNotFound(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error(transparent)]
    Regex(#[from] regex::Error),
    #[error("Google Workspace request failed (exit {exit:?}, HTTP {code:?}): {message}")]
    Workspace {
        exit: Option<i32>,
        code: Option<u16>,
        message: String,
    },
    #[error("Google Workspace request timed out")]
    Timeout,
    #[error("operation cancelled")]
    Cancelled,
    #[error("pagination did not terminate: {0}")]
    Pagination(String),
    #[error("{operation}; additionally failed to record the sync outcome: {record}")]
    Recording {
        operation: Box<Error>,
        record: Box<Error>,
    },
}

pub type Result<T> = std::result::Result<T, Error>;
