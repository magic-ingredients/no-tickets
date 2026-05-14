//! Opt-in machine-hash attribute for `nt publish` source metadata.
//!
//! Mirrors `src/agent-detect.ts::detectSource` (TS reference) at the
//! one piece of behaviour that survived the "CI provenance is caller-
//! driven" rewrite: when `NO_TICKETS_INCLUDE_MACHINE=1` is set, emit
//! `source.attributes.machine = SHA-256("{salt}:{hostname}")[..16]`.
//!
//! The salt is generated once per installation and persisted at
//! `<config-dir>/.machine-salt` (resolved through `paths::config_dir`,
//! which respects `NO_TICKETS_HOME` for test sandboxing). The hashed
//! hostname identifies the producing machine without leaking the raw
//! hostname value to any party that doesn't also hold the salt.
//!
//! Best-effort: any filesystem failure (missing parent dir, no write
//! permission, etc.) silently drops the attribute. `nt publish` MUST
//! NOT fail because the machine hash couldn't be computed.

use std::path::PathBuf;

use crate::env::Env;
use crate::paths::config_dir;

/// Filename of the per-installation salt persisted inside `config_dir`.
/// 16 random bytes hex-encoded (32 chars), mode `0o600` on POSIX.
#[allow(dead_code)] // Used by Task 18 GREEN in commands::publish::build_metadata.
pub const MACHINE_SALT_FILE: &str = ".machine-salt";

/// Returns the machine-hash attribute value when opted in via
/// `NO_TICKETS_INCLUDE_MACHINE=1`, or `None` otherwise (env var unset,
/// env var set to any value other than the literal "1", or any
/// filesystem failure during salt persistence / read).
///
/// On success returns a 16-character lowercase-hex string: the first
/// 16 chars of `SHA-256("{salt}:{hostname}")`.
#[allow(dead_code)] // Wired by Task 18 GREEN into commands::publish::build_metadata.
pub fn machine_hash_attribute(_env: &dyn Env) -> Option<String> {
    unimplemented!("Task 18 GREEN — opt-in machine-hash attribute")
}

/// Resolved filesystem path of the salt file under the active config
/// dir. Exposed so the inline tests can assert presence + read it back
/// to drive "different salt → different hash" assertions.
#[allow(dead_code)] // Wired by Task 18 GREEN; used directly in inline tests.
pub fn machine_salt_path(env: &dyn Env) -> Option<PathBuf> {
    config_dir(env).map(|d| d.join(MACHINE_SALT_FILE))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::env::HashMapEnv;
    use std::fs;

    fn tempdir() -> tempfile::TempDir {
        tempfile::tempdir().expect("tempdir")
    }

    fn env_in(home: &std::path::Path, include_machine: Option<&str>) -> HashMapEnv {
        match include_machine {
            Some(v) => HashMapEnv::with(&[
                ("NO_TICKETS_HOME", home.to_str().expect("utf8 path")),
                ("NO_TICKETS_INCLUDE_MACHINE", v),
            ]),
            None => HashMapEnv::with(&[("NO_TICKETS_HOME", home.to_str().expect("utf8 path"))]),
        }
    }

    // ─── Env-var gate ──────────────────────────────────────────────────

    #[test]
    fn machine_hash_returns_none_when_include_machine_unset() {
        let home = tempdir();
        let env = env_in(home.path(), None);
        assert_eq!(machine_hash_attribute(&env), None);
    }

    #[test]
    fn machine_hash_returns_none_when_include_machine_not_one() {
        // Only the literal "1" enables. Any other value — empty string,
        // "0", "true", "yes" — leaves the attribute absent. Pinned per
        // value so a regression that loosens to "truthy" reads can't slip.
        for value in ["0", "true", "yes", "TRUE", "ON", ""] {
            let home = tempdir();
            let env = env_in(home.path(), Some(value));
            assert_eq!(
                machine_hash_attribute(&env),
                None,
                "value {value:?} must NOT enable the attribute",
            );
        }
    }

    // ─── Hash format + stability ──────────────────────────────────────

    #[test]
    fn machine_hash_returns_some_16_hex_chars_when_include_machine_one() {
        let home = tempdir();
        let env = env_in(home.path(), Some("1"));
        let hash = machine_hash_attribute(&env).expect("env=1 must produce a hash");
        assert_eq!(hash.len(), 16, "hash must be 16 chars; got {hash:?}");
        assert!(
            hash.chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
            "hash must be lowercase hex; got {hash:?}",
        );
    }

    #[test]
    fn machine_hash_is_stable_across_calls_with_same_salt() {
        let home = tempdir();
        let env = env_in(home.path(), Some("1"));
        let a = machine_hash_attribute(&env).expect("first call");
        let b = machine_hash_attribute(&env).expect("second call");
        assert_eq!(a, b, "stable hash across calls with same salt + hostname");
    }

    #[test]
    fn machine_hash_changes_when_salt_changes() {
        let home = tempdir();
        let env = env_in(home.path(), Some("1"));
        let hash_a = machine_hash_attribute(&env).expect("first call");
        // Overwrite the salt with a known different value.
        let salt_path = machine_salt_path(&env).expect("salt path resolves");
        fs::write(&salt_path, "totally-different-salt").expect("overwrite salt");
        let hash_b = machine_hash_attribute(&env).expect("second call");
        assert_ne!(hash_a, hash_b, "different salt MUST produce different hash");
    }

    // ─── Salt persistence ─────────────────────────────────────────────

    #[test]
    fn machine_hash_persists_salt_file_under_config_dir() {
        let home = tempdir();
        let env = env_in(home.path(), Some("1"));
        let _ = machine_hash_attribute(&env).expect("hash succeeds");
        let salt_path = machine_salt_path(&env).expect("salt path resolves");
        assert!(
            salt_path.exists(),
            "salt file must persist at {salt_path:?}"
        );
        let contents = fs::read_to_string(&salt_path).expect("salt readable");
        assert!(!contents.trim().is_empty(), "salt file must be non-empty");
    }

    #[test]
    fn machine_hash_reuses_existing_salt_file() {
        let home = tempdir();
        let env = env_in(home.path(), Some("1"));
        // Pre-seed a salt file before any hash call. The hash function
        // must read it (not regenerate).
        let salt_path = machine_salt_path(&env).expect("salt path resolves");
        fs::create_dir_all(salt_path.parent().unwrap()).expect("mkdir notickets");
        fs::write(&salt_path, "preexisting-salt-value").expect("seed salt");
        let a = machine_hash_attribute(&env).expect("first call reads seed");
        // After the call, salt file contents must still be the seed —
        // proves no regeneration happened.
        let still_seed = fs::read_to_string(&salt_path).expect("still readable");
        assert_eq!(
            still_seed, "preexisting-salt-value",
            "existing salt must be reused, not overwritten",
        );
        let b = machine_hash_attribute(&env).expect("second call");
        assert_eq!(a, b);
    }

    #[test]
    fn machine_hash_regenerates_salt_when_existing_file_is_empty() {
        let home = tempdir();
        let env = env_in(home.path(), Some("1"));
        let salt_path = machine_salt_path(&env).expect("salt path resolves");
        fs::create_dir_all(salt_path.parent().unwrap()).expect("mkdir notickets");
        fs::write(&salt_path, "").expect("seed empty salt");
        let hash = machine_hash_attribute(&env).expect("regenerates from empty seed");
        assert_eq!(hash.len(), 16);
        let regenerated = fs::read_to_string(&salt_path).expect("salt readable after regen");
        assert!(
            !regenerated.trim().is_empty(),
            "empty salt must be regenerated to non-empty",
        );
    }

    #[test]
    fn machine_hash_regenerates_salt_when_existing_file_is_whitespace_only() {
        let home = tempdir();
        let env = env_in(home.path(), Some("1"));
        let salt_path = machine_salt_path(&env).expect("salt path resolves");
        fs::create_dir_all(salt_path.parent().unwrap()).expect("mkdir notickets");
        fs::write(&salt_path, "   \n\t  ").expect("seed whitespace salt");
        let hash = machine_hash_attribute(&env).expect("regenerates from whitespace seed");
        assert_eq!(hash.len(), 16);
        let regenerated = fs::read_to_string(&salt_path).expect("salt readable");
        assert!(!regenerated.trim().is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn machine_hash_writes_salt_with_restrictive_perms_on_posix() {
        use std::os::unix::fs::PermissionsExt;
        let home = tempdir();
        let env = env_in(home.path(), Some("1"));
        let _ = machine_hash_attribute(&env).expect("hash succeeds");
        let salt_path = machine_salt_path(&env).expect("salt path resolves");
        let mode = fs::metadata(&salt_path)
            .expect("metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(
            mode, 0o600,
            "salt file must be 0o600 (user-only read/write); got {mode:o}",
        );
    }

    // ─── Best-effort: filesystem failures drop the attribute ──────────

    #[test]
    fn machine_hash_returns_none_when_config_dir_unwritable() {
        // Point NO_TICKETS_HOME at a path that can't be created (a
        // file, not a directory — so mkdir_all of a child fails).
        let blocker_file = tempfile::NamedTempFile::new().expect("tempfile");
        let home_pointing_at_a_file = blocker_file.path();
        let env = HashMapEnv::with(&[
            (
                "NO_TICKETS_HOME",
                home_pointing_at_a_file.to_str().expect("utf8"),
            ),
            ("NO_TICKETS_INCLUDE_MACHINE", "1"),
        ]);
        // Must NOT panic; must return None.
        let result = machine_hash_attribute(&env);
        assert_eq!(
            result, None,
            "unwritable config dir must drop the attribute silently",
        );
    }
}
