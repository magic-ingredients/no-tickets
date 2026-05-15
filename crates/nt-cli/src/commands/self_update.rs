//! `nt self-update` — direct-download binary upgrade path.
//!
//! Scoped to install.sh / direct-download installs. Package-manager
//! installs (Homebrew, Cargo, Scoop, npm) detect on launch and redirect
//! the user to their package manager instead of swapping the binary.
//!
//! Structure:
//! - `detect_install_kind` — pure path classification (no I/O)
//! - `redirect_message` — pure per-manager hint
//! - `is_downgrade` / `is_update_available` — pure semver comparison
//! - `fetch_latest_release` — minimal GitHub Releases client (`GET
//!   /repos/{owner}/{repo}/releases/latest`). Configurable base URL so
//!   wiremock can stand in for `api.github.com`
//! - `orchestrate` — pure decision tree over the above, returning an
//!   `UpdateOutcome` the caller maps to exit code + user output

// RED phase: the public surface below is exercised only by the in-file
// unit tests. `run()` doesn't wire the helpers yet (each helper is
// `unimplemented!()`), so the bin-target's dead-code analysis flags the
// public items as never used. GREEN replaces every stub with a real
// implementation and `run()` wires them, at which point this allow is
// removed.
#![allow(dead_code)]

use std::path::Path;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum InstallKind {
    Direct,
    Managed(Manager),
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Manager {
    Homebrew,
    Cargo,
    Scoop,
    Npm,
}

#[derive(Debug, PartialEq, Eq)]
pub enum UpdateOutcome {
    NoUpdate,
    Updated { from: String, to: String },
    ManagedRedirect(Manager),
    DowngradeRefused { current: String, latest: String },
    FetchFailed(String),
    SwapFailed { target: String, reason: String },
}

#[derive(Debug)]
pub struct LatestRelease {
    pub tag_name: String,
}

pub trait LatestFetcher {
    fn fetch(&self) -> Result<String, String>;
}

pub trait SwapPerformer {
    fn apply(&self, target_version: &str) -> Result<(), String>;
}

pub fn detect_install_kind(_exe_path: &Path) -> InstallKind {
    unimplemented!("detect_install_kind: GREEN phase pending")
}

pub fn redirect_message(_manager: Manager) -> String {
    unimplemented!("redirect_message: GREEN phase pending")
}

pub fn is_downgrade(_current: &str, _latest: &str) -> bool {
    unimplemented!("is_downgrade: GREEN phase pending")
}

pub fn is_update_available(_current: &str, _latest: &str) -> bool {
    unimplemented!("is_update_available: GREEN phase pending")
}

pub fn orchestrate<F: LatestFetcher, S: SwapPerformer>(
    _install_kind: InstallKind,
    _current_version: &str,
    _fetcher: &F,
    _swap: &S,
) -> UpdateOutcome {
    unimplemented!("orchestrate: GREEN phase pending")
}

pub async fn fetch_latest_release(
    _api_base: &str,
    _owner: &str,
    _repo: &str,
) -> Result<LatestRelease, String> {
    unimplemented!("fetch_latest_release: GREEN phase pending")
}

/// `nt self-update` subcommand entry point. Returns the process exit code.
///
/// RED-phase stub. GREEN wires `detect_install_kind` → `fetch_latest_release`
/// → `orchestrate` → `redirect_message` / atomic swap, mapping each
/// `UpdateOutcome` to its exit code + stdout/stderr message.
pub async fn run() -> i32 {
    unimplemented!("run: GREEN phase pending")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::path::PathBuf;

    // ---- detect_install_kind --------------------------------------------

    #[test]
    fn detect_homebrew_apple_silicon() {
        assert_eq!(
            detect_install_kind(&PathBuf::from("/opt/homebrew/bin/nt")),
            InstallKind::Managed(Manager::Homebrew)
        );
    }

    #[test]
    fn detect_homebrew_cellar() {
        assert_eq!(
            detect_install_kind(&PathBuf::from("/usr/local/Cellar/nt/0.1.0/bin/nt")),
            InstallKind::Managed(Manager::Homebrew)
        );
    }

    #[test]
    fn detect_homebrew_linuxbrew() {
        assert_eq!(
            detect_install_kind(&PathBuf::from("/home/linuxbrew/.linuxbrew/bin/nt")),
            InstallKind::Managed(Manager::Homebrew)
        );
    }

    #[test]
    fn detect_cargo_user_home() {
        assert_eq!(
            detect_install_kind(&PathBuf::from("/Users/alice/.cargo/bin/nt")),
            InstallKind::Managed(Manager::Cargo)
        );
    }

    #[test]
    fn detect_cargo_linux_home() {
        assert_eq!(
            detect_install_kind(&PathBuf::from("/home/alice/.cargo/bin/nt")),
            InstallKind::Managed(Manager::Cargo)
        );
    }

    #[test]
    fn detect_scoop_apps_current() {
        let path = PathBuf::from(r"C:\Users\alice\scoop\apps\nt\current\nt.exe");
        assert_eq!(
            detect_install_kind(&path),
            InstallKind::Managed(Manager::Scoop)
        );
    }

    #[test]
    fn detect_scoop_versioned() {
        let path = PathBuf::from(r"C:\Users\alice\scoop\apps\nt\0.1.0\nt.exe");
        assert_eq!(
            detect_install_kind(&path),
            InstallKind::Managed(Manager::Scoop)
        );
    }

    #[test]
    fn detect_npm_node_modules_bin() {
        assert_eq!(
            detect_install_kind(&PathBuf::from("/Users/alice/project/node_modules/.bin/nt")),
            InstallKind::Managed(Manager::Npm)
        );
    }

    #[test]
    fn detect_direct_usr_local() {
        // install.sh's default prefix lands binaries here on many systems.
        // Not under /usr/local/Cellar — that's Homebrew's intermediate
        // path; plain /usr/local/bin is direct.
        assert_eq!(
            detect_install_kind(&PathBuf::from("/usr/local/bin/nt")),
            InstallKind::Direct
        );
    }

    #[test]
    fn detect_direct_user_local_bin() {
        assert_eq!(
            detect_install_kind(&PathBuf::from("/home/alice/.local/bin/nt")),
            InstallKind::Direct
        );
    }

    #[test]
    fn detect_direct_arbitrary_path() {
        assert_eq!(
            detect_install_kind(&PathBuf::from("/opt/nt/nt")),
            InstallKind::Direct
        );
    }

    // ---- redirect_message -----------------------------------------------

    #[test]
    fn redirect_message_homebrew_names_brew_upgrade() {
        let msg = redirect_message(Manager::Homebrew);
        assert!(msg.contains("brew upgrade"), "got: {msg}");
    }

    #[test]
    fn redirect_message_cargo_names_cargo_install() {
        let msg = redirect_message(Manager::Cargo);
        assert!(msg.contains("cargo install"), "got: {msg}");
    }

    #[test]
    fn redirect_message_scoop_names_scoop_update() {
        let msg = redirect_message(Manager::Scoop);
        assert!(msg.contains("scoop update"), "got: {msg}");
    }

    #[test]
    fn redirect_message_npm_explains_npm_retired() {
        // npm distribution retired in Task 12 — if someone still has an
        // old install on their PATH the message must steer them to the
        // new channels, not pretend npm is still supported.
        let msg = redirect_message(Manager::Npm);
        let lc = msg.to_lowercase();
        assert!(
            lc.contains("retired") || lc.contains("brew") || lc.contains("install.sh"),
            "npm redirect must steer to new channels, got: {msg}"
        );
    }

    // ---- is_downgrade / is_update_available -----------------------------

    #[test]
    fn downgrade_higher_to_lower_is_true() {
        assert!(is_downgrade("0.2.0", "0.1.0"));
    }

    #[test]
    fn downgrade_lower_to_higher_is_false() {
        assert!(!is_downgrade("0.1.0", "0.2.0"));
    }

    #[test]
    fn downgrade_same_version_is_false() {
        assert!(!is_downgrade("0.1.0", "0.1.0"));
    }

    #[test]
    fn downgrade_strips_leading_v_on_either_side() {
        // GH tags often have leading "v"; comparison must normalise.
        assert!(is_downgrade("0.2.0", "v0.1.0"));
        assert!(is_downgrade("v0.2.0", "0.1.0"));
        assert!(!is_downgrade("v0.1.0", "0.2.0"));
    }

    #[test]
    fn update_available_when_latest_higher() {
        assert!(is_update_available("0.1.0", "0.2.0"));
    }

    #[test]
    fn update_available_strips_v_prefix() {
        assert!(is_update_available("0.1.0", "v0.2.0"));
        assert!(is_update_available("v0.1.0", "0.2.0"));
    }

    #[test]
    fn update_not_available_when_same_version() {
        assert!(!is_update_available("0.1.0", "0.1.0"));
    }

    #[test]
    fn update_not_available_when_latest_lower() {
        assert!(!is_update_available("0.2.0", "0.1.0"));
    }

    // ---- orchestrate ----------------------------------------------------

    // `dead_code` allows are temporary: the fields below are only read
    // through trait dispatch from `orchestrate`, which is `unimplemented!()`
    // in the RED phase. GREEN-phase code will exercise them.
    #[allow(dead_code)]
    struct StubFetcher(Result<String, String>);
    impl LatestFetcher for StubFetcher {
        fn fetch(&self) -> Result<String, String> {
            self.0.clone()
        }
    }

    #[allow(dead_code)]
    struct StubSwap {
        called_with: RefCell<Option<String>>,
        result: Result<(), String>,
    }
    impl StubSwap {
        fn ok() -> Self {
            Self {
                called_with: RefCell::new(None),
                result: Ok(()),
            }
        }
        fn err(reason: &str) -> Self {
            Self {
                called_with: RefCell::new(None),
                result: Err(reason.to_string()),
            }
        }
    }
    impl SwapPerformer for StubSwap {
        fn apply(&self, target_version: &str) -> Result<(), String> {
            *self.called_with.borrow_mut() = Some(target_version.to_string());
            self.result.clone()
        }
    }

    #[test]
    fn orchestrate_managed_install_returns_redirect_without_fetching_or_swapping() {
        // The fetcher returning Err would fail the test if it were ever
        // called — pin: managed-install path short-circuits before any
        // network or swap activity.
        let fetcher = StubFetcher(Err("must not call fetcher on managed install".into()));
        let swap = StubSwap::ok();
        let outcome = orchestrate(
            InstallKind::Managed(Manager::Homebrew),
            "0.1.0",
            &fetcher,
            &swap,
        );
        assert_eq!(outcome, UpdateOutcome::ManagedRedirect(Manager::Homebrew));
        assert!(
            swap.called_with.borrow().is_none(),
            "swap must not run on managed install"
        );
    }

    #[test]
    fn orchestrate_no_update_when_current_matches_latest() {
        let fetcher = StubFetcher(Ok("0.1.0".into()));
        let swap = StubSwap::ok();
        let outcome = orchestrate(InstallKind::Direct, "0.1.0", &fetcher, &swap);
        assert_eq!(outcome, UpdateOutcome::NoUpdate);
        assert!(swap.called_with.borrow().is_none());
    }

    #[test]
    fn orchestrate_refuses_downgrade_without_swapping() {
        let fetcher = StubFetcher(Ok("0.0.9".into()));
        let swap = StubSwap::ok();
        let outcome = orchestrate(InstallKind::Direct, "0.1.0", &fetcher, &swap);
        assert_eq!(
            outcome,
            UpdateOutcome::DowngradeRefused {
                current: "0.1.0".into(),
                latest: "0.0.9".into(),
            }
        );
        assert!(swap.called_with.borrow().is_none());
    }

    #[test]
    fn orchestrate_applies_swap_on_available_update() {
        let fetcher = StubFetcher(Ok("0.2.0".into()));
        let swap = StubSwap::ok();
        let outcome = orchestrate(InstallKind::Direct, "0.1.0", &fetcher, &swap);
        assert_eq!(
            outcome,
            UpdateOutcome::Updated {
                from: "0.1.0".into(),
                to: "0.2.0".into(),
            }
        );
        assert_eq!(swap.called_with.borrow().as_deref(), Some("0.2.0"));
    }

    #[test]
    fn orchestrate_normalises_v_prefix_from_latest_tag() {
        let fetcher = StubFetcher(Ok("v0.2.0".into()));
        let swap = StubSwap::ok();
        let outcome = orchestrate(InstallKind::Direct, "0.1.0", &fetcher, &swap);
        // Updated `to` carries the normalised version (no "v"), so the
        // user-visible "X → Y" line stays consistent regardless of tag
        // shape.
        assert_eq!(
            outcome,
            UpdateOutcome::Updated {
                from: "0.1.0".into(),
                to: "0.2.0".into(),
            }
        );
    }

    #[test]
    fn orchestrate_fetch_error_surfaces_as_fetch_failed() {
        let fetcher = StubFetcher(Err("network down".into()));
        let swap = StubSwap::ok();
        let outcome = orchestrate(InstallKind::Direct, "0.1.0", &fetcher, &swap);
        match outcome {
            UpdateOutcome::FetchFailed(msg) => assert!(
                msg.contains("network down"),
                "FetchFailed must carry the underlying reason, got: {msg}"
            ),
            other => panic!("expected FetchFailed, got {other:?}"),
        }
        assert!(swap.called_with.borrow().is_none());
    }

    #[test]
    fn orchestrate_swap_error_surfaces_as_swap_failed() {
        let fetcher = StubFetcher(Ok("0.2.0".into()));
        let swap = StubSwap::err("sha256 mismatch");
        let outcome = orchestrate(InstallKind::Direct, "0.1.0", &fetcher, &swap);
        match outcome {
            UpdateOutcome::SwapFailed { target, reason } => {
                assert_eq!(target, "0.2.0");
                assert!(reason.contains("sha256 mismatch"), "got: {reason}");
            }
            other => panic!("expected SwapFailed, got {other:?}"),
        }
        // Swap was attempted (and failed) — distinguishes this from
        // FetchFailed where swap is never called.
        assert_eq!(swap.called_with.borrow().as_deref(), Some("0.2.0"));
    }

    // ---- fetch_latest_release (wiremock) --------------------------------

    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn fetch_latest_release_parses_tag_name_from_github_response() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/repos/magic-ingredients/no-tickets/releases/latest"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "tag_name": "v0.1.5",
                "name": "0.1.5",
            })))
            .mount(&server)
            .await;

        let release = fetch_latest_release(&server.uri(), "magic-ingredients", "no-tickets")
            .await
            .expect("fetch should succeed");
        assert_eq!(release.tag_name, "v0.1.5");
    }

    #[tokio::test]
    async fn fetch_latest_release_sends_github_api_v3_accept_header() {
        // GitHub's API contract requires the v3 accept header for stable
        // schemas. Pin it so a future request-builder refactor can't
        // silently drop it (the API would still respond, but with a
        // different default representation).
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/repos/foo/bar/releases/latest"))
            .and(header("accept", "application/vnd.github+json"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "tag_name": "v0.1.0",
            })))
            .mount(&server)
            .await;

        let result = fetch_latest_release(&server.uri(), "foo", "bar").await;
        assert!(
            result.is_ok(),
            "expected accept-header match, got {result:?}"
        );
    }

    #[tokio::test]
    async fn fetch_latest_release_sends_user_agent_header() {
        // GitHub rejects requests without a User-Agent. Pin that the
        // fetcher sets one so a wrapper's no-UA default never leaks
        // through.
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/repos/foo/bar/releases/latest"))
            .and(header("user-agent", "nt-self-update"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "tag_name": "v0.1.0",
            })))
            .mount(&server)
            .await;

        let result = fetch_latest_release(&server.uri(), "foo", "bar").await;
        assert!(result.is_ok(), "expected user-agent match, got {result:?}");
    }

    #[tokio::test]
    async fn fetch_latest_release_returns_err_on_5xx() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;

        let result = fetch_latest_release(&server.uri(), "foo", "bar").await;
        assert!(result.is_err(), "503 must surface as Err");
    }

    #[tokio::test]
    async fn fetch_latest_release_returns_err_on_404() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let result = fetch_latest_release(&server.uri(), "foo", "missing").await;
        assert!(result.is_err(), "404 must surface as Err");
    }

    #[tokio::test]
    async fn fetch_latest_release_returns_err_on_malformed_body() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
            .mount(&server)
            .await;

        let result = fetch_latest_release(&server.uri(), "foo", "bar").await;
        assert!(result.is_err(), "malformed JSON must surface as Err");
    }

    #[tokio::test]
    async fn fetch_latest_release_returns_err_on_missing_tag_name() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "name": "0.1.0",
                // tag_name omitted
            })))
            .mount(&server)
            .await;

        let result = fetch_latest_release(&server.uri(), "foo", "bar").await;
        assert!(result.is_err(), "missing tag_name must surface as Err");
    }
}
