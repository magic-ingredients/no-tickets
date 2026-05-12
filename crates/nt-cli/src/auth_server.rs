//! Local one-shot HTTP callback server for `nt init`. Minimal port of
//! `src/sdk/auth-server.ts`.
//!
//! Flow:
//! 1. The CLI starts this server on 127.0.0.1:<random-port>.
//! 2. The CLI opens the user's browser at `<auth_url>?port=<port>&code=<nonce>`.
//! 3. The web app authenticates the user and redirects the browser to
//!    `http://127.0.0.1:<port>/callback?token=<x>&email=<y>&state=<nonce>`.
//! 4. The server validates `state` against the nonce, responds with a small
//!    success page, and returns the `(token, email)` to the caller.
//!
//! Single accept, then shuts down. Times out if no callback in `timeout`.

use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::time::Duration;

const SUCCESS_HTML: &str = "<!doctype html><html><head><meta charset=utf-8><title>no-tickets — CLI authentication successful</title></head><body style=\"font-family:system-ui,sans-serif;display:flex;align-items:center;justify-content:center;height:100vh;margin:0\"><main style=\"text-align:center\"><h1>CLI authentication successful!</h1><p>You can close this tab.</p></main></body></html>";

#[derive(Debug)]
pub enum AuthServerError {
    Io(std::io::Error),
    Timeout,
    BadRequest,
    StateMismatch,
}

impl std::fmt::Display for AuthServerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthServerError::Io(e) => write!(f, "auth server I/O: {e}"),
            AuthServerError::Timeout => write!(f, "auth server timed out waiting for callback"),
            AuthServerError::BadRequest => write!(f, "auth server saw a malformed callback"),
            AuthServerError::StateMismatch => write!(f, "auth callback state did not match (CSRF)"),
        }
    }
}

impl std::error::Error for AuthServerError {}

impl From<std::io::Error> for AuthServerError {
    fn from(e: std::io::Error) -> Self {
        AuthServerError::Io(e)
    }
}

#[derive(Debug)]
pub struct CallbackResult {
    pub token: String,
    pub email: String,
}

/// Bind a TCP listener on 127.0.0.1 to a kernel-assigned port. Returns the
/// listener and the port it landed on. Caller is expected to call
/// `accept_callback` next.
pub fn bind() -> Result<(TcpListener, u16), AuthServerError> {
    let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))?;
    let port = listener.local_addr()?.port();
    Ok((listener, port))
}

/// Wait up to `timeout` for the browser to hit `/callback`, validate state
/// against `expected_state`, return token + email. Sends a small HTML page
/// back to the browser on success.
pub fn accept_callback(
    listener: TcpListener,
    expected_state: &str,
    timeout: Duration,
) -> Result<CallbackResult, AuthServerError> {
    listener.set_nonblocking(false)?;
    // accept() doesn't natively take a timeout. Set per-connection
    // read/write timeouts and emulate accept-timeout by polling with
    // set_nonblocking + sleep loop — simpler than rolling poll/epoll.
    let deadline = std::time::Instant::now() + timeout;
    listener.set_nonblocking(true)?;
    loop {
        match listener.accept() {
            Ok((stream, _addr)) => {
                listener.set_nonblocking(false)?;
                return handle_one(stream, expected_state);
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if std::time::Instant::now() >= deadline {
                    return Err(AuthServerError::Timeout);
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => return Err(AuthServerError::Io(e)),
        }
    }
}

fn handle_one(
    mut stream: TcpStream,
    expected_state: &str,
) -> Result<CallbackResult, AuthServerError> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;

    let mut buf = [0u8; 8192];
    let n = stream.read(&mut buf)?;
    let request = std::str::from_utf8(&buf[..n]).map_err(|_| AuthServerError::BadRequest)?;

    let (method, target) = parse_request_line(request).ok_or(AuthServerError::BadRequest)?;
    if method != "GET" {
        write_status(&mut stream, 405)?;
        return Err(AuthServerError::BadRequest);
    }
    let (path, query) = split_target(target);
    if path != "/callback" {
        write_status(&mut stream, 404)?;
        return Err(AuthServerError::BadRequest);
    }

    let token = get_query(query, "token");
    let email = get_query(query, "email");
    let state = get_query(query, "state");
    let (Some(token), Some(email), Some(state)) = (token, email, state) else {
        write_status(&mut stream, 400)?;
        return Err(AuthServerError::BadRequest);
    };
    if !constant_time_eq(state.as_bytes(), expected_state.as_bytes()) {
        write_status(&mut stream, 400)?;
        return Err(AuthServerError::StateMismatch);
    }

    write_success(&mut stream)?;
    Ok(CallbackResult { token, email })
}

fn parse_request_line(req: &str) -> Option<(&str, &str)> {
    let first_line = req.split("\r\n").next()?;
    let mut parts = first_line.split(' ');
    let method = parts.next()?;
    let target = parts.next()?;
    Some((method, target))
}

fn split_target(target: &str) -> (&str, &str) {
    match target.find('?') {
        Some(i) => (&target[..i], &target[i + 1..]),
        None => (target, ""),
    }
}

/// Extracts a query-string value by name. Does NOT do form-urlencoded
/// `+` → space substitution, so values like `alice+a@b.com` survive
/// verbatim. Mirrors the TS `getRawQueryValue`.
fn get_query(query: &str, key: &str) -> Option<String> {
    for pair in query.split('&') {
        let Some(eq) = pair.find('=') else { continue };
        let raw_key = &pair[..eq];
        let raw_value = &pair[eq + 1..];
        let Ok(decoded_key) = percent_decode(raw_key) else {
            continue;
        };
        if decoded_key == key {
            return percent_decode(raw_value).ok();
        }
    }
    None
}

fn percent_decode(s: &str) -> Result<String, ()> {
    let mut out = String::with_capacity(s.len());
    let mut bytes = s.bytes();
    while let Some(b) = bytes.next() {
        if b == b'%' {
            let h1 = bytes.next().ok_or(())?;
            let h2 = bytes.next().ok_or(())?;
            let hi = hex_nibble(h1).ok_or(())?;
            let lo = hex_nibble(h2).ok_or(())?;
            out.push(char::from((hi << 4) | lo));
        } else {
            out.push(char::from(b));
        }
    }
    Ok(out)
}

fn hex_nibble(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(10 + b - b'a'),
        b'A'..=b'F' => Some(10 + b - b'A'),
        _ => None,
    }
}

/// Constant-time byte compare. Bails early on length mismatch (length is
/// not sensitive — caller controls it). XORs each pair into an accumulator
/// to avoid short-circuiting on the first difference.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

fn write_status(stream: &mut TcpStream, code: u16) -> std::io::Result<()> {
    let body = format!("HTTP/1.1 {code}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
    stream.write_all(body.as_bytes())
}

fn write_success(stream: &mut TcpStream) -> std::io::Result<()> {
    let body = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        SUCCESS_HTML.len(),
        SUCCESS_HTML,
    );
    stream.write_all(body.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_request_line_extracts_method_and_target() {
        let (m, t) = parse_request_line("GET /callback?x=1 HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
            .expect("parses");
        assert_eq!(m, "GET");
        assert_eq!(t, "/callback?x=1");
    }

    #[test]
    fn parse_request_line_rejects_malformed_input() {
        assert!(parse_request_line("nonsense").is_none());
        assert!(parse_request_line("GET\r\n\r\n").is_none());
    }

    #[test]
    fn split_target_separates_path_and_query() {
        assert_eq!(split_target("/callback?a=1&b=2"), ("/callback", "a=1&b=2"));
        assert_eq!(split_target("/callback"), ("/callback", ""));
    }

    #[test]
    fn get_query_returns_value_when_key_present() {
        let q = "token=xyz&email=a%40b.com&state=abcd";
        assert_eq!(get_query(q, "token").as_deref(), Some("xyz"));
        assert_eq!(get_query(q, "email").as_deref(), Some("a@b.com"));
        assert_eq!(get_query(q, "state").as_deref(), Some("abcd"));
    }

    #[test]
    fn get_query_preserves_literal_plus_in_value() {
        // `+` in a value MUST NOT be rewritten to space — emails like
        // `alice+test@example.com` are common and we'd break them.
        let q = "email=alice+test%40example.com";
        assert_eq!(
            get_query(q, "email").as_deref(),
            Some("alice+test@example.com"),
        );
    }

    #[test]
    fn get_query_returns_none_when_key_missing() {
        assert!(get_query("a=1&b=2", "token").is_none());
    }

    #[test]
    fn percent_decode_handles_hex_and_returns_err_on_malformed() {
        assert_eq!(percent_decode("hello%20world").unwrap(), "hello world");
        assert!(percent_decode("%G0").is_err(), "non-hex must error");
        assert!(percent_decode("%2").is_err(), "truncated must error");
    }

    #[test]
    fn constant_time_eq_returns_true_for_equal_slices() {
        assert!(constant_time_eq(b"abc", b"abc"));
    }

    #[test]
    fn constant_time_eq_returns_false_for_different_length() {
        assert!(!constant_time_eq(b"abc", b"abcd"));
    }

    #[test]
    fn constant_time_eq_returns_false_for_same_length_different_content() {
        assert!(!constant_time_eq(b"abc", b"abd"));
    }

    // Integration-style: spin up the server in a thread, fire a real HTTP
    // request at it, confirm the handler returns the expected result.
    #[test]
    fn accept_callback_returns_token_email_on_valid_state() {
        let (listener, port) = bind().expect("bind");
        let handle = std::thread::spawn(move || {
            accept_callback(listener, "expected-state", Duration::from_secs(2))
        });
        // Tiny client: open a TCP connection and write a single HTTP GET.
        let mut s =
            std::net::TcpStream::connect(format!("127.0.0.1:{port}")).expect("client connect");
        let req = "GET /callback?token=nt_session_x&email=a%40b.com&state=expected-state HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n";
        s.write_all(req.as_bytes()).unwrap();
        // Drain response so server's write_all doesn't block.
        let mut resp = Vec::new();
        let _ = s.read_to_end(&mut resp);

        let result = handle.join().expect("thread joined").expect("ok");
        assert_eq!(result.token, "nt_session_x");
        assert_eq!(result.email, "a@b.com");
    }

    #[test]
    fn accept_callback_rejects_state_mismatch() {
        let (listener, port) = bind().expect("bind");
        let handle = std::thread::spawn(move || {
            accept_callback(listener, "expected-state", Duration::from_secs(2))
        });
        let mut s = std::net::TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
        let req = "GET /callback?token=x&email=y&state=WRONG HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n";
        s.write_all(req.as_bytes()).unwrap();
        let mut resp = Vec::new();
        let _ = s.read_to_end(&mut resp);
        let err = handle.join().unwrap().expect_err("must reject");
        assert!(matches!(err, AuthServerError::StateMismatch), "got {err:?}");
    }

    #[test]
    fn accept_callback_times_out_when_no_browser_callback_arrives() {
        let (listener, _port) = bind().expect("bind");
        let err =
            accept_callback(listener, "any", Duration::from_millis(150)).expect_err("times out");
        assert!(matches!(err, AuthServerError::Timeout), "got {err:?}");
    }
}
