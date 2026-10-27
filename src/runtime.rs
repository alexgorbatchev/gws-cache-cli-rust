use crate::{Error, args, output};
use chrono::{DateTime, Utc};
use cli_command_kit::{
    Mode,
    clap::{ArgMatches, parser::ValueSource},
};
use gws_cache::{
    CalendarOptions, Classifier, Client, EventFilter, ExportFormat, ExportOptions, MailOptions,
    MessageFilter, Store, Window,
};
use std::{io::Write, path::PathBuf, time::Duration};
use tokio_util::sync::CancellationToken;

fn text<'a>(matches: &'a ArgMatches, key: &str) -> &'a str {
    matches.get_one::<String>(key).map_or("", String::as_str)
}
fn window(matches: &ArgMatches, key: &str) -> Window {
    *matches.get_one::<Window>(key).expect("validated window")
}
fn explicit(matches: &ArgMatches, key: &str) -> bool {
    matches.value_source(key) == Some(ValueSource::CommandLine)
}
fn date(value: Option<DateTime<Utc>>, mode: Mode) -> String {
    value.map_or_else(
        || "never".into(),
        |d| {
            if mode == Mode::Agent {
                d.to_rfc3339()
            } else {
                d.format("%Y-%m-%d %H:%M").to_string()
            }
        },
    )
}

pub async fn run(
    root: &ArgMatches,
    out: &mut dyn Write,
    err: &mut dyn Write,
    mode: Mode,
    cancel: &CancellationToken,
) -> Result<(), Error> {
    let path = root
        .get_one::<PathBuf>("db")
        .cloned()
        .unwrap_or_else(args::cache_path);
    let (subject, group) = root.subcommand().expect("required subject");
    let (verb, sub) = group.subcommand().expect("required verb");
    let now = Utc::now();
    // Cross-flag calendar validation is entirely offline and precedes file access.
    if subject == "event"
        && verb == "sync"
        && window(sub, "past")
            .start(now)?
            .zip(window(sub, "future").end(now)?)
            .is_some_and(|(s, e)| e <= s)
    {
        return Err(gws_cache::Error::Invalid(
            "calendar window end must be later than its start".into(),
        )
        .into());
    }
    if cancel.is_cancelled() {
        return Err(gws_cache::Error::Cancelled.into());
    }
    let mut store = Store::open(&path)?;
    match (subject, verb) {
        ("topic", "add") => {
            let topic =
                store.create_topic(text(sub, "topic"), text(sub, "name"), text(sub, "query"))?;
            output::status(out, mode, "OK", &format!("Tracking {}", topic.slug))?;
        }
        ("topic", "list") => output::rows(
            out,
            mode,
            &["slug", "name", "search", "last_synced"],
            store
                .topics()?
                .into_iter()
                .map(|t| {
                    vec![
                        t.slug,
                        t.display_name,
                        t.query,
                        date(t.last_synced_at, mode),
                    ]
                })
                .collect(),
            "No topics tracked yet.",
        )?,
        ("topic", "rm") => {
            store.delete_topic(text(sub, "topic"))?;
            output::status(
                out,
                mode,
                "OK",
                &format!("Removed topic {}", text(sub, "topic")),
            )?;
        }
        ("cache", "status") => {
            let stats = store.stats()?;
            output::rows(
                out,
                mode,
                &[
                    "location",
                    "topics",
                    "conversations",
                    "messages",
                    "calendar_events",
                ],
                vec![vec![
                    cli_command_kit::display_path(&path),
                    stats.topics.to_string(),
                    stats.threads.to_string(),
                    stats.messages.to_string(),
                    stats.calendar_events.to_string(),
                ]],
                "",
            )?;
        }
        ("message", "search") => {
            let messages = store.messages(&MessageFilter {
                keyword: text(sub, "words").into(),
                topic: sub.get_one::<String>("topic").cloned(),
                human_only: sub.get_flag("human-only"),
                since: window(sub, "since").start(now)?,
                ..Default::default()
            })?;
            if text(sub, "format") == "json" {
                output::json(out, &messages)?;
            } else {
                output::rows(
                    out,
                    mode,
                    &["date", "from", "subject", "snippet"],
                    messages
                        .into_iter()
                        .map(|m| {
                            vec![
                                if mode == Mode::Agent {
                                    m.date_iso
                                } else {
                                    DateTime::parse_from_rfc3339(&m.date_iso)
                                        .map_or(m.date_iso.clone(), |d| {
                                            d.format("%Y-%m-%d").to_string()
                                        })
                                },
                                if m.from_name.is_empty() {
                                    m.from_address
                                } else {
                                    m.from_name
                                },
                                m.subject,
                                m.snippet,
                            ]
                        })
                        .collect(),
                    "No saved email matches that search.",
                )?;
            }
        }
        ("message", "export") => {
            let body = gws_cache::export_topic(
                &store,
                text(sub, "topic"),
                &ExportOptions {
                    human_only: sub.get_flag("human-only"),
                    format: if text(sub, "format") == "json" {
                        ExportFormat::Json
                    } else {
                        ExportFormat::Markdown
                    },
                    since: window(sub, "since"),
                },
                now,
            )?;
            writeln!(out, "{}", body.trim_end_matches('\n'))?;
        }
        ("message", "sync") => {
            let client = Client::new(
                "gws".into(),
                *root
                    .get_one::<Duration>("timeout")
                    .expect("validated timeout"),
            )?;
            let rules = Classifier::load(&args::rules_path())?;
            let options = MailOptions {
                force_full: sub.get_flag("force-full"),
                since: window(sub, "since"),
                since_explicit: explicit(sub, "since"),
                max_threads: *sub.get_one::<u32>("max-threads").expect("validated limit") as usize,
            };
            let all = sub.get_flag("all");
            let results = if all {
                gws_cache::sync_all_topics(&mut store, &client, &rules, &options, cancel, now)
                    .await?
            } else {
                vec![if sub.contains_id("query") {
                    gws_cache::sync_query(
                        &mut store,
                        &client,
                        &rules,
                        text(sub, "query"),
                        &options,
                        cancel,
                        now,
                    )
                    .await?
                } else {
                    gws_cache::sync_topic(
                        &mut store,
                        &client,
                        &rules,
                        text(sub, "topic"),
                        &options,
                        cancel,
                        now,
                    )
                    .await?
                }]
            };
            let failures = results.iter().filter(|r| r.error.is_some()).count();
            if results.is_empty() && text(sub, "format") != "json" {
                writeln!(out, "No topics tracked yet.")?;
            }
            if text(sub, "format") == "json" {
                if all {
                    output::json(out, &results)?;
                } else {
                    output::json(out, &results[0])?;
                }
            }
            for result in results {
                for warning in result.warnings {
                    output::status(err, mode, "WARN", &warning)?;
                }
                if text(sub, "format") != "json" {
                    if let Some(error) = result.error {
                        output::status(err, mode, "WARN", &format!("{}: {error}", result.target))?;
                    } else {
                        if result.threads_remaining > 0 {
                            output::status(
                                err,
                                mode,
                                "WARN",
                                &format!(
                                    "{} threads remain for {}",
                                    result.threads_remaining, result.target
                                ),
                            )?;
                        }
                        output::status(
                            out,
                            mode,
                            "OK",
                            &format!(
                                "{}: {} threads fetched, {} messages saved",
                                result.target, result.threads_fetched, result.messages_ingested
                            ),
                        )?;
                    }
                }
            }
            if failures > 0 {
                return Err(Error::Partial(failures));
            }
        }
        ("event", "sync") => {
            let client = Client::new(
                "gws".into(),
                *root
                    .get_one::<Duration>("timeout")
                    .expect("validated timeout"),
            )?;
            let result = gws_cache::sync_calendar(
                &mut store,
                &client,
                &CalendarOptions {
                    calendar_id: text(sub, "calendar-id").into(),
                    past: window(sub, "past"),
                    future: window(sub, "future"),
                    force_full: sub.get_flag("force-full"),
                    window_explicit: explicit(sub, "past") || explicit(sub, "future"),
                },
                cancel,
                now,
            )
            .await?;
            if text(sub, "format") == "json" {
                output::json(out, &result)?;
            } else {
                output::status(
                    out,
                    mode,
                    "OK",
                    &format!(
                        "{}: {} events saved, {} cancelled",
                        result.calendar_id, result.events_ingested, result.events_deleted
                    ),
                )?;
            }
        }
        ("event", "search") => {
            let events = store.events(&EventFilter {
                keyword: text(sub, "words").into(),
                calendar: Some(text(sub, "calendar-id").into()),
                topic: sub.get_one::<String>("topic").cloned(),
                since: window(sub, "since").start(now)?,
            })?;
            if text(sub, "format") == "json" {
                output::json(out, &events)?;
            } else {
                output::rows(
                    out,
                    mode,
                    &["date", "time", "summary", "status", "location"],
                    events
                        .into_iter()
                        .map(|e| {
                            vec![
                                e.start_time.map_or_else(
                                    || "unknown".into(),
                                    |t| {
                                        if mode == Mode::Agent && !e.all_day {
                                            t.to_rfc3339()
                                        } else {
                                            t.format("%Y-%m-%d").to_string()
                                        }
                                    },
                                ),
                                e.start_time
                                    .filter(|_| !e.all_day)
                                    .map_or(String::new(), |t| t.format("%H:%M").to_string()),
                                e.summary,
                                e.status,
                                e.location,
                            ]
                        })
                        .collect(),
                    "No saved calendar events match that search.",
                )?;
            }
        }
        _ => unreachable!("native command tree restricts dispatch"),
    }
    Ok(())
}
