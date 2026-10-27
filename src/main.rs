use std::io;
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() {
    let cancel = CancellationToken::new();
    let signal = cancel.clone();
    let handler = tokio::spawn(async move {
        #[cfg(unix)]
        {
            if let Ok(mut terminate) =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            {
                tokio::select! { _=tokio::signal::ctrl_c()=>{}, _=terminate.recv()=>{} }
                signal.cancel();
            }
        }
        #[cfg(not(unix))]
        if tokio::signal::ctrl_c().await.is_ok() {
            signal.cancel();
        }
    });
    let status = gws_cache_cli::entry(
        std::env::args_os(),
        &mut io::stdout().lock(),
        &mut io::stderr().lock(),
        cli_command_kit::Mode::from_env(),
        &cancel,
    )
    .await;
    handler.abort();
    std::process::exit(i32::from(status));
}
