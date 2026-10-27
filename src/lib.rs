pub mod args;
mod output;
mod runtime;
use cli_command_kit::{Mode, clap::error::ErrorKind};
use std::{ffi::OsString, io::Write};
use tokio_util::sync::CancellationToken;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Cache(#[from] gws_cache::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("{0} topic syncs failed; inspect the output for individual outcomes")]
    Partial(usize),
}
pub async fn entry(
    argv: impl IntoIterator<Item = OsString>,
    mut out: &mut dyn Write,
    err: &mut dyn Write,
    mode: Mode,
    cancel: &CancellationToken,
) -> u8 {
    let mut command = args::command(mode);
    let matches = match command.clone().try_get_matches_from(argv) {
        Ok(matches) => matches,
        Err(error) => {
            let informational = matches!(
                error.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            );
            let target: &mut dyn Write = if informational { out } else { err };
            return if write!(
                target,
                "{}",
                cli_command_kit::display_message(&error.to_string())
            )
            .is_err()
            {
                1
            } else if informational {
                0
            } else {
                1
            };
        }
    };
    let result = if matches.get_flag("version") {
        writeln!(out, "{}", env!("CARGO_PKG_VERSION")).map_err(Error::from)
    } else if matches.subcommand_name() == Some("skill") {
        cli_command_kit::write_skill(&matches, &args::help_options(), &mut out)
            .map(|_| ())
            .map_err(Error::from)
    } else if let Some(("completion", sub)) = matches.subcommand() {
        let mut completion = Vec::new();
        clap_complete::generate(
            *sub.get_one::<clap_complete::Shell>("shell")
                .expect("validated shell"),
            &mut command,
            "gws-cache",
            &mut completion,
        );
        out.write_all(&completion).map_err(Error::from)
    } else if matches.subcommand_name().is_none() {
        command.write_long_help(&mut out).map_err(Error::from)
    } else {
        runtime::run(&matches, out, err, mode, cancel).await
    };
    match result {
        Ok(()) => 0,
        Err(error) => {
            let status = if matches!(error, Error::Cache(gws_cache::Error::Cancelled)) {
                130
            } else {
                1
            };
            let _ = output::status(err, mode, "ERR", &error.to_string());
            status
        }
    }
}
