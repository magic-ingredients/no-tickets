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

use std::path::{Path, PathBuf};

use rand::RngCore;
use sha2::{Digest, Sha256};

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
pub fn machine_hash_attribute(env: &dyn Env) -> Option<String> {
    if env.var("NO_TICKETS_INCLUDE_MACHINE").as_deref() != Some("1") {
        return None;
    }
    // Best-effort: any filesystem failure here drops the attribute.
    let salt_path = machine_salt_path(env)?;
    let salt = read_or_create_salt(&salt_path).ok()?;
    let host = hostname::get().ok()?.to_string_lossy().into_owned();
    Some(compute_hash(&salt, &host))
}

/// Pure hash function — `SHA-256("{salt}:{hostname}")[..8]` hex-encoded.
///
/// Extracted from `machine_hash_attribute` so the salt-and-hostname
/// concatenation, the separator byte, AND the 8-byte digest slice all
/// fall under direct unit-test coverage. The golden-vector test pins
/// each of these against mutations that would otherwise survive (drop
/// `hasher.update(host)`, change `b":"` to `b""` / `b"|"`, slice
/// `[8..16]` instead of `[..8]`).
fn compute_hash(salt: &str, hostname: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(salt.as_bytes());
    hasher.update(b":");
    hasher.update(hostname.as_bytes());
    let digest = hasher.finalize();
    // First 8 bytes → 16 lowercase-hex chars.
    let mut hex = String::with_capacity(16);
    for byte in &digest[..8] {
        use std::fmt::Write as _;
        // SAFETY: writing to a String can't fail.
        let _ = write!(&mut hex, "{byte:02x}");
    }
    hex
}

/// Resolved filesystem path of the salt file under the active config
/// dir. Exposed so the inline tests can assert presence + read it back
/// to drive "different salt → different hash" assertions.
pub fn machine_salt_path(env: &dyn Env) -> Option<PathBuf> {
    config_dir(env).map(|d| d.join(MACHINE_SALT_FILE))
}

/// Read the existing salt or create a fresh one. The salt is 16
/// random bytes hex-encoded (32 chars). Empty or whitespace-only
/// existing files are treated as missing and regenerated.
///
/// This is NOT atomic across concurrent first-runs: two parallel
/// processes both generate fresh salts and last-write-wins. The
/// collision window is small (a single mkdir + write) and the
/// loser's salt was never observed, so the downstream stable-hash-
/// across-calls invariant still holds for each process individually.
/// Acceptable for the audit-trail use case; not suitable for any
/// salt with security-token semantics.
///
/// Returns `Err` for any I/O failure the caller hasn't already
/// guarded against, including a parent directory that can't be
/// created (e.g. the `NO_TICKETS_HOME` override points at a file).
fn read_or_create_salt(path: &Path) -> std::io::Result<String> {
    // Single trim() call rather than guard-then-trim-again: the guard
    // form re-trimmed on the return path which doubled the allocation
    // and parsing cost on every cache-hit invocation.
    if let Ok(existing) = std::fs::read_to_string(path) {
        let trimmed = existing.trim();
        if !trimmed.is_empty() {
            return Ok(trimmed.to_string());
        }
        // Fall through to regen: file present but empty/whitespace.
    }
    // File missing, unreadable as utf-8, or empty/whitespace — regen.

    let mut bytes = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut bytes);
    let mut salt = String::with_capacity(32);
    for byte in &bytes {
        use std::fmt::Write as _;
        let _ = write!(&mut salt, "{byte:02x}");
    }
    // Ensure parent dir exists. If creation fails (e.g. parent is
    // a file, not a dir), propagate so the caller drops the attribute.
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    write_salt_with_restrictive_perms(path, &salt)?;
    Ok(salt)
}

#[cfg(unix)]
fn write_salt_with_restrictive_perms(path: &Path, salt: &str) -> std::io::Result<()> {
    use std::fs::Permissions;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    std::io::Write::write_all(&mut file, salt.as_bytes())?;
    // `OpenOptions::mode(0o600)` is ONLY honoured on file creation —
    // if the file pre-existed (e.g. an attacker pre-seeded an empty
    // world-readable file to trigger the regen path), the mode would
    // stay at the pre-existing value (e.g. 0o644). Force it post-write
    // so the regen path lands the same perms as a clean create.
    std::fs::set_permissions(path, Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn write_salt_with_restrictive_perms(path: &Path, salt: &str) -> std::io::Result<()> {
    // Windows has no POSIX mode bits; the file inherits the parent's
    // ACL. The .machine-salt dotfile under the user's profile dir is
    // already user-scoped via the OS conventions.
    std::fs::write(path, salt)
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
        // Only the literal "1" enables. Any other value leaves the
        // attribute absent. Table includes:
        //   - "" — empty (pins against `unwrap_or_default()` regressions)
        //   - "0" — literal false
        //   - "true", "yes", "TRUE", "ON" — common truthy aliases
        //     (pins against any future "parse as bool" loosening)
        //   - "2", "11" — pin against `starts_with("1")` / `contains("1")`
        //     / `parse::<u32>() != 0` reads (all would let "2" enable)
        //   - " 1", "1 " — pin against `trim() == "1"` (would let
        //     "  1" enable; the contract is strict equality, no trim)
        for value in ["", "0", "true", "yes", "TRUE", "ON", "2", "11", " 1", "1 "] {
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

    // ─── compute_hash: pure golden vectors ────────────────────────────

    #[test]
    fn compute_hash_golden_vector_pins_salt_separator_and_digest_slice() {
        // Offline-computed reference value:
        //   printf 'abc-salt:host-example' | shasum -a 256 | cut -c1-16
        //   → 6bbbb393af39cc0c
        //
        // This single assertion pins THREE behaviours simultaneously:
        //   - the `b":"` separator (changing to `b""` or `b"|"` yields
        //     a totally different digest → different prefix)
        //   - the salt-then-hostname concatenation order (swapping
        //     produces a different digest)
        //   - the `[..8]` digest slice (taking `[8..16]` yields a
        //     different prefix; the test pins the exact one we want)
        assert_eq!(compute_hash("abc-salt", "host-example"), "6bbbb393af39cc0c",);
    }

    #[test]
    fn compute_hash_changes_when_hostname_changes() {
        // Pins that `hasher.update(hostname.as_bytes())` is actually
        // wired — a mutation that drops this update or replaces
        // `hostname` with a constant survives every test that uses
        // the real OS hostname (which doesn't vary across the suite).
        //
        // Offline-computed via `printf '...:...' | shasum -a 256 | cut -c1-16`:
        //   "abc-salt:host-example" → 6bbbb393af39cc0c
        //   "abc-salt:host-other"   → 3c1ba8c360894bde
        let a = compute_hash("abc-salt", "host-example");
        let b = compute_hash("abc-salt", "host-other");
        assert_ne!(a, b, "different hostname MUST produce different hash");
        // And pin both values so a mutation that drops the salt
        // update AND swaps inputs can't accidentally still pass.
        assert_eq!(a, "6bbbb393af39cc0c");
        assert_eq!(b, "3c1ba8c360894bde");
    }

    #[test]
    fn compute_hash_returns_16_lowercase_hex_chars_for_arbitrary_inputs() {
        for (salt, host) in [
            ("a", "b"),
            ("", ""),
            ("very-long-salt-value-that-exceeds-32-chars-x", "h"),
            ("salt-with-colon:embedded", "host"), // separator must not confuse parser
        ] {
            let h = compute_hash(salt, host);
            assert_eq!(h.len(), 16, "shape for salt={salt:?} host={host:?}");
            assert!(
                h.chars()
                    .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
                "lowercase hex for salt={salt:?} host={host:?}, got {h:?}",
            );
        }
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
        // Pin the SHAPE of the regenerated salt, not just "non-empty".
        // A mutation that writes `"."` would pass a `!is_empty()` check
        // but fail this one. The 32-char + hex-charset assertion
        // protects the contract that the salt is a 16-byte random value
        // hex-encoded.
        assert_eq!(
            regenerated.trim().len(),
            32,
            "regenerated salt must be 32 hex chars (16 random bytes); got {regenerated:?}",
        );
        assert!(
            regenerated
                .trim()
                .chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
            "regenerated salt must be lowercase hex; got {regenerated:?}",
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

    #[cfg(unix)]
    #[test]
    fn machine_hash_regen_path_forces_0o600_even_when_existing_file_is_world_readable() {
        // Pre-existing world-readable empty salt file simulates the
        // attacker / stale-state scenario where someone (or a bad
        // umask on a prior run) left an empty `.machine-salt` with
        // mode 0o644. The regen path MUST land 0o600 — the
        // `OpenOptions::mode(...)` flag alone is silently ignored
        // when the file pre-exists, so this test pins the explicit
        // `set_permissions` step after the write.
        use std::fs::Permissions;
        use std::os::unix::fs::PermissionsExt;
        let home = tempdir();
        let env = env_in(home.path(), Some("1"));
        let salt_path = machine_salt_path(&env).expect("salt path resolves");
        fs::create_dir_all(salt_path.parent().unwrap()).expect("mkdir notickets");
        fs::write(&salt_path, "").expect("seed empty salt");
        fs::set_permissions(&salt_path, Permissions::from_mode(0o644)).expect("seed wide perms");
        let _ = machine_hash_attribute(&env).expect("regenerates from empty seed");
        let mode = fs::metadata(&salt_path)
            .expect("metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(
            mode, 0o600,
            "regen path must force 0o600 even when pre-existing file had wider perms; got {mode:o}",
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
