//! `nt logout` — delete the local session credentials file. No server call.
//!
//! Symmetric with `nt init`. Idempotent: a no-op if no credentials are present.

use std::fs;

use crate::env::Env;
use crate::paths;

pub fn run(env: &dyn Env) -> i32 {
    let Some(path) = paths::config_dir(env).map(|d| d.join(paths::CREDENTIALS_FILE)) else {
        eprintln!("Could not resolve config directory.");
        return 1;
    };
    match fs::remove_file(&path) {
        Ok(()) => {
            println!("Logged out.");
            0
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            println!("Already logged out.");
            0
        }
        Err(e) => {
            eprintln!("Could not delete credentials: {e}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::env::HashMapEnv;

    fn env_with_home(home: &std::path::Path) -> HashMapEnv {
        HashMapEnv::with(&[("NO_TICKETS_HOME", home.to_str().unwrap())])
    }

    #[test]
    fn logout_deletes_existing_credentials_file() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join(".notickets");
        fs::create_dir_all(&dir).unwrap();
        let creds = dir.join("credentials");
        fs::write(&creds, "anything").unwrap();
        assert!(creds.exists());
        let code = run(&env_with_home(tmp.path()));
        assert_eq!(code, 0);
        assert!(
            !creds.exists(),
            "credentials file must be gone after logout"
        );
    }

    #[test]
    fn logout_is_idempotent_when_no_credentials_present() {
        let tmp = tempfile::tempdir().unwrap();
        let code = run(&env_with_home(tmp.path()));
        assert_eq!(code, 0, "logout must succeed even with no credentials file");
    }
}
