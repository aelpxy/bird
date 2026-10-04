use std::fmt;

use bird_core::Name;

use crate::profile::Profile;

pub(crate) const DEFAULT_PROJECT: &str = "default";
// the environment every project starts with
pub(crate) const FIRST_ENVIRONMENT: &str = "production";

// the project and environment a command acts in
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Scope {
    pub(crate) project: Name,
    pub(crate) environment: Name,
}

// each comes from flags, then bird.toml, then `bird switch`; a switched environment only counts
// for the project it was chosen in
pub(crate) struct Sources<'a> {
    pub(crate) flags: (Option<Name>, Option<Name>),
    pub(crate) manifest: (Option<Name>, Option<Name>),
    pub(crate) saved: Option<&'a Profile>,
}

impl Scope {
    pub(crate) fn resolve(sources: Sources<'_>) -> Self {
        let Sources {
            flags,
            manifest,
            saved,
        } = sources;
        let switched_project = saved.and_then(|profile| profile.project.clone());
        let project = flags
            .0
            .or(manifest.0)
            .or_else(|| switched_project.clone())
            .unwrap_or_else(|| default_name(DEFAULT_PROJECT));
        let switched_environment = saved
            .filter(|_| switched_project.as_ref() == Some(&project))
            .and_then(|profile| profile.environment.clone());
        let environment = flags
            .1
            .or(manifest.1)
            .or(switched_environment)
            .unwrap_or_else(|| default_name(FIRST_ENVIRONMENT));
        Self {
            project,
            environment,
        }
    }

    // for requests outside any project, like logging in
    pub(crate) fn fallback() -> Self {
        Self {
            project: default_name(DEFAULT_PROJECT),
            environment: default_name(FIRST_ENVIRONMENT),
        }
    }

    // `rest` is what follows the environment, like `services/web/logs`
    pub(crate) fn path(&self, rest: &str) -> String {
        format!(
            "/v1/projects/{}/environments/{}/{rest}",
            self.project, self.environment
        )
    }
}

impl fmt::Display for Scope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.project, self.environment)
    }
}

fn default_name(raw: &str) -> Name {
    raw.parse().expect("built-in names are valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(raw: &str) -> Name {
        raw.parse().unwrap()
    }

    fn saved(project: &str, environment: &str) -> Profile {
        Profile {
            api: "127.0.0.1:7070".to_owned(),
            token: "t".to_owned(),
            project: Some(name(project)),
            environment: Some(name(environment)),
        }
    }

    fn resolve(
        flags: (Option<&str>, Option<&str>),
        manifest: (Option<&str>, Option<&str>),
        saved: Option<&Profile>,
    ) -> String {
        let pick = |pair: (Option<&str>, Option<&str>)| (pair.0.map(name), pair.1.map(name));
        Scope::resolve(Sources {
            flags: pick(flags),
            manifest: pick(manifest),
            saved,
        })
        .to_string()
    }

    #[test]
    fn flags_win_then_bird_toml_then_switch() {
        let switched = saved("shop", "staging");
        assert_eq!(
            resolve((None, None), (None, None), None),
            "default/production"
        );
        assert_eq!(
            resolve((None, None), (None, None), Some(&switched)),
            "shop/staging"
        );
        assert_eq!(
            resolve((None, None), (Some("blog"), None), Some(&switched)),
            "blog/production"
        );
        assert_eq!(
            resolve((None, None), (Some("shop"), None), Some(&switched)),
            "shop/staging"
        );
        assert_eq!(
            resolve((Some("blog"), Some("qa")), (Some("shop"), None), None),
            "blog/qa"
        );
        assert_eq!(
            resolve((None, Some("qa")), (None, None), Some(&switched)),
            "shop/qa"
        );
    }

    #[test]
    fn builds_scoped_paths() {
        let scope = Scope {
            project: name("shop"),
            environment: name("staging"),
        };
        assert_eq!(
            scope.path("services/web"),
            "/v1/projects/shop/environments/staging/services/web"
        );
    }
}
