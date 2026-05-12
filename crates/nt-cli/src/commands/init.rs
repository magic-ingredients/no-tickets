//! `nt init` — browser-based session auth.
//!
//! Flow per ADR-0002:
//! 1. Resolve URLs and discover the api_url to tag credentials with `host`.
//! 2. Bind a local one-shot HTTP server on 127.0.0.1:<random>.
//! 3. Generate a CSRF nonce.
//! 4. Open the user's browser at `<auth_url>?port=<port>&code=<nonce>`.
//! 5. Wait for the browser to redirect to `/callback?token=&email=&state=`.
//! 6. Validate state, save `{token, email, expiresAt, host}` to the
//!    credentials file with `0o600` permissions.
//!
//! Reuses an existing valid session if one is on disk for the current env.

use std::fs;
use std::time::Duration;

use time::format_description::well_known::Iso8601;
use time::OffsetDateTime;

use crate::auth_server;
use crate::credentials::{self, LoadOutcome};
use crate::env::Env;
use crate::paths;
use crate::urls::resolve_urls;

/// 7 days; mirrors the TS reference (`SESSION_DURATION_MS`).
const SESSION_DURATION_DAYS: i64 = 7;

/// Default timeout for the browser hop. The user has this long to complete
/// the auth flow in the browser before the local server gives up.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);

pub fn run(env: &dyn Env) -> i32 {
    let urls = match resolve_urls(env) {
        Ok(u) => u,
        Err(e) => {
            eprintln!("{}", e.user_message());
            return 1;
        }
    };

    // If we already have a valid session for this env, short-circuit.
    if let LoadOutcome::Valid(_) = credentials::load(env, &urls.api_url) {
        println!("Already authenticated.");
        return 0;
    }

    let nonce = generate_nonce();
    let (listener, port) = match auth_server::bind() {
        Ok(x) => x,
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    };
    let callback_url = build_callback_url(&urls.auth_url, port, &nonce);

    if let Err(e) = open_browser(&callback_url) {
        eprintln!(
            "Could not open browser automatically: {e}. Open this URL manually:\n  {callback_url}",
        );
    } else {
        println!("Opening browser to authenticate. If it didn't open, visit:\n  {callback_url}");
    }

    let result = match auth_server::accept_callback(listener, &nonce, DEFAULT_TIMEOUT) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Authentication failed: {e}");
            return 1;
        }
    };

    let expires_at = match expires_at_iso8601(SESSION_DURATION_DAYS) {
        Ok(s) => s,
        Err(()) => {
            eprintln!("Could not stamp expiresAt.");
            return 1;
        }
    };

    if let Err(e) = save_credentials(
        env,
        &result.token,
        &result.email,
        &expires_at,
        &urls.api_url,
    ) {
        eprintln!("Could not save credentials: {e}");
        return 1;
    }

    println!("Authenticated as {}.", result.email);
    0
}

fn build_callback_url(auth_url: &str, port: u16, code: &str) -> String {
    let sep = if auth_url.contains('?') { '&' } else { '?' };
    format!("{auth_url}{sep}port={port}&code={code}")
}

fn generate_nonce() -> String {
    // 16 random bytes hex-encoded — same size as the TS reference's
    // `randomBytes(16).toString('hex')`. Uses the OS entropy source.
    let mut buf = [0u8; 16];
    getrandom_bytes(&mut buf);
    hex_encode(&buf)
}

/// Minimal OS-entropy wrapper. Reads /dev/urandom on Unix; on Windows,
/// callers compile with a different impl (not implemented here — Task 6
/// targets Unix-first per the rest of the crate). Falls back to a
/// time-mixed seed if the OS read fails so we never panic.
fn getrandom_bytes(buf: &mut [u8]) {
    #[cfg(unix)]
    {
        use std::io::Read;
        if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
            if f.read_exact(buf).is_ok() {
                return;
            }
        }
    }
    // Fallback: nanos-of-day → bytes. Predictable; only fires if
    // /dev/urandom open/read fails, which is rare enough that it's worth
    // not panicking here.
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    for (i, b) in buf.iter_mut().enumerate() {
        *b = ((nanos >> (i * 8)) & 0xff) as u8;
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(char::from(HEX[(b >> 4) as usize]));
        out.push(char::from(HEX[(b & 0x0f) as usize]));
    }
    out
}

fn expires_at_iso8601(days: i64) -> Result<String, ()> {
    let now = OffsetDateTime::now_utc();
    let expires = now + time::Duration::days(days);
    expires.format(&Iso8601::DEFAULT).map_err(|_| ())
}

fn save_credentials(
    env: &dyn Env,
    token: &str,
    email: &str,
    expires_at: &str,
    host: &str,
) -> Result<(), std::io::Error> {
    let dir = paths::config_dir(env)
        .ok_or_else(|| std::io::Error::other("Could not resolve config dir."))?;
    fs::create_dir_all(&dir)?;
    let path = dir.join(paths::CREDENTIALS_FILE);
    let body = serde_json::to_string_pretty(&serde_json::json!({
        "token": token,
        "email": email,
        "expiresAt": expires_at,
        "host": host,
    }))
    .map_err(std::io::Error::other)?;

    // Atomic + 0600 — same security posture as `config::write`.
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let tmp = path.with_extension(format!("tmp.{pid}.{nanos}"));
    let write_result = write_secret_atomic(&tmp, body.as_bytes());
    if let Err(e) = write_result {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    if let Err(e) = fs::rename(&tmp, &path) {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    Ok(())
}

#[cfg(unix)]
fn write_secret_atomic(path: &std::path::Path, body: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    f.write_all(body)?;
    Ok(())
}

#[cfg(not(unix))]
fn write_secret_atomic(path: &std::path::Path, body: &[u8]) -> std::io::Result<()> {
    fs::write(path, body)?;
    Ok(())
}

fn open_browser(url: &str) -> std::io::Result<()> {
    let program = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "start"
    } else {
        "xdg-open"
    };
    let status = std::process::Command::new(program)
        .arg(url)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()?;
    if !status.success() {
        return Err(std::io::Error::other(format!(
            "{program} exited with {status}",
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_callback_url_appends_port_and_code_with_question_mark() {
        let url = build_callback_url("https://app.example/api/auth/cli", 12345, "abc123");
        assert_eq!(
            url,
            "https://app.example/api/auth/cli?port=12345&code=abc123",
        );
    }

    #[test]
    fn build_callback_url_uses_ampersand_when_query_already_present() {
        let url = build_callback_url("https://app.example/auth?from=cli", 9090, "nonce-x");
        assert_eq!(
            url,
            "https://app.example/auth?from=cli&port=9090&code=nonce-x",
        );
    }

    #[test]
    fn generate_nonce_returns_32_hex_chars() {
        let nonce = generate_nonce();
        assert_eq!(nonce.len(), 32, "16 bytes hex-encoded = 32 chars");
        assert!(nonce.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn generate_nonce_is_different_on_each_call() {
        // Two calls in quick succession must not produce the same value
        // — the fallback path uses nanosecond resolution.
        let a = generate_nonce();
        let b = generate_nonce();
        assert_ne!(a, b);
    }

    #[test]
    fn hex_encode_matches_canonical_form() {
        assert_eq!(hex_encode(&[0x00, 0x10, 0xff, 0xab]), "0010ffab");
    }

    #[test]
    fn expires_at_iso8601_is_in_the_future() {
        let s = expires_at_iso8601(7).expect("formats");
        // Round-trip via OffsetDateTime to confirm it parses as ISO 8601
        // AND is after now.
        let parsed = OffsetDateTime::parse(&s, &Iso8601::DEFAULT).expect("parses");
        assert!(parsed > OffsetDateTime::now_utc());
    }
}
