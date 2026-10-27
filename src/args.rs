use cli_command_kit::{
    HelpOptions, Mode, TreeOptions,
    clap::{Arg, ArgAction, ArgGroup, Command, value_parser},
};
use gws_cache::Window;
use std::{path::PathBuf, time::Duration};

pub const INSTALL: &str = "Install the gws-cache skill using the Skills CLI:\n\n  npx skills add alexgorbatchev/gws-cache-cli --skill gws-cache\n\nSkill:\n\n  https://github.com/alexgorbatchev/gws-cache-cli/tree/main/skills/gws-cache\n";

pub fn help_options() -> HelpOptions {
    let width = cli_command_kit::terminal_width();
    HelpOptions {
        skill: Some(INSTALL.into()),
        tree: TreeOptions {
            terminal_width: Some(if width == 0 { 100 } else { width }),
            ..Default::default()
        },
        ..Default::default()
    }
}
pub fn cache_path() -> PathBuf {
    std::env::var_os("XDG_CACHE_HOME")
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
        .or_else(dirs::cache_dir)
        .or_else(|| dirs::home_dir().map(|h| h.join(".cache")))
        .map_or_else(
            || PathBuf::from("cache.db"),
            |p| p.join("gws-cache/cache.db"),
        )
}
pub fn rules_path() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
        .or_else(dirs::config_dir)
        .or_else(|| dirs::home_dir().map(|h| h.join(".config")))
        .map_or_else(
            || PathBuf::from("classifier.json"),
            |p| p.join("gws-cache/classifier.json"),
        )
}
fn text(value: &str) -> Result<String, String> {
    if value.trim().is_empty() {
        Err("value must contain text".into())
    } else {
        Ok(value.into())
    }
}
fn timeout(value: &str) -> Result<Duration, String> {
    let nanos = if value == "0" {
        0
    } else {
        go_duration::parse_go_duration(value)
            .map_err(|e| e.to_string())?
            .nanoseconds()
    };
    Ok(Duration::from_nanos(nanos.max(0) as u64))
}
fn window(name: &'static str, default: &'static str) -> Arg {
    Arg::new(name)
        .long(name)
        .default_value(default)
        .value_parser(value_parser!(Window))
        .help("Lookback/window: all, YYYY-MM-DD, or integer h/d/w/m/y")
}
fn format(values: &'static [&'static str], default: &'static str) -> Arg {
    Arg::new("format")
        .long("format")
        .value_parser(values.to_vec())
        .default_value(default)
        .help("Output format")
}
fn boolean(name: &'static str, default: &'static str) -> Arg {
    Arg::new(name)
        .long(name)
        .action(ArgAction::Set)
        .num_args(0..=1)
        .require_equals(true)
        .default_missing_value("true")
        .default_value(default)
        .value_parser(value_parser!(bool))
        .help("Select human-written email only")
}
fn topic() -> Arg {
    Arg::new("topic").value_name("TOPIC").value_parser(text)
}
fn calendar() -> Arg {
    Arg::new("calendar-id")
        .long("calendar-id")
        .default_value("primary")
        .help("Google Calendar ID")
}
fn force() -> Arg {
    Arg::new("force-full")
        .long("force-full")
        .action(ArgAction::SetTrue)
        .help("Refetch the full requested window")
}
fn search(subject: &'static str, since: &'static str) -> Command {
    let command = Command::new("search")
        .about(if subject == "message" {
            "Search cached email"
        } else {
            "Search cached calendar events"
        })
        .arg(Arg::new("words").value_name("WORDS"))
        .arg(Arg::new("topic").long("topic").help("Filter by topic slug"))
        .arg(window("since", since))
        .arg(format(&["table", "json"], "table"));
    if subject == "message" {
        command.arg(boolean("human-only", "false"))
    } else {
        command.arg(calendar())
    }
}
pub fn command(mode: Mode) -> Command {
    let command = Command::new("gws-cache")
        .about("Keep a fast local copy of your email and calendar")
        .arg(
            Arg::new("db")
                .long("db")
                .global(true)
                .value_parser(value_parser!(PathBuf))
                .help("Local SQLite cache path"),
        )
        .arg(
            Arg::new("timeout")
                .long("timeout")
                .global(true)
                .allow_hyphen_values(true)
                .default_value("2m")
                .value_parser(timeout)
                .help("Per-request Go duration; nonpositive disables the deadline"),
        )
        .arg(
            Arg::new("version")
                .long("version")
                .action(ArgAction::SetTrue)
                .help("Print the raw version"),
        )
        .subcommand(
            Command::new("topic")
                .about("Manage tracked topics")
                .subcommand_required(true)
                .subcommand(
                    Command::new("add")
                        .about("Track a topic")
                        .arg(topic().required(true))
                        .arg(
                            Arg::new("name")
                                .long("name")
                                .default_value("")
                                .help("Display name"),
                        )
                        .arg(
                            Arg::new("query")
                                .long("query")
                                .default_value("")
                                .help("Gmail search query"),
                        ),
                )
                .subcommand(Command::new("list").about("List tracked topics"))
                .subcommand(
                    Command::new("rm")
                        .visible_alias("remove")
                        .about("Remove a topic while preserving cached records")
                        .arg(topic().required(true)),
                ),
        )
        .subcommand(
            Command::new("message")
                .about("Sync, search and export email")
                .subcommand_required(true)
                .subcommand(
                    Command::new("sync")
                        .about("Fetch Gmail threads into the cache")
                        .arg(topic())
                        .arg(
                            Arg::new("all")
                                .long("all")
                                .action(ArgAction::SetTrue)
                                .help("Sync every tracked topic"),
                        )
                        .arg(
                            Arg::new("query")
                                .long("query")
                                .value_parser(text)
                                .help("Sync an ad-hoc Gmail search"),
                        )
                        .group(
                            ArgGroup::new("target")
                                .args(["topic", "all", "query"])
                                .required(true)
                                .multiple(false),
                        )
                        .arg(force())
                        .arg(window("since", "4w"))
                        .arg(
                            Arg::new("max-threads")
                                .long("max-threads")
                                .default_value("25")
                                .value_parser(value_parser!(u32).range(1..))
                                .help("Maximum uncached thread fetch attempts"),
                        )
                        .arg(format(&["table", "json"], "table")),
                )
                .subcommand(search("message", "14d"))
                .subcommand(
                    Command::new("export")
                        .about("Export a topic timeline")
                        .arg(topic().required(true))
                        .arg(boolean("human-only", "true"))
                        .arg(window("since", "4w"))
                        .arg(format(&["markdown", "json"], "markdown")),
                ),
        )
        .subcommand(
            Command::new("event")
                .about("Sync and search calendar events")
                .subcommand_required(true)
                .subcommand(
                    Command::new("sync")
                        .about("Fetch Google Calendar events")
                        .arg(calendar())
                        .arg(window("past", "4w"))
                        .arg(window("future", "4w"))
                        .arg(force())
                        .arg(format(&["table", "json"], "table")),
                )
                .subcommand(search("event", "4w")),
        )
        .subcommand(
            Command::new("cache")
                .about("Inspect the local cache")
                .subcommand_required(true)
                .subcommand(Command::new("status").about("Show cache location and record counts")),
        )
        .subcommand(
            Command::new("completion")
                .about("Generate shell completion")
                .arg(
                    Arg::new("shell")
                        .value_name("SHELL")
                        .required(true)
                        .value_parser(value_parser!(clap_complete::Shell)),
                ),
        );
    cli_command_kit::setup(command, mode, &help_options()).expect("unique skill command")
}
