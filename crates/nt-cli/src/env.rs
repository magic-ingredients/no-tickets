//! Environment-variable read port.
//!
//! Threads through `auth`, `urls`, `credentials`, and `paths` instead of
//! the inline `std::env::var(...)` calls that used to be sprinkled
//! across those modules. Production wires `SystemEnv`; tests substitute
//! a fake so resolution-branch coverage works without process-env
//! mutation (and runs in parallel without env races).
//!
//! Scope: env-var reads only. Filesystem reads (credentials file,
//! config.json profile loader) stay direct — the OS is the OS — and
//! tests sandbox those via `NO_TICKETS_HOME`, which is itself an env
//! var routed through this trait.

pub trait Env {
    fn var(&self, key: &str) -> Option<String>;
}

/// Production impl. Reads from the calling process's actual environment
/// via `std::env::var`.
///
/// Returns `Some(value)` if the var is set (including the empty string)
/// and is valid Unicode; `None` if the var is unset or not valid Unicode.
///
/// **The trait makes no judgment about what "set" means semantically.**
/// Different callers have different rules — `urls.rs` treats
/// whitespace-only as unset (via `trim().is_empty()`); `auth.rs` treats
/// empty as unset; `paths.rs` treats empty as unset for `NO_TICKETS_HOME`.
/// Each caller applies its own filter on top. Keeping the trait honest
/// about what's actually in the env (rather than collapsing) lets
/// callers express their own semantics without losing information.
pub struct SystemEnv;

impl Env for SystemEnv {
    fn var(&self, key: &str) -> Option<String> {
        std::env::var(key).ok()
    }
}

#[cfg(test)]
pub(crate) struct HashMapEnv {
    map: std::collections::HashMap<String, String>,
}

#[cfg(test)]
impl HashMapEnv {
    pub(crate) fn empty() -> Self {
        Self {
            map: std::collections::HashMap::new(),
        }
    }

    pub(crate) fn with(pairs: &[(&str, &str)]) -> Self {
        Self {
            map: pairs
                .iter()
                .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
                .collect(),
        }
    }
}

#[cfg(test)]
impl Env for HashMapEnv {
    fn var(&self, key: &str) -> Option<String> {
        self.map.get(key).cloned()
    }
}
