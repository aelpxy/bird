use std::str::FromStr;

use bird_core::{
    Certificate, Deployment, Domain, Environment, Hostname, Machine, MachineId, Port, Project,
    Replicas, Service, Variable,
};

use crate::RouteEntry;
use rusqlite::Row;
use rusqlite::types::Type;

fn parse<T>(row: &Row<'_>, idx: usize) -> rusqlite::Result<T>
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

fn replicas(row: &Row<'_>, idx: usize) -> rusqlite::Result<Replicas> {
    let raw: u8 = row.get(idx)?;
    Replicas::try_from(raw)
        .map_err(|err| rusqlite::Error::FromSqlConversionFailure(idx, Type::Integer, Box::new(err)))
}

pub(crate) fn project(row: &Row<'_>) -> rusqlite::Result<Project> {
    Ok(Project {
        id: parse(row, 0)?,
        name: parse(row, 1)?,
        created_at: row.get(2)?,
    })
}

pub(crate) fn environment(row: &Row<'_>) -> rusqlite::Result<Environment> {
    Ok(Environment {
        id: parse(row, 0)?,
        project_id: parse(row, 1)?,
        name: parse(row, 2)?,
        created_at: row.get(3)?,
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
