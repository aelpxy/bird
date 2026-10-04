mod args;
mod client;
mod commands;
mod context;
mod dockerignore;
mod manifest;
mod profile;
mod ui;

use std::process::ExitCode;

use clap::Parser;

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let args = args::Args::parse();
    match commands::run(args).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) if closed_pipe(&err) => ExitCode::SUCCESS,
        Err(err) if err.is::<ui::prompt::Cancelled>() => {
            eprintln!("cancelled");
            ExitCode::FAILURE
        }
        Err(err) => {
            eprintln!(
                "{} {err:#}",
                ui::style::err(ui::style::Paint::Red, "error:")
            );
            ExitCode::FAILURE
        }
    }
}

// `bird logs | head` closing early is a normal way to stop reading, not an error
fn closed_pipe(err: &anyhow::Error) -> bool {
    err.chain()
        .filter_map(|cause| cause.downcast_ref::<std::io::Error>())
        .any(|io| io.kind() == std::io::ErrorKind::BrokenPipe)
}
