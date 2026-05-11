//! URL resolution per ADR-0002 (three layers):
//!
//! 1. **Default** — production URLs (`api.no-tickets.com` + `app.no-tickets.com/api/auth/cli`).
//! 2. **Preset** — `NO_TICKETS_ENV=staging|local|prod` picks from a closed table. Unknown values error.
//! 3. **Explicit pair** — `NO_TICKETS_API_URL` + `NO_TICKETS_AUTH_URL` (both required) — the escape hatch.
//!
//! Layers 2 and 3 are mutually exclusive: setting both errors with `EnvAndPairBothSet`.

use crate::env::Env;

pub const DEFAULT_API: &str = "https://api.no-tickets.com";
pub const DEFAULT_AUTH: &str = "https://app.no-tickets.com/api/auth/cli";

pub const STAGING_API: &str = "https://api-staging.no-tickets.com";
pub const STAGING_AUTH: &str = "https://app-staging.no-tickets.com/api/auth/cli";

pub const LOCAL_API: &str = "http://localhost:5002";
pub const LOCAL_AUTH: &str = "http://localhost:5001/api/auth/cli";

#[derive(Debug)]
pub struct ResolvedUrls {
    pub api_url: String,
    pub auth_url: String,
}

#[derive(Debug)]
pub enum UrlError {
    PartialPair {
        which: &'static str,
        value: String,
        missing: &'static str,
    },
    UnknownEnv {
        value: String,
    },
    EnvAndPairBothSet {
        env_value: String,
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
            UrlError::UnknownEnv { value } => format!(
                "NO_TICKETS_ENV={value} is not a known preset. Known: staging, local, prod.",
            ),
            UrlError::EnvAndPairBothSet { env_value } => format!(
                "NO_TICKETS_ENV={env_value} is set together with NO_TICKETS_API_URL/NO_TICKETS_AUTH_URL. \
                 Set NO_TICKETS_ENV (preset) OR both URL vars (escape hatch), not both.",
            ),
        }
    }
}

pub fn resolve_urls(env: &dyn Env) -> Result<ResolvedUrls, UrlError> {
    let env_api = env.var("NO_TICKETS_API_URL").unwrap_or_default();
    let env_auth = env.var("NO_TICKETS_AUTH_URL").unwrap_or_default();
    let api_trim = env_api.trim();
    let auth_trim = env_auth.trim();
    let api_set = !api_trim.is_empty();
    let auth_set = !auth_trim.is_empty();

    if api_set != auth_set {
        let (which, value, missing) = if api_set {
            (
                "NO_TICKETS_API_URL",
                api_trim.to_string(),
                "NO_TICKETS_AUTH_URL",
            )
        } else {
            (
                "NO_TICKETS_AUTH_URL",
                auth_trim.to_string(),
                "NO_TICKETS_API_URL",
            )
        };
        return Err(UrlError::PartialPair {
            which,
            value,
            missing,
        });
    }

    // Both set or both unset by here. Read the env-preset knob so we can
    // enforce mutual exclusion before falling through to layer 1/2.
    let preset = env.var("NO_TICKETS_ENV").filter(|s| !s.trim().is_empty());

    if api_set {
        if let Some(env_value) = preset {
            return Err(UrlError::EnvAndPairBothSet { env_value });
        }
        // Layer 3 — explicit pair wins.
        return Ok(ResolvedUrls {
            api_url: api_trim.to_string(),
            auth_url: auth_trim.to_string(),
        });
    }

    // Layer 2 — preset table. Unset/empty preset falls through to layer 1.
    match preset.as_deref() {
        Some("staging") => Ok(ResolvedUrls {
            api_url: STAGING_API.to_string(),
            auth_url: STAGING_AUTH.to_string(),
        }),
        Some("local") => Ok(ResolvedUrls {
            api_url: LOCAL_API.to_string(),
            auth_url: LOCAL_AUTH.to_string(),
        }),
        // `prod` and `None` deliberately collapse: layer 1 (defaults) and
        // layer 2 (`NO_TICKETS_ENV=prod`) resolve to the same URLs. The
        // explicit `prod` value exists so users can document intent in
        // shell config; it MUST stay equivalent to "unset".
        Some("prod") | None => Ok(ResolvedUrls {
            api_url: DEFAULT_API.to_string(),
            auth_url: DEFAULT_AUTH.to_string(),
        }),
        Some(unknown) => Err(UrlError::UnknownEnv {
            value: unknown.to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::env::HashMapEnv;

    // Distinctive sentinels: cannot collide with host env state, so a
    // RED-state implementation (still reading process env) will fail
    // every assertion deterministically.
    const SENTINEL_API: &str = "https://red-phase-api.sentinel-z9q3.example";
    const SENTINEL_AUTH: &str = "https://red-phase-auth.sentinel-z9q3.example";

    #[test]
    fn resolve_urls_uses_injected_env_when_both_urls_set() {
        let env = HashMapEnv::with(&[
            ("NO_TICKETS_API_URL", SENTINEL_API),
            ("NO_TICKETS_AUTH_URL", SENTINEL_AUTH),
        ]);
        let resolved = resolve_urls(&env).expect("resolves");
        assert_eq!(resolved.api_url, SENTINEL_API);
        assert_eq!(resolved.auth_url, SENTINEL_AUTH);
    }

    #[test]
    fn resolve_urls_returns_defaults_when_injected_env_empty() {
        let env = HashMapEnv::empty();
        let resolved = resolve_urls(&env).expect("resolves");
        assert_eq!(resolved.api_url, DEFAULT_API);
        assert_eq!(resolved.auth_url, DEFAULT_AUTH);
    }

    #[test]
    fn resolve_urls_partial_pair_only_api_set_returns_error() {
        let env = HashMapEnv::with(&[("NO_TICKETS_API_URL", SENTINEL_API)]);
        let err = resolve_urls(&env).expect_err("partial pair errors");
        assert!(
            matches!(
                err,
                UrlError::PartialPair {
                    which: "NO_TICKETS_API_URL",
                    missing: "NO_TICKETS_AUTH_URL",
                    ..
                }
            ),
            "expected PartialPair (API set, AUTH missing); got {err:?}"
        );
    }

    #[test]
    fn resolve_urls_partial_pair_only_auth_set_returns_error() {
        let env = HashMapEnv::with(&[("NO_TICKETS_AUTH_URL", SENTINEL_AUTH)]);
        let err = resolve_urls(&env).expect_err("partial pair errors");
        assert!(matches!(
            err,
            UrlError::PartialPair {
                which: "NO_TICKETS_AUTH_URL",
                missing: "NO_TICKETS_API_URL",
                ..
            }
        ));
    }

    // ─── Three-layer resolution (ADR-0002) ───────────────────────────────

    #[test]
    fn resolve_urls_no_env_no_pair_returns_default_prod_urls() {
        let env = HashMapEnv::empty();
        let resolved = resolve_urls(&env).expect("layer 1 — defaults");
        assert_eq!(resolved.api_url, DEFAULT_API);
        assert_eq!(resolved.auth_url, DEFAULT_AUTH);
    }

    #[test]
    fn resolve_urls_env_preset_staging_returns_staging_urls() {
        let env = HashMapEnv::with(&[("NO_TICKETS_ENV", "staging")]);
        let resolved = resolve_urls(&env).expect("layer 2 — staging preset");
        assert_eq!(resolved.api_url, STAGING_API);
        assert_eq!(resolved.auth_url, STAGING_AUTH);
    }

    #[test]
    fn resolve_urls_env_preset_local_returns_local_urls() {
        let env = HashMapEnv::with(&[("NO_TICKETS_ENV", "local")]);
        let resolved = resolve_urls(&env).expect("layer 2 — local preset");
        assert_eq!(resolved.api_url, LOCAL_API);
        assert_eq!(resolved.auth_url, LOCAL_AUTH);
    }

    #[test]
    fn resolve_urls_env_preset_prod_returns_default_prod_urls() {
        let env = HashMapEnv::with(&[("NO_TICKETS_ENV", "prod")]);
        let resolved = resolve_urls(&env).expect("layer 2 — explicit prod preset");
        assert_eq!(resolved.api_url, DEFAULT_API);
        assert_eq!(resolved.auth_url, DEFAULT_AUTH);
    }

    #[test]
    fn resolve_urls_unknown_env_preset_returns_unknown_env_error() {
        let env = HashMapEnv::with(&[("NO_TICKETS_ENV", "bogus")]);
        let err = resolve_urls(&env).expect_err("unknown preset errors");
        assert!(
            matches!(&err, UrlError::UnknownEnv { value } if value == "bogus"),
            "expected UnknownEnv {{ value: \"bogus\" }}; got {err:?}",
        );
    }

    #[test]
    fn resolve_urls_env_and_explicit_pair_both_set_returns_mutual_exclusion_error() {
        let env = HashMapEnv::with(&[
            ("NO_TICKETS_ENV", "staging"),
            ("NO_TICKETS_API_URL", SENTINEL_API),
            ("NO_TICKETS_AUTH_URL", SENTINEL_AUTH),
        ]);
        let err = resolve_urls(&env).expect_err("env + pair both set errors");
        assert!(
            matches!(&err, UrlError::EnvAndPairBothSet { env_value } if env_value == "staging"),
            "expected EnvAndPairBothSet {{ env_value: \"staging\" }}; got {err:?}",
        );
    }

    #[test]
    fn resolve_urls_unknown_env_with_pair_set_surfaces_mutual_exclusion_not_unknown_env() {
        // When NO_TICKETS_ENV is invalid AND the explicit pair is set,
        // EnvAndPairBothSet wins over UnknownEnv — the mutual-exclusion
        // check runs before preset-value validation. Pinning this so a
        // future refactor that reorders the checks has to break this
        // test deliberately.
        let env = HashMapEnv::with(&[
            ("NO_TICKETS_ENV", "bogus"),
            ("NO_TICKETS_API_URL", SENTINEL_API),
            ("NO_TICKETS_AUTH_URL", SENTINEL_AUTH),
        ]);
        let err = resolve_urls(&env).expect_err("unknown env + pair errors");
        assert!(
            matches!(&err, UrlError::EnvAndPairBothSet { env_value } if env_value == "bogus"),
            "mutual-exclusion must win over unknown-preset validation; got {err:?}",
        );
    }

    #[test]
    fn resolve_urls_explicit_pair_wins_over_env_preset_when_only_pair_set() {
        // Layer 3 (explicit pair, both set) takes precedence when
        // NO_TICKETS_ENV is unset. The pair is the documented escape hatch.
        let env = HashMapEnv::with(&[
            ("NO_TICKETS_API_URL", SENTINEL_API),
            ("NO_TICKETS_AUTH_URL", SENTINEL_AUTH),
        ]);
        let resolved = resolve_urls(&env).expect("layer 3 — explicit pair");
        assert_eq!(resolved.api_url, SENTINEL_API);
        assert_eq!(resolved.auth_url, SENTINEL_AUTH);
    }

    #[test]
    fn resolve_urls_whitespace_only_env_preset_falls_through_to_defaults() {
        // Whitespace-only NO_TICKETS_ENV must behave like unset, not like
        // an unknown preset. Filter happens before the match.
        let env = HashMapEnv::with(&[("NO_TICKETS_ENV", "   ")]);
        let resolved = resolve_urls(&env).expect("whitespace-only treated as unset");
        assert_eq!(resolved.api_url, DEFAULT_API);
        assert_eq!(resolved.auth_url, DEFAULT_AUTH);
    }

    #[test]
    fn unknown_env_user_message_lists_known_presets() {
        let msg = UrlError::UnknownEnv { value: "qa".into() }.user_message();
        assert!(msg.contains("NO_TICKETS_ENV=qa"), "got: {msg}");
        assert!(msg.contains("staging"), "got: {msg}");
        assert!(msg.contains("local"), "got: {msg}");
        assert!(msg.contains("prod"), "got: {msg}");
    }

    #[test]
    fn env_and_pair_both_set_user_message_names_the_collision() {
        let msg = UrlError::EnvAndPairBothSet {
            env_value: "staging".into(),
        }
        .user_message();
        assert!(msg.contains("NO_TICKETS_ENV=staging"), "got: {msg}");
        assert!(msg.contains("NO_TICKETS_API_URL"), "got: {msg}");
        assert!(msg.contains("not both"), "got: {msg}");
    }

    #[test]
    fn resolve_urls_whitespace_only_injected_env_treated_as_unset() {
        // Whitespace-only values should be treated as unset by the
        // trim() inside resolve_urls — pinned both for the env-read
        // semantics and to mirror the existing integration test
        // `status_whitespace_only_env_url_counts_as_unset`.
        let env = HashMapEnv::with(&[
            ("NO_TICKETS_API_URL", "   "),
            ("NO_TICKETS_AUTH_URL", "\t\n"),
        ]);
        let resolved = resolve_urls(&env).expect("falls back to defaults");
        assert_eq!(resolved.api_url, DEFAULT_API);
        assert_eq!(resolved.auth_url, DEFAULT_AUTH);
    }
}
