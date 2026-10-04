use bird_core::{Name, Org, OrgRole, User};

use crate::state::AppState;
use crate::{Error, Result};

pub(crate) async fn find(state: &AppState, name: &Name) -> Result<Org> {
    let lookup = name.clone();
    state
        .db
        .call(move |store| store.org_by_name(&lookup))
        .await?
        .ok_or_else(|| Error::OrgNotFound(name.clone()))
}

pub(crate) async fn create(state: &AppState, name: &Name, owner: Option<&User>) -> Result<Org> {
    let (owned_name, owner_id) = (name.clone(), owner.map(|user| user.id));
    let org = state
        .db
        .call(move |store| {
            store.transaction(|store| {
                let org = store.create_org(&owned_name)?;
                if let Some(owner_id) = owner_id {
                    store.set_member(org.id, owner_id, OrgRole::Owner)?;
                }
                Ok(org)
            })
        })
        .await
        .map_err(|err| match err {
            Error::Store(bird_store::Error::AlreadyExists(_)) => Error::OrgExists(name.clone()),
            other => other,
        })?;
    tracing::info!(org = %name, "org created");
    Ok(org)
}

// refused while it owns projects, checked in the same transaction as the delete
pub(crate) async fn remove(state: &AppState, org: &Org) -> Result<()> {
    let id = org.id;
    let projects = state
        .db
        .call(move |store| {
            store.transaction(|store| {
                let projects = store.list_org_projects(id)?;
                if projects.is_empty() {
                    store.delete_org(id)?;
                }
                Ok(projects)
            })
        })
        .await?;
    if !projects.is_empty() {
        return Err(Error::OrgHasProjects {
            org: org.name.clone(),
            projects: projects.into_iter().map(|project| project.name).collect(),
        });
    }
    tracing::info!(org = %org.name, "org deleted");
    Ok(())
}

pub(crate) async fn members(state: &AppState, org: &Org) -> Result<Vec<(User, OrgRole)>> {
    let id = org.id;
    state.db.call(move |store| store.list_members(id)).await
}

// an org keeps at least one owner once it has had one, so someone can always manage it
pub(crate) async fn set_member(
    state: &AppState,
    org: &Org,
    user: &User,
    role: OrgRole,
) -> Result<()> {
    if role != OrgRole::Owner {
        keep_an_owner(state, org, user).await?;
    }
    let (org_id, user_id) = (org.id, user.id);
    state
        .db
        .call(move |store| store.set_member(org_id, user_id, role))
        .await?;
    tracing::info!(org = %org.name, user = %user.name, role = %role, "org member set");
    Ok(())
}

enum Left {
    Member,
    // the last owner left an org that owned nothing, so it went with them
    OrgDeleted,
    Refused(Vec<Name>),
}

// the last owner may only leave an org that owns no projects, which is then deleted; checked in
// the same transaction as the change, so a project created meanwhile cannot slip through
pub(crate) async fn remove_member(state: &AppState, org: &Org, user: &User) -> Result<()> {
    let (org_id, user_id) = (org.id, user.id);
    let left = state
        .db
        .call(move |store| {
            store.transaction(|store| {
                let last_owner = store
                    .sole_owned_orgs(user_id)?
                    .iter()
                    .any(|owned| owned.id == org_id);
                if !last_owner {
                    store.remove_member(org_id, user_id)?;
                    return Ok(Left::Member);
                }
                let projects = store.list_org_projects(org_id)?;
                if !projects.is_empty() {
                    return Ok(Left::Refused(
                        projects.into_iter().map(|p| p.name).collect(),
                    ));
                }
                store.delete_org(org_id)?;
                Ok(Left::OrgDeleted)
            })
        })
        .await
        .map_err(|err| match err {
            Error::Store(bird_store::Error::NotFound(_)) => Error::NotMember {
                org: org.name.clone(),
                user: user.name.clone(),
            },
            other => other,
        })?;
    match left {
        Left::Member => tracing::info!(org = %org.name, user = %user.name, "org member removed"),
        Left::OrgDeleted => {
            tracing::info!(org = %org.name, user = %user.name, "last owner left, empty org deleted");
        }
        Left::Refused(projects) => {
            return Err(Error::OwnsProjects {
                user: user.name.clone(),
                org: org.name.clone(),
                projects,
            });
        }
    }
    Ok(())
}

async fn keep_an_owner(state: &AppState, org: &Org, leaving: &User) -> Result<()> {
    let members = members(state, org).await?;
    let owners: Vec<&User> = members
        .iter()
        .filter(|(_, role)| *role == OrgRole::Owner)
        .map(|(user, _)| user)
        .collect();
    if owners.len() == 1 && owners.iter().any(|owner| owner.id == leaving.id) {
        return Err(Error::LastOwner {
            org: org.name.clone(),
            user: leaving.name.clone(),
        });
    }
    Ok(())
}
