use crate::{
    Classifier, Client, Error, Result, Store, Window,
    model::{MailSyncResult, Topic},
};
use chrono::{DateTime, TimeDelta, Utc};
use std::collections::HashSet;
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct MailOptions {
    pub force_full: bool,
    pub since: Window,
    pub since_explicit: bool,
    pub max_threads: usize,
}
impl Default for MailOptions {
    fn default() -> Self {
        Self {
            force_full: false,
            since: Window::Relative(TimeDelta::weeks(4)),
            since_explicit: false,
            max_threads: 25,
        }
    }
}

pub async fn sync_topic(
    store: &mut Store,
    client: &Client,
    rules: &Classifier,
    slug: &str,
    options: &MailOptions,
    cancel: &CancellationToken,
    now: DateTime<Utc>,
) -> Result<MailSyncResult> {
    validate(options, cancel)?;
    let topic = match store.topic(slug) {
        Ok(topic) => topic,
        Err(Error::TopicNotFound(_)) => store.create_topic(slug, slug, "")?,
        Err(error) => return Err(error),
    };
    run(
        store,
        client,
        rules,
        Some(&topic),
        slug,
        options,
        cancel,
        now,
    )
    .await
}
pub async fn sync_query(
    store: &mut Store,
    client: &Client,
    rules: &Classifier,
    query: &str,
    options: &MailOptions,
    cancel: &CancellationToken,
    now: DateTime<Utc>,
) -> Result<MailSyncResult> {
    if query.trim().is_empty() {
        return Err(Error::Invalid("Gmail query must contain text".into()));
    }
    run(store, client, rules, None, query, options, cancel, now).await
}
pub async fn sync_all_topics(
    store: &mut Store,
    client: &Client,
    rules: &Classifier,
    options: &MailOptions,
    cancel: &CancellationToken,
    now: DateTime<Utc>,
) -> Result<Vec<MailSyncResult>> {
    validate(options, cancel)?;
    let topics = store.topics()?;
    let mut results = Vec::new();
    for topic in topics {
        if cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        results.push(
            match sync_topic(store, client, rules, &topic.slug, options, cancel, now).await {
                Ok(result) => result,
                Err(Error::Cancelled) => return Err(Error::Cancelled),
                Err(error) => MailSyncResult {
                    target: topic.slug,
                    error: Some(error.to_string()),
                    ..Default::default()
                },
            },
        );
    }
    Ok(results)
}

// The private runner carries both topic identity and ad-hoc query text; public
// entry points retain the concrete, distinct operations above.
#[allow(clippy::too_many_arguments)]
async fn run(
    store: &mut Store,
    client: &Client,
    rules: &Classifier,
    topic: Option<&Topic>,
    target: &str,
    options: &MailOptions,
    cancel: &CancellationToken,
    now: DateTime<Utc>,
) -> Result<MailSyncResult> {
    validate(options, cancel)?;
    let topic_id = topic.map(|t| t.id);
    let run_id = store.start_run(topic_id, now)?;
    let mut result = MailSyncResult {
        target: target.into(),
        ..Default::default()
    };
    let outcome = async {
        let cutoff = if let Some(topic) = topic.filter(|t| {
            !options.force_full
                && !options.since_explicit
                && t.last_synced_at.is_some()
                && t.last_message_internal_date > 0
        }) {
            Some(
                DateTime::from_timestamp_millis(topic.last_message_internal_date)
                    .and_then(|at| at.checked_sub_signed(TimeDelta::days(1)))
                    .ok_or_else(|| {
                        Error::Invalid(
                            "stored Gmail watermark exceeds supported calendar range".into(),
                        )
                    })?,
            )
        } else {
            options.since.start(now)?
        };
        let queries = if let Some(topic) = topic {
            let queries = store.queries(topic.id)?;
            if queries.is_empty() {
                vec![if topic.query.is_empty() {
                    topic.display_name.clone()
                } else {
                    topic.query.clone()
                }]
            } else {
                queries
            }
        } else {
            vec![target.into()]
        };
        let mut seen = HashSet::new();
        let mut threads = Vec::new();
        for query in queries {
            let scoped = cutoff.map_or(query.clone(), |at| {
                format!("{query} after:{}", at.timestamp())
            });
            for summary in client.list_threads(&scoped, cancel).await? {
                if summary.id.is_empty() {
                    return Err(Error::Invalid(
                        "Gmail listing contains a thread without an ID".into(),
                    ));
                }
                if seen.insert(summary.id.clone()) {
                    threads.push(summary);
                }
            }
        }
        result.threads_listed = threads.len();
        let mut spent = 0;
        for summary in threads {
            if cancel.is_cancelled() {
                return Err(Error::Cancelled);
            }
            if !options.force_full && store.has_thread_history(&summary.id, &summary.history_id)? {
                if let Some(topic_id) = topic_id {
                    store.attach_thread(&summary.id, topic_id)?;
                }
                continue;
            }
            if spent >= options.max_threads {
                result.threads_remaining += 1;
                continue;
            }
            spent += 1;
            let detail = match client.get_thread(&summary.id, cancel).await {
                Ok(detail) => detail,
                Err(Error::Cancelled) => return Err(Error::Cancelled),
                Err(error) => {
                    result.threads_remaining += 1;
                    result
                        .warnings
                        .push(format!("thread {}: {error}", summary.id));
                    continue;
                }
            };
            if detail.id != summary.id {
                return Err(Error::Invalid(format!(
                    "Gmail returned thread {} when {} was requested",
                    detail.id, summary.id
                )));
            }
            let history = if summary.history_id.is_empty() {
                &detail.history_id
            } else {
                &summary.history_id
            };
            let newest = max_history(&result.max_history_id, &detail.history_id)?;
            result.messages_ingested +=
                store.ingest_thread(&detail, topic_id, history, &summary.snippet, rules, now)?;
            result.threads_fetched += 1;
            result.max_history_id = newest;
        }
        if let Some(topic_id) = topic_id {
            let watermark = if result.threads_remaining == 0 {
                store.max_message_date(topic_id)?
            } else {
                0
            };
            store.update_topic_state(topic_id, &result.max_history_id, watermark, now)?;
        }
        Ok(())
    }
    .await;
    let failure = outcome.as_ref().err().map(ToString::to_string);
    if let Err(record) = store.finish_run(run_id, &result, failure.as_deref(), now) {
        return Err(match outcome {
            Err(operation) => Error::Recording {
                operation: Box::new(operation),
                record: Box::new(record),
            },
            Ok(()) => record,
        });
    }
    outcome?;
    Ok(result)
}

fn max_history(left: &str, right: &str) -> Result<String> {
    let parse = |value: &str| {
        if value.is_empty() {
            Ok(0)
        } else {
            value
                .parse::<u64>()
                .map_err(|_| Error::Invalid(format!("invalid Gmail history ID {value:?}")))
        }
    };
    Ok(if parse(right)? > parse(left)? {
        right
    } else {
        left
    }
    .into())
}

fn validate(options: &MailOptions, cancel: &CancellationToken) -> Result<()> {
    if options.max_threads == 0 {
        return Err(Error::Invalid("max-threads must be positive".into()));
    }
    if cancel.is_cancelled() {
        return Err(Error::Cancelled);
    }
    Ok(())
}
