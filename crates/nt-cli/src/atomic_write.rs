//! Atomic write-via-tmp-rename for non-secret files.
//!
//! Used by `session.rs` and `state.rs` for `active-session.json` and
//! `state.json`. `config.rs` keeps its own implementation because the
//! push-token file additionally needs Unix mode `0o600` set
//! at-create-time on the tmp — that constraint complicates the helper
//! for the more common non-secret case, so the secret variant lives
//! alongside the file it serves.
//!
//! Properties pinned by the using modules' tests:
//!   - Parent directory is created on demand.
//!   - The tmp file uses a PID + nanos suffix so two concurrent writers
//!     to the same destination never share a tmp name.
//!   - The body is `sync_all`'d before rename — a crash between write
//!     and rename leaves no half-written file in the destination
//!     location.
//!   - On any failure between create and rename, the tmp is scrubbed so
//!     leftover plaintext doesn't linger on disk.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Build the PID+nanos-suffixed temp path that sits next to `dest`.
/// Same parent → POSIX rename is atomic within the destination
/// filesystem.
pub fn tmp_path_for(dest: &Path) -> PathBuf {
    let parent = dest.parent().unwrap_or_else(|| Path::new("."));
    let filename = dest
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    parent.join(format!("{filename}.tmp.{pid}.{nanos}"))
}

/// Atomic write of `body` to `dest`. Creates parent directories on
/// demand, syncs the tmp, then renames into place. Scrubs the tmp on
/// any failure path.
pub fn write_atomic(dest: &Path, body: &[u8]) -> io::Result<()> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = tmp_path_for(dest);
    let write_then_sync = (|| -> io::Result<()> {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(body)?;
        f.sync_all()?;
        Ok(())
    })();
    if let Err(e) = write_then_sync {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    if let Err(e) = fs::rename(&tmp, dest) {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_atomic_creates_destination() {
        let tmp = tempfile::tempdir().unwrap();
        let dest = tmp.path().join("nested").join("dir").join("file.json");
        write_atomic(&dest, br#"{"hi":1}"#).expect("write");
        assert_eq!(fs::read_to_string(&dest).unwrap(), r#"{"hi":1}"#);
    }

    #[test]
    fn write_atomic_replaces_existing_destination() {
        let tmp = tempfile::tempdir().unwrap();
        let dest = tmp.path().join("file.json");
        write_atomic(&dest, b"first").unwrap();
        write_atomic(&dest, b"second").unwrap();
        assert_eq!(fs::read_to_string(&dest).unwrap(), "second");
    }

    #[test]
    fn write_atomic_leaves_no_tmp_files_in_parent() {
        let tmp = tempfile::tempdir().unwrap();
        let dest = tmp.path().join("file.json");
        write_atomic(&dest, b"body").unwrap();
        for entry in fs::read_dir(tmp.path()).unwrap() {
            let name = entry.unwrap().file_name().into_string().unwrap();
            assert!(!name.contains(".tmp."), "tmp leftover: {name}");
        }
    }

    #[test]
    fn tmp_path_sits_next_to_destination() {
        let dest = Path::new("/tmp/foo/active-session.json");
        let tmp = tmp_path_for(dest);
        assert_eq!(tmp.parent(), Some(Path::new("/tmp/foo")));
        let name = tmp.file_name().unwrap().to_string_lossy();
        assert!(name.starts_with("active-session.json.tmp."), "got {name}");
    }
}
