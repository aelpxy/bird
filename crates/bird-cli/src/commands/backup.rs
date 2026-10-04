use std::time::Duration;

use anyhow::Result;
use bird_api::{BackupInfo, RestoreRequest, RestoreResponse};
use bird_core::Name;

use super::history::{ago, unix_now};
use super::table::render;
use crate::args::BackupCommand;
use crate::client::ApiClient;

const TIMEOUT: Duration = Duration::from_secs(30);
// copying large volumes takes a while, and a restore copies twice and starts the app again
const TRANSFER_TIMEOUT: Duration = Duration::from_hours(3);

pub(crate) async fn run(client: &ApiClient, command: BackupCommand) -> Result<()> {
    match command {
        BackupCommand::Create { name } => {
            println!("backing up {name}, its machines pause until the copy is done...");
            let backup: BackupInfo = client
                .post(
                    &format!("/v1/services/{name}/backups"),
                    &(),
                    TRANSFER_TIMEOUT,
                )
                .await?;
            println!(
                "backup {} of {name}: {}",
                backup.id,
                size(backup.volumes.iter().map(|v| v.size_bytes).sum())
            );
        }
        BackupCommand::List { name } => list(client, &name).await?,
        BackupCommand::Restore {
            name,
            backup,
            allow_image_change,
        } => {
            println!("restoring {name} from backup {backup}, its machines restart...");
            let response: RestoreResponse = client
                .post(
                    &format!("/v1/services/{name}/backups/{backup}/restore"),
                    &RestoreRequest { allow_image_change },
                    TRANSFER_TIMEOUT,
                )
                .await?;
            println!(
                "restored {name} from {}; the data it replaced is backup {}",
                response.restored, response.safety_backup
            );
        }
        BackupCommand::Remove { name, backup } => {
            client
                .delete(&format!("/v1/services/{name}/backups/{backup}"), TIMEOUT)
                .await?;
            println!("deleted backup {backup}");
        }
    }
    Ok(())
}

async fn list(client: &ApiClient, name: &Name) -> Result<()> {
    let backups: Vec<BackupInfo> = client
        .get(&format!("/v1/services/{name}/backups"), TIMEOUT)
        .await?;
    if backups.is_empty() {
        println!("{name} has no backups yet, make one with `bird backup create {name}`");
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
