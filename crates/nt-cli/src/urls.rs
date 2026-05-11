//! URL resolution: --profile > env vars > defaults.
//! Mirrors `src/sdk/url-resolver.ts::resolveUrls`.

use indexmap::IndexMap;
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

use crate::home;

pub const DEFAULT_API: &str = "https://api.no-tickets.com";
pub const DEFAULT_AUTH: &str = "https://app.no-tickets.com/api/auth/cli";

pub struct ResolvedUrls {
    pub api_url: String,
    pub auth_url: String,
}

pub enum UrlError {
    PartialPair {
        which: &'static str,
        value: String,
        missing: &'static str,
    },
    HomeUnresolvable,
    ProfileFileMissing {
        name: String,
        path: PathBuf,
    },
    ProfileFileUnreadable {
        path: PathBuf,
        message: String,
    },
    ProfileFileInvalidJson {
        path: PathBuf,
        message: String,
    },
    ProfileNotFound {
        name: String,
        path: PathBuf,
        available: Vec<String>,
    },
    ProfileInvalidUrls {
        name: String,
        path: PathBuf,
    },
}

impl UrlError {
    pub fn user_message(&self) -> String {
        match self {
            UrlError::PartialPair { which, value, missing } => {
                let quoted = serde_json::to_string(value)
                    .unwrap_or_else(|_| format!("{value:?}"));
                format!(
                    "{which}={quoted} is set but {missing} is not. \
                     Set both (or neither) so the API and auth flow agree on which environment to use.",
                )
            }
            UrlError::HomeUnresolvable => {
                "Could not resolve home directory. Set NO_TICKETS_HOME, HOME, or USERPROFILE.".to_string()
            }
            UrlError::ProfileFileMissing { name, path } => format!(
                "profile \"{name}\" not found: {path} does not exist.\n\
                 Create it with:\n  \
                 {{ \"profiles\": {{ \"{name}\": {{ \"apiUrl\": \"https://…\", \"authUrl\": \"https://…\" }} }} }}",
                path = path.display(),
            ),
            UrlError::ProfileFileUnreadable { path, message } => format!(
                "{path} could not be read: {message}",
                path = path.display(),
            ),
            UrlError::ProfileFileInvalidJson { path, message } => format!(
                "{path} contains invalid JSON: {message}",
                path = path.display(),
            ),
            UrlError::ProfileNotFound { name, path, available } => {
                let hint = if available.is_empty() {
                    String::new()
                } else {
                    format!(" Available: {}.", available.join(", "))
                };
                format!(
                    "profile \"{name}\" not found in {path}.{hint}",
                    path = path.display(),
                )
            }
            UrlError::ProfileInvalidUrls { name, path } => format!(
                "profile \"{name}\" in {path} is invalid: apiUrl and authUrl must be http(s) URL strings.",
                path = path.display(),
            ),
        }
    }
}

/// IndexMap preserves insertion order from the on-disk JSON — required so
/// the "Available: a, b, c" hint matches the TS `Object.keys()` order, not
/// alphabetical.
#[derive(Deserialize)]
struct ConfigFile {
    #[serde(default)]
    profiles: Option<IndexMap<String, ProfileConfig>>,
}

#[derive(Deserialize)]
struct ProfileConfig {
    #[serde(rename = "apiUrl")]
    api_url: String,
    #[serde(rename = "authUrl")]
    auth_url: String,
}

pub fn resolve_urls(profile: Option<&str>) -> Result<ResolvedUrls, UrlError> {
    if let Some(name) = profile {
        return load_profile(name);
    }

    let env_api = std::env::var("NO_TICKETS_API_URL").unwrap_or_default();
    let env_auth = std::env::var("NO_TICKETS_AUTH_URL").unwrap_or_default();
    let api_trim = env_api.trim();
    let auth_trim = env_auth.trim();
    let api_set = !api_trim.is_empty();
    let auth_set = !auth_trim.is_empty();

    if api_set != auth_set {
        let (which, value, missing) = if api_set {
            ("NO_TICKETS_API_URL", api_trim.to_string(), "NO_TICKETS_AUTH_URL")
        } else {
            ("NO_TICKETS_AUTH_URL", auth_trim.to_string(), "NO_TICKETS_API_URL")
        };
        return Err(UrlError::PartialPair { which, value, missing });
    }

    if api_set && auth_set {
        return Ok(ResolvedUrls {
            api_url: api_trim.to_string(),
            auth_url: auth_trim.to_string(),
        });
    }

    Ok(ResolvedUrls {
        api_url: DEFAULT_API.to_string(),
        auth_url: DEFAULT_AUTH.to_string(),
    })
}

fn load_profile(name: &str) -> Result<ResolvedUrls, UrlError> {
    let path = home::config_path().ok_or(UrlError::HomeUnresolvable)?;

    if !path.exists() {
        return Err(UrlError::ProfileFileMissing {
            name: name.to_string(),
            path,
        });
    }

    let raw = fs::read_to_string(&path).map_err(|e| UrlError::ProfileFileUnreadable {
        path: path.clone(),
        message: e.to_string(),
    })?;

    let parsed: ConfigFile =
        serde_json::from_str(&raw).map_err(|e| UrlError::ProfileFileInvalidJson {
            path: path.clone(),
            message: e.to_string(),
        })?;

    let profiles = parsed.profiles.unwrap_or_default();
    let profile = match profiles.get(name) {
        Some(p) => p,
        None => {
            let available: Vec<String> = profiles.keys().cloned().collect();
            return Err(UrlError::ProfileNotFound {
                name: name.to_string(),
                path,
                available,
            });
        }
    };

    if !is_http_url(&profile.api_url) || !is_http_url(&profile.auth_url) {
        return Err(UrlError::ProfileInvalidUrls {
            name: name.to_string(),
            path,
        });
    }

    Ok(ResolvedUrls {
        api_url: profile.api_url.clone(),
        auth_url: profile.auth_url.clone(),
    })
}

/// Matches TS `new URL(s)` then `protocol === 'http:' || protocol === 'https:'`.
/// Stricter than a `starts_with` prefix check: rejects malformed URLs that
/// happen to begin with the scheme (e.g. `https://`, `http:// nope`,
/// embedded newlines).
fn is_http_url(s: &str) -> bool {
    let Ok(parsed) = url::Url::parse(s) else {
        return false;
    };
    if !matches!(parsed.scheme(), "http" | "https") {
        return false;
    }
    // `new URL("https://")` throws in JS; `url::Url::parse("https://")`
    // accepts it with empty host. Require a non-empty host for parity.
    parsed.host_str().is_some_and(|h| !h.is_empty())
}
