//! Home-directory resolution.
//!
//! NO_TICKETS_HOME wins over the OS home dir. On Unix the OS home is HOME;
//! on Windows it's USERPROFILE. Matches the TS reference implementation
//! (`process.env['NO_TICKETS_HOME'] || os.homedir()`).

use std::path::PathBuf;

pub fn home_dir() -> Option<PathBuf> {
    if let Ok(h) = std::env::var("NO_TICKETS_HOME") {
        if !h.is_empty() {
            return Some(PathBuf::from(h));
        }
    }
    #[cfg(unix)]
    {
        std::env::var("HOME").ok().filter(|s| !s.is_empty()).map(PathBuf::from)
    }
    #[cfg(windows)]
    {
        std::env::var("USERPROFILE").ok().filter(|s| !s.is_empty()).map(PathBuf::from)
    }
}

pub fn credentials_path() -> Option<PathBuf> {
    Some(home_dir()?.join(".notickets").join("credentials"))
}

pub fn config_path() -> Option<PathBuf> {
    Some(home_dir()?.join(".notickets").join("config.json"))
}
