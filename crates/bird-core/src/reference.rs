use std::collections::BTreeMap;

use crate::{EnvKey, Name};

const OPEN: &str = "${{";
const CLOSE: &str = "}}";
const MAX_DEPTH: usize = 8;

pub type ServiceVariables = BTreeMap<Name, BTreeMap<EnvKey, String>>;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ReferenceError {
    #[error("invalid reference {0:?}: use ${{{{service.KEY}}}}, ${{{{KEY}}}} or ${{{{secret}}}}")]
    Malformed(String),
    #[error("{0} references service {1}, which does not exist")]
    UnknownService(String, Name),
    #[error("{0} references {1}.{2}, which is not set")]
    UnknownVariable(String, Name, EnvKey),
    #[error("variable references loop through {0}")]
    Cycle(String),
}

#[derive(Debug, PartialEq, Eq)]
enum Token<'a> {
    Text(&'a str),
    Reference { service: Option<Name>, key: EnvKey },
    Secret(&'a str),
}

pub fn validate(value: &str) -> Result<(), ReferenceError> {
    tokens(value).map(|_| ())
}

// services a value reads from; malformed values reference nothing
#[must_use]
pub fn referenced_services(value: &str) -> Vec<Name> {
    tokens(value)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|token| match token {
            Token::Reference { service, .. } => service,
            _ => None,
        })
        .collect()
}

#[must_use]
pub fn has_secrets(value: &str) -> bool {
    tokens(value).is_ok_and(|tokens| tokens.iter().any(|t| matches!(t, Token::Secret(_))))
}

// replaces each ${{secret}} with a value from the generator, given the requested length
pub fn expand_secrets<E>(
    value: &str,
    mut generate: impl FnMut(usize) -> Result<String, E>,
) -> Result<Result<String, E>, ReferenceError> {
    let mut out = String::with_capacity(value.len());
    for token in tokens(value)? {
        match token {
            Token::Text(text) => out.push_str(text),
            Token::Reference { service, key } => {
                out.push_str(OPEN);
                if let Some(service) = service {
                    out.push_str(service.as_str());
                    out.push('.');
                }
                out.push_str(key.as_str());
                out.push_str(CLOSE);
            }
            Token::Secret(inner) => {
                let length = secret_length(inner)?;
                match generate(length) {
                    Ok(secret) => out.push_str(&secret),
                    Err(err) => return Ok(Err(err)),
                }
            }
        }
    }
    Ok(Ok(out))
}

pub fn resolve(
    service: &Name,
    variables: &ServiceVariables,
) -> Result<BTreeMap<EnvKey, String>, ReferenceError> {
    let own = variables.get(service).cloned().unwrap_or_default();
    own.keys()
        .map(|key| {
            let mut path = Vec::new();
            resolve_key(service, key, variables, &mut path).map(|value| (key.clone(), value))
        })
        .collect()
}

fn resolve_key(
    service: &Name,
    key: &EnvKey,
    variables: &ServiceVariables,
    path: &mut Vec<String>,
) -> Result<String, ReferenceError> {
    let label = format!("{service}.{key}");
    if path.contains(&label) || path.len() >= MAX_DEPTH {
        path.push(label);
        return Err(ReferenceError::Cycle(path.join(" -> ")));
    }
    let raw = variables
        .get(service)
        .and_then(|vars| vars.get(key))
        .ok_or_else(|| {
            ReferenceError::UnknownVariable(label.clone(), service.clone(), key.clone())
        })?;
    path.push(label.clone());
    let mut out = String::with_capacity(raw.len());
    for token in tokens(raw)? {
        match token {
            Token::Text(text) => out.push_str(text),
            Token::Secret(inner) => return Err(ReferenceError::Malformed(inner.to_owned())),
            Token::Reference {
                service: target,
                key: target_key,
            } => {
                let target = target.unwrap_or_else(|| service.clone());
                let target_vars = variables
                    .get(&target)
                    .ok_or_else(|| ReferenceError::UnknownService(label.clone(), target.clone()))?;
                if !target_vars.contains_key(&target_key) {
                    return Err(ReferenceError::UnknownVariable(
                        label.clone(),
                        target,
                        target_key,
                    ));
                }
                out.push_str(&resolve_key(&target, &target_key, variables, path)?);
            }
        }
    }
    path.pop();
    Ok(out)
}

fn tokens(value: &str) -> Result<Vec<Token<'_>>, ReferenceError> {
    let mut tokens = Vec::new();
    let mut rest = value;
    while let Some(start) = rest.find(OPEN) {
        let (text, tail) = rest.split_at(start);
        if !text.is_empty() {
            tokens.push(Token::Text(text));
        }
        let body = tail.get(OPEN.len()..).unwrap_or_default();
        let end = body
            .find(CLOSE)
            .ok_or_else(|| ReferenceError::Malformed(tail.to_owned()))?;
        let inner = body.get(..end).unwrap_or_default().trim();
        tokens.push(parse_inner(inner)?);
        rest = body.get(end + CLOSE.len()..).unwrap_or_default();
    }
    if !rest.is_empty() {
        tokens.push(Token::Text(rest));
    }
    Ok(tokens)
}

fn parse_inner(inner: &str) -> Result<Token<'_>, ReferenceError> {
    let malformed = || ReferenceError::Malformed(inner.to_owned());
    if inner == "secret" || inner.starts_with("secret(") {
        secret_length(inner)?;
        return Ok(Token::Secret(inner));
    }
    let (service, key) = match inner.split_once('.') {
        Some((service, key)) => (Some(service.parse().map_err(|_| malformed())?), key),
        None => (None, inner),
    };
    let key = key.parse().map_err(|_| malformed())?;
    Ok(Token::Reference { service, key })
}

fn secret_length(inner: &str) -> Result<usize, ReferenceError> {
    const DEFAULT: usize = 32;
    const RANGE: std::ops::RangeInclusive<usize> = 16..=256;
    if inner == "secret" {
        return Ok(DEFAULT);
    }
    inner
        .strip_prefix("secret(")
        .and_then(|rest| rest.strip_suffix(')'))
        .and_then(|digits| digits.trim().parse::<usize>().ok())
        .filter(|length| RANGE.contains(length))
        .ok_or_else(|| ReferenceError::Malformed(inner.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars(entries: &[(&str, &[(&str, &str)])]) -> ServiceVariables {
        entries
            .iter()
            .map(|(service, pairs)| {
                let pairs = pairs
                    .iter()
                    .map(|(k, v)| (k.parse().unwrap(), (*v).to_owned()))
                    .collect();
                (service.parse().unwrap(), pairs)
            })
            .collect()
    }

    fn resolved(service: &str, variables: &ServiceVariables) -> BTreeMap<String, String> {
        resolve(&service.parse().unwrap(), variables)
            .unwrap()
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect()
    }

    #[test]
    fn resolves_own_and_other_services() {
        let variables = vars(&[
            (
                "pg",
                &[
                    ("POSTGRES_PASSWORD", "s3cret"),
                    (
                        "DATABASE_URL",
                        "postgresql://postgres:${{POSTGRES_PASSWORD}}@pg:5432/postgres",
                    ),
                ],
            ),
            (
                "web",
                &[("DATABASE_URL", "${{ pg.DATABASE_URL }}"), ("MODE", "prod")],
            ),
        ]);
        let web = resolved("web", &variables);
        assert_eq!(
            web["DATABASE_URL"],
            "postgresql://postgres:s3cret@pg:5432/postgres"
        );
        assert_eq!(web["MODE"], "prod");
    }

    #[test]
    fn reports_missing_targets() {
        let variables = vars(&[("web", &[("A", "${{nope.X}}")]), ("pg", &[])]);
        assert!(matches!(
            resolve(&"web".parse().unwrap(), &variables),
            Err(ReferenceError::UnknownService(..))
        ));
        let variables = vars(&[("web", &[("A", "${{pg.X}}")]), ("pg", &[])]);
        let err = resolve(&"web".parse().unwrap(), &variables).unwrap_err();
        assert_eq!(err.to_string(), "web.A references pg.X, which is not set");
    }

    #[test]
    fn detects_cycles() {
        let variables = vars(&[("a", &[("X", "${{b.Y}}")]), ("b", &[("Y", "${{a.X}}")])]);
        let err = resolve(&"a".parse().unwrap(), &variables).unwrap_err();
        assert!(
            matches!(err, ReferenceError::Cycle(ref path) if path.contains("a.X")),
            "{err}"
        );
    }

    #[test]
    fn validates_syntax() {
        for ok in [
            "plain",
            "${{KEY}}",
            "${{pg.KEY}}",
            "x${{ pg.KEY }}y",
            "${{secret}}",
            "${{secret(64)}}",
        ] {
            assert!(validate(ok).is_ok(), "{ok}");
        }
        for bad in [
            "${{",
            "${{}}",
            "${{Pg.KEY}}",
            "${{pg.}}",
            "${{secret(4)}}",
            "${{secret(x)}}",
        ] {
            assert!(validate(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn lists_referenced_services() {
        let services = referenced_services("${{pg.URL}} ${{KEY}} ${{ valkey.URL }}");
        let names: Vec<&str> = services.iter().map(Name::as_str).collect();
        assert_eq!(names, ["pg", "valkey"]);
        assert_eq!(referenced_services("${{broken"), Vec::new());
    }

    #[test]
    fn expands_secrets_and_keeps_references() {
        let value = "user:${{secret(20)}}@${{pg.HOST}}";
        assert!(has_secrets(value));
        let expanded = expand_secrets(value, |n| Ok::<_, ()>("s".repeat(n)))
            .unwrap()
            .unwrap();
        assert_eq!(
            expanded,
            format!("user:{}@${{{{pg.HOST}}}}", "s".repeat(20))
        );
        assert!(!has_secrets(&expanded));
    }
}
