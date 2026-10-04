use std::str::FromStr;

use bird_core::{
    Backup, BackupKeep, BackupSchedule, BackupVolume, Certificate, Command, Deployment, Domain,
    EnvKey, Environment, HealthCheck, Hostname, ImageRef, Machine, MachineId, Port, Project,
    Registry, Replicas, Service, ServiceId, Variable, Volume,
};

use crate::RouteEntry;
use rusqlite::Row;
use rusqlite::types::Type;

pub(crate) fn parse<T>(row: &Row<'_>, idx: usize) -> rusqlite::Result<T>
where
    T: FromStr,
    T::Err: std::error::Error + Send + Sync + 'static,
{
    let raw: String = row.get(idx)?;
    raw.parse()
        .map_err(|err| rusqlite::Error::FromSqlConversionFailure(idx, Type::Text, Box::new(err)))
}

fn parse_optional<T>(row: &Row<'_>, idx: usize) -> rusqlite::Result<Option<T>>
where
    T: FromStr,
    T::Err: std::error::Error + Send + Sync + 'static,
{
    let raw: Option<String> = row.get(idx)?;
    raw.map(|raw| {
        raw.parse().map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(idx, Type::Text, Box::new(err))
        })
    })
    .transpose()
}

fn port(row: &Row<'_>, idx: usize) -> rusqlite::Result<Port> {
    let raw: u16 = row.get(idx)?;
    Port::try_from(raw)
        .map_err(|err| rusqlite::Error::FromSqlConversionFailure(idx, Type::Integer, Box::new(err)))
}

fn command(row: &Row<'_>, idx: usize) -> rusqlite::Result<Option<Command>> {
    let raw: Option<String> = row.get(idx)?;
    raw.map(|raw| {
        let args: Vec<String> = serde_json::from_str(&raw).map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(idx, Type::Text, Box::new(err))
        })?;
        Command::try_from(args).map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(idx, Type::Text, Box::new(err))
        })
    })
    .transpose()
}

fn limit<T>(row: &Row<'_>, idx: usize) -> rusqlite::Result<T>
where
    T: TryFrom<u32>,
    T::Error: std::error::Error + Send + Sync + 'static,
{
    let raw: u32 = row.get(idx)?;
    T::try_from(raw)
        .map_err(|err| rusqlite::Error::FromSqlConversionFailure(idx, Type::Integer, Box::new(err)))
}

// the check's kind and its path live in two columns, the path only ever set for http
fn health(row: &Row<'_>, kind: usize, path: usize) -> rusqlite::Result<HealthCheck> {
    let check: HealthCheck = parse(row, kind)?;
    match (check, parse_optional(row, path)?) {
        (HealthCheck::Http, Some(path)) => Ok(HealthCheck::Path(path)),
        (check, _) => Ok(check),
    }
}

fn replicas(row: &Row<'_>, idx: usize) -> rusqlite::Result<Replicas> {
    let raw: u8 = row.get(idx)?;
    Replicas::try_from(raw)
        .map_err(|err| rusqlite::Error::FromSqlConversionFailure(idx, Type::Integer, Box::new(err)))
}

pub(crate) fn project(row: &Row<'_>) -> rusqlite::Result<Project> {
    Ok(Project {
        id: parse(row, 0)?,
        org_id: parse(row, 1)?,
        name: parse(row, 2)?,
        created_at: row.get(3)?,
    })
}

pub(crate) fn environment(row: &Row<'_>) -> rusqlite::Result<Environment> {
    Ok(Environment {
        id: parse(row, 0)?,
        project_id: parse(row, 1)?,
        name: parse(row, 2)?,
        network: row.get(3)?,
        created_at: row.get(4)?,
    })
}

pub(crate) fn service(row: &Row<'_>) -> rusqlite::Result<Service> {
    Ok(Service {
        id: parse(row, 0)?,
        environment_id: parse(row, 1)?,
        name: parse(row, 2)?,
        image: parse(row, 3)?,
        port: port(row, 4)?,
        created_at: row.get(5)?,
        replicas: replicas(row, 6)?,
        health: health(row, 7, 11)?,
        health_timeout: limit(row, 12)?,
        state: parse(row, 13)?,
        command: command(row, 8)?,
        memory: limit(row, 9)?,
        cpus: limit(row, 10)?,
    })
}

pub(crate) fn deployment(row: &Row<'_>) -> rusqlite::Result<Deployment> {
    Ok(Deployment {
        id: parse(row, 0)?,
        service_id: parse(row, 1)?,
        image: parse(row, 2)?,
        port: port(row, 3)?,
        status: parse(row, 4)?,
        created_at: row.get(5)?,
        command: command(row, 6)?,
    })
}

pub(crate) fn machine(row: &Row<'_>) -> rusqlite::Result<Machine> {
    Ok(Machine {
        id: parse(row, 0)?,
        deployment_id: parse(row, 1)?,
        container_id: row.get(2)?,
        address: parse_optional(row, 3)?,
        state: parse(row, 4)?,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
    })
}

pub(crate) fn machine_id(row: &Row<'_>) -> rusqlite::Result<MachineId> {
    parse(row, 0)
}

pub(crate) fn domain(row: &Row<'_>) -> rusqlite::Result<Domain> {
    Ok(Domain {
        hostname: parse(row, 0)?,
        service_id: parse(row, 1)?,
        created_at: row.get(2)?,
    })
}

pub(crate) fn variable(row: &Row<'_>) -> rusqlite::Result<Variable> {
    Ok(Variable {
        service_id: parse(row, 0)?,
        key: parse(row, 1)?,
        value: row.get(2)?,
    })
}

pub(crate) fn route(row: &Row<'_>) -> rusqlite::Result<RouteEntry> {
    Ok(RouteEntry {
        hostname: parse(row, 0)?,
        address: parse_optional(row, 1)?,
    })
}

pub(crate) fn key_value(row: &Row<'_>) -> rusqlite::Result<(EnvKey, String)> {
    Ok((parse(row, 0)?, row.get(1)?))
}

pub(crate) fn image_use(row: &Row<'_>) -> rusqlite::Result<(ImageRef, bool)> {
    Ok((parse(row, 0)?, row.get(1)?))
}

pub(crate) fn hostname(row: &Row<'_>) -> rusqlite::Result<Hostname> {
    parse(row, 0)
}

pub(crate) fn certificate(row: &Row<'_>) -> rusqlite::Result<Certificate> {
    Ok(Certificate {
        hostname: parse(row, 0)?,
        chain_pem: row.get(1)?,
        key_pem: row.get(2)?,
        not_after: row.get(3)?,
    })
}

pub(crate) fn volume(row: &Row<'_>) -> rusqlite::Result<Volume> {
    Ok(Volume {
        id: parse(row, 0)?,
        service_id: parse(row, 1)?,
        name: parse(row, 2)?,
        mount_path: parse(row, 3)?,
        lineage: row.get(4)?,
        created_at: row.get(5)?,
    })
}

// volumes are loaded separately and filled in by the caller
pub(crate) fn backup(row: &Row<'_>) -> rusqlite::Result<Backup> {
    Ok(Backup {
        id: parse(row, 0)?,
        environment_id: parse(row, 1)?,
        service: parse(row, 2)?,
        trigger: parse(row, 3)?,
        storage: row.get(4)?,
        volumes: Vec::new(),
        created_at: row.get(5)?,
    })
}

pub(crate) fn backup_volume(row: &Row<'_>) -> rusqlite::Result<BackupVolume> {
    let size: i64 = row.get(3)?;
    Ok(BackupVolume {
        name: parse(row, 0)?,
        lineage: row.get(1)?,
        key: row.get(2)?,
        size_bytes: u64::try_from(size).map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(3, Type::Integer, Box::new(err))
        })?,
    })
}

pub(crate) fn registry(row: &Row<'_>) -> rusqlite::Result<Registry> {
    Ok(Registry {
        host: parse(row, 0)?,
        username: row.get(1)?,
        password: row.get(2)?,
        insecure: row.get(3)?,
    })
}

pub(crate) fn backup_schedule(row: &Row<'_>) -> rusqlite::Result<BackupSchedule> {
    Ok(BackupSchedule {
        every: limit(row, 0)?,
        keep: keep(row, 1)?,
    })
}

pub(crate) fn scheduled_service(row: &Row<'_>) -> rusqlite::Result<(ServiceId, BackupSchedule)> {
    Ok((parse(row, 2)?, backup_schedule(row)?))
}

fn keep(row: &Row<'_>, idx: usize) -> rusqlite::Result<BackupKeep> {
    let raw: u16 = row.get(idx)?;
    BackupKeep::try_from(raw)
        .map_err(|err| rusqlite::Error::FromSqlConversionFailure(idx, Type::Integer, Box::new(err)))
}
