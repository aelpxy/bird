use std::io::Write;
use std::time::Duration;

use anyhow::Result;
use bird_api::{LogEntry, LogStream};
use bird_core::Name;

use crate::client::ApiClient;

const TIMEOUT: Duration = Duration::from_secs(30);

pub(crate) async fn run(client: &ApiClient, name: &Name, tail: u32) -> Result<()> {
    let path = format!("/v1/services/{name}/logs?tail={tail}");
    let entries: Vec<LogEntry> = client.get(&path, TIMEOUT).await?;
    let mut stdout = std::io::stdout().lock();
    let mut stderr = std::io::stderr().lock();
    for entry in entries {
        match entry.stream {
            LogStream::Stdout => writeln!(stdout, "{}", entry.text)?,
            LogStream::Stderr => writeln!(stderr, "{}", entry.text)?,
        }
    }
    Ok(())
}
