//! Environment-variable read port.
//!
//! Threads through `auth`, `urls`, `credentials`, and `home` instead of
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
/// via `std::env::var`. Empty-string / unset / NotUnicode all collapse
/// to `None` — the historic semantics of the inline reads this module
/// replaces (every prior call site did `std::env::var(k).ok()` followed
/// by an `if !s.is_empty()` check at the caller).
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
