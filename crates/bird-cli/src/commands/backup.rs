use std::time::Duration;

use anyhow::Result;
use bird_api::{BackupInfo, RestoreRequest, RestoreResponse};
use bird_core::Name;

use super::history::{ago, unix_now};
use super::table::render;
use crate::args::BackupCommand;
use crate::client::ApiClient;
use crate::ui::style::{self, Paint};
use crate::ui::{Output, Spinner, duration, prompt};

const TIMEOUT: Duration = Duration::from_secs(30);
// copying large volumes takes a while, and a restore copies twice and starts the app again
const TRANSFER_TIMEOUT: Duration = Duration::from_hours(3);

pub(crate) async fn run(
    client: &ApiClient,
    name: &Name,
    command: BackupCommand,
    out: Output,
) -> Result<()> {
    match command {
        BackupCommand::Create => {
            let spinner = Spinner::start(format!(
                "backing up {name}, its machines pause until the copy is done"
            ));
            let backup: BackupInfo = client
                .post(
                    &format!("/v1/services/{name}/backups"),
                    &(),
                    TRANSFER_TIMEOUT,
                )
                .await?;
            let elapsed = spinner.elapsed();
            drop(spinner);
            if !out.json(&backup)? {
                println!(
                    "{} backed up {name} in {}: {} {}",
                    style::out(Paint::Green, "✓"),
                    duration(elapsed),
                    size(backup.volumes.iter().map(|v| v.size_bytes).sum()),
                    style::out(Paint::Dim, format!("(backup {})", backup.id))
                );
            }
        }
        BackupCommand::List => list(client, name, out).await?,
        BackupCommand::Restore {
            backup,
            allow_image_change,
            yes,
        } => {
            prompt::confirm(
                &format!(
                    "replace {name}'s data with backup {backup}? the current data is saved as a new backup first, and its machines restart"
                ),
                yes,
            )?;
            let spinner = Spinner::start(format!("restoring {name} from backup {backup}"));
            let response: RestoreResponse = client
                .post(
                    &format!("/v1/services/{name}/backups/{backup}/restore"),
                    &RestoreRequest { allow_image_change },
                    TRANSFER_TIMEOUT,
                )
                .await?;
            let elapsed = spinner.elapsed();
            drop(spinner);
            if !out.json(&response)? {
                println!(
                    "{} restored {name} from {} in {}",
                    style::out(Paint::Green, "✓"),
                    response.restored,
                    duration(elapsed)
                );
                println!(
                    "  {}",
                    style::out(
                        Paint::Dim,
                        format!(
                            "the data it replaced is backup {}, restore that to undo",
                            response.safety_backup
                        )
                    )
                );
            }
        }
        BackupCommand::Remove { backup, yes } => {
            prompt::confirm(&format!("delete backup {backup} of {name}?"), yes)?;
            client
                .delete(&format!("/v1/services/{name}/backups/{backup}"), TIMEOUT)
                .await?;
            println!("{} deleted backup {backup}", style::out(Paint::Green, "✓"));
        }
    }
    Ok(())
}

async fn list(client: &ApiClient, name: &Name, out: Output) -> Result<()> {
    let backups: Vec<BackupInfo> = client
        .get(&format!("/v1/services/{name}/backups"), TIMEOUT)
        .await?;
    if out.json(&backups)? {
        return Ok(());
    }
    if backups.is_empty() {
        println!(
            "{name} has no backups yet, make one with {}",
            style::out(Paint::Bold, "bird backup create")
        );
        return Ok(());
    }
    let now = unix_now();
    let rows: Vec<Vec<String>> = backups
        .iter()
        .map(|b| {
            let volumes: Vec<&str> = b.volumes.iter().map(|v| v.name.as_str()).collect();
            vec![
                b.id.to_string(),
                b.trigger.to_string(),
                volumes.join(","),
                size(b.volumes.iter().map(|v| v.size_bytes).sum()),
                b.storage.clone(),
                ago(now.saturating_sub(b.created_at)),
            ]
        })
        .collect();
    print!(
        "{}",
        render(
            &["BACKUP", "TRIGGER", "VOLUMES", "SIZE", "STORAGE", "CREATED"],
            &rows
        )
    );
    Ok(())
}

fn size(bytes: u64) -> String {
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut value = bytes;
    let mut unit = "KiB";
    for larger in ["MiB", "GiB", "TiB"] {
        if value < 1024 * 1024 {
            break;
        }
        value /= 1024;
        unit = larger;
    }
    let tenths = value.saturating_mul(10) / 1024;
    format!("{}.{} {unit}", tenths / 10, tenths % 10)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_sizes() {
        assert_eq!(size(0), "0 B");
        assert_eq!(size(1023), "1023 B");
        assert_eq!(size(1024), "1.0 KiB");
        assert_eq!(size(1536), "1.5 KiB");
        assert_eq!(size(45 * 1024 * 1024), "45.0 MiB");
        assert_eq!(size(3 * 1024 * 1024 * 1024 / 2), "1.5 GiB");
    }
}
