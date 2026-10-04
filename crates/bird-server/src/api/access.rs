use bird_core::{OrgId, OrgRole, Project, UserRole};

use super::auth::Principal;
use crate::state::AppState;
use crate::{Error, Result};

// the role a principal has in an org; server admins act as its owners
pub(crate) async fn role_in(
    state: &AppState,
    principal: &Principal,
    org: OrgId,
) -> Result<Option<OrgRole>> {
    match principal {
        Principal::Root => Ok(Some(OrgRole::Owner)),
        Principal::User(user) if user.role == UserRole::Admin => Ok(Some(OrgRole::Owner)),
        Principal::User(user) => {
            let user_id = user.id;
            state
                .db
                .call(move |store| store.membership(org, user_id))
                .await
        }
    }
}

// a project outside the principal's orgs looks missing, so its name does not leak
pub(crate) async fn require_project(
    state: &AppState,
    principal: &Principal,
    project: &Project,
    needed: OrgRole,
) -> Result<()> {
    match role_in(state, principal, project.org_id).await? {
        None => Err(Error::ProjectNotFound(project.name.clone())),
        Some(role) if role.at_least(needed) => Ok(()),
        Some(_) => Err(forbidden(needed)),
    }
}

pub(crate) const fn forbidden(needed: OrgRole) -> Error {
    Error::Forbidden(match needed {
        OrgRole::Owner => "only the org's owners can do that",
        OrgRole::Admin => "only the org's admins and owners can do that",
        OrgRole::Member => "only the org's members can do that",
    })
}
