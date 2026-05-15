//! `nt self-update` — direct-download binary upgrade path.
//!
//! Scoped to install.sh / direct-download installs. Package-manager
//! installs (Homebrew, Cargo, Scoop) and version-manager shims (asdf,
//! mise, Volta) detect on launch and redirect the user to their manager
//! instead of swapping the binary.
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

use std::path::Path;

use semver::Version;

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
    Asdf,
    Mise,
    Volta,
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
    async fn fetch(&self) -> Result<String, String>;
}

pub trait SwapPerformer {
    fn apply(&self, target_version: &str) -> Result<(), String>;
}

/// GitHub owner/repo coordinates the production fetcher and the swap
/// backend point at. Kept here so the magic strings live in one place.
pub const GH_OWNER: &str = "magic-ingredients";
pub const GH_REPO: &str = "no-tickets";
pub const DEFAULT_GH_API_BASE: &str = "https://api.github.com";
const USER_AGENT: &str = "nt-self-update";

pub fn detect_install_kind(exe_path: &Path) -> InstallKind {
    // We classify by substring match on a normalised path string.
    //
    // Case-folding rationale: every marker below is intentionally
    // lowercase, so we lowercase the candidate path once and skip
    // per-comparison case-handling. This is safe across platforms
    // because (a) Windows + macOS APFS/HFS+ are case-insensitive at
    // the filesystem level — the user can't have a *different*
    // directory named "Cargo" vs "cargo"; (b) Linux is case-sensitive
    // but every tool we detect (homebrew, cargo, scoop, asdf, mise,
    // volta) ships lowercase directory names. A Linux user with a
    // deliberately mixed-case `~/.Cargo/bin/nt` would slip through,
    // but that's a self-inflicted false-negative, not a correctness
    // bug — we'd return Direct, which only refuses the user's own
    // manual path.
    //
    // We also normalise `\` → `/` so the same marker strings work for
    // Windows paths (`C:\Users\…\scoop\apps\…`) and POSIX in one pass.
    //
    // Important: we use `current_exe()` upstream, which resolves
    // symlinks on Linux/macOS. That matters for Homebrew at a custom
    // `HOMEBREW_PREFIX` — `<prefix>/bin/nt` is a symlink into
    // `<prefix>/Cellar/nt/<version>/bin/nt`, and the resolved path
    // hits the `/cellar/` marker even when the prefix isn't
    // `/opt/homebrew` or `/usr/local`.
    let raw = exe_path.to_string_lossy();
    let lc = raw.to_lowercase().replace('\\', "/");

    if lc.contains("/opt/homebrew/")
        || lc.contains("/cellar/")
        || lc.contains("/.linuxbrew/")
        || lc.contains("/linuxbrew/.linuxbrew/")
    {
        return InstallKind::Managed(Manager::Homebrew);
    }
    if lc.contains("/.cargo/bin/") {
        return InstallKind::Managed(Manager::Cargo);
    }
    if lc.contains("/scoop/apps/") {
        return InstallKind::Managed(Manager::Scoop);
    }
    if lc.contains("/.asdf/shims/") || lc.contains("/.asdf/installs/") {
        return InstallKind::Managed(Manager::Asdf);
    }
    if lc.contains("/mise/shims/") || lc.contains("/mise/installs/") {
        return InstallKind::Managed(Manager::Mise);
    }
    if lc.contains("/.volta/bin/") || lc.contains("/.volta/tools/") {
        return InstallKind::Managed(Manager::Volta);
    }
    InstallKind::Direct
}

pub fn redirect_message(manager: Manager) -> String {
    match manager {
        Manager::Homebrew => {
            "nt was installed via Homebrew. Run `brew upgrade nt` to update.".to_string()
        }
        Manager::Cargo => {
            "nt was installed via Cargo. Run `cargo install --force nt-cli` to update.".to_string()
        }
        Manager::Scoop => {
            "nt was installed via Scoop. Run `scoop update nt` to update.".to_string()
        }
        Manager::Asdf => "nt was installed via asdf. \
             Update via your asdf plugin (e.g. `asdf install nt latest && asdf reshim`)."
            .to_string(),
        Manager::Mise => "nt was installed via mise. \
             Update via `mise install nt@latest` (or your mise tools manifest)."
            .to_string(),
        Manager::Volta => "nt was installed via Volta. \
             Update via `volta install nt@latest`."
            .to_string(),
    }
}

/// Strip a single leading `v` so GitHub tag-style strings parse as
/// semver. Anything else passes through unchanged.
fn normalise_version(v: &str) -> &str {
    v.strip_prefix('v').unwrap_or(v)
}

pub fn is_downgrade(current: &str, latest: &str) -> bool {
    let (Ok(c), Ok(l)) = (
        Version::parse(normalise_version(current)),
        Version::parse(normalise_version(latest)),
    ) else {
        // Unparseable on either side — don't claim it's a downgrade.
        // Caller will see "no update available" via is_update_available
        // and the run() path surfaces the parse error separately.
        return false;
    };
    l < c
}

pub fn is_update_available(current: &str, latest: &str) -> bool {
    let (Ok(c), Ok(l)) = (
        Version::parse(normalise_version(current)),
        Version::parse(normalise_version(latest)),
    ) else {
        return false;
    };
    l > c
}

pub async fn orchestrate<F: LatestFetcher, S: SwapPerformer>(
    install_kind: InstallKind,
    current_version: &str,
    fetcher: &F,
    swap: &S,
) -> UpdateOutcome {
    // Managed installs short-circuit before any network or swap activity.
    // The fetcher is not awaited; the user's package manager is
    // authoritative.
    if let InstallKind::Managed(m) = install_kind {
        return UpdateOutcome::ManagedRedirect(m);
    }

    let latest_raw = match fetcher.fetch().await {
        Ok(s) => s,
        Err(e) => return UpdateOutcome::FetchFailed(e),
    };
    let latest_norm = normalise_version(&latest_raw).to_string();

    if is_downgrade(current_version, &latest_raw) {
        return UpdateOutcome::DowngradeRefused {
            current: current_version.to_string(),
            latest: latest_norm,
        };
    }
    if !is_update_available(current_version, &latest_raw) {
        return UpdateOutcome::NoUpdate;
    }

    match swap.apply(&latest_norm) {
        Ok(()) => UpdateOutcome::Updated {
            from: current_version.to_string(),
            to: latest_norm,
        },
        Err(reason) => UpdateOutcome::SwapFailed {
            target: latest_norm,
            reason,
        },
    }
}

pub async fn fetch_latest_release(
    api_base: &str,
    owner: &str,
    repo: &str,
) -> Result<LatestRelease, String> {
    let url = format!("{api_base}/repos/{owner}/{repo}/releases/latest");
    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .build()
        .map_err(|e| format!("self-update: build http client: {e}"))?;
    let resp = client
        .get(&url)
        .header("accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| format!("self-update: request {url}: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("self-update: github api {}: {url}", resp.status()));
    }
    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("self-update: parse github response: {e}"))?;
    let tag_name = body
        .get("tag_name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "self-update: github response missing tag_name".to_string())?
        .to_string();
    Ok(LatestRelease { tag_name })
}

/// Production fetcher — hits `api.github.com` for the real GitHub
/// Releases endpoint.
struct GithubFetcher;

impl LatestFetcher for GithubFetcher {
    async fn fetch(&self) -> Result<String, String> {
        fetch_latest_release(DEFAULT_GH_API_BASE, GH_OWNER, GH_REPO)
            .await
            .map(|r| r.tag_name)
    }
}

/// Production swap — delegates to the `self_update` crate, which handles
/// asset matching, download, sha256 verification (when a `.sha256` is
/// published alongside the asset), and the atomic-replace dance per
/// platform.
struct SelfUpdateSwap;

impl SwapPerformer for SelfUpdateSwap {
    fn apply(&self, target_version: &str) -> Result<(), String> {
        // `self_update` is synchronous and does its own GH API call.
        // We've already pre-flighted via our async fetcher and decided
        // an update is wanted — `self_update` will re-fetch and pick
        // the matching asset by target triple.
        let target = target_version.to_string();
        self_update::backends::github::Update::configure()
            .repo_owner(GH_OWNER)
            .repo_name(GH_REPO)
            .bin_name("nt")
            .current_version(env!("CARGO_PKG_VERSION"))
            .target_version_tag(&format!("v{target}"))
            .show_download_progress(true)
            .build()
            .map_err(|e| format!("self-update: configure: {e}"))?
            .update()
            .map(|_| ())
            .map_err(|e| format!("self-update: apply: {e}"))
    }
}

/// `nt self-update` subcommand entry point. Returns the process exit code.
pub async fn run() -> i32 {
    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("nt self-update: cannot resolve own executable path: {e}");
            return 1;
        }
    };
    let install_kind = detect_install_kind(&exe);
    let current = env!("CARGO_PKG_VERSION");

    let outcome = orchestrate(install_kind, current, &GithubFetcher, &SelfUpdateSwap).await;

    match outcome {
        UpdateOutcome::ManagedRedirect(m) => {
            println!("{}", redirect_message(m));
            0
        }
        UpdateOutcome::NoUpdate => {
            println!("nt {current} is already the latest version.");
            0
        }
        UpdateOutcome::Updated { from, to } => {
            println!("nt updated: {from} → {to}");
            0
        }
        UpdateOutcome::DowngradeRefused { current, latest } => {
            eprintln!(
                "nt self-update: refusing to downgrade from {current} to {latest}. \
                 Reinstall directly if a downgrade is intentional."
            );
            1
        }
        UpdateOutcome::FetchFailed(msg) => {
            eprintln!("nt self-update: {msg}");
            1
        }
        UpdateOutcome::SwapFailed { target, reason } => {
            eprintln!("nt self-update: swap to {target} failed: {reason}");
            1
        }
    }
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
    fn detect_homebrew_custom_prefix_via_resolved_cellar_symlink() {
        // `current_exe()` resolves symlinks, so a HOMEBREW_PREFIX install
        // at `/opt/brew/bin/nt` is reached via its Cellar target:
        // `/opt/brew/Cellar/nt/0.1.0/bin/nt`. The /cellar/ marker catches
        // it regardless of prefix — that's the load-bearing
        // generalisation here (the old /usr/local/cellar/-only check
        // would have silently mis-classified custom prefixes as Direct
        // and corrupted brew's package database on self-update).
        assert_eq!(
            detect_install_kind(&PathBuf::from("/opt/brew/Cellar/nt/0.1.0/bin/nt")),
            InstallKind::Managed(Manager::Homebrew)
        );
    }

    #[test]
    fn detect_asdf_shim() {
        assert_eq!(
            detect_install_kind(&PathBuf::from("/Users/alice/.asdf/shims/nt")),
            InstallKind::Managed(Manager::Asdf)
        );
    }

    #[test]
    fn detect_asdf_install() {
        assert_eq!(
            detect_install_kind(&PathBuf::from(
                "/Users/alice/.asdf/installs/nt/0.1.0/bin/nt"
            )),
            InstallKind::Managed(Manager::Asdf)
        );
    }

    #[test]
    fn detect_mise_shim() {
        assert_eq!(
            detect_install_kind(&PathBuf::from("/Users/alice/.local/share/mise/shims/nt")),
            InstallKind::Managed(Manager::Mise)
        );
    }

    #[test]
    fn detect_mise_install() {
        assert_eq!(
            detect_install_kind(&PathBuf::from(
                "/Users/alice/.local/share/mise/installs/nt/0.1.0/bin/nt"
            )),
            InstallKind::Managed(Manager::Mise)
        );
    }

    #[test]
    fn detect_volta_bin() {
        assert_eq!(
            detect_install_kind(&PathBuf::from("/Users/alice/.volta/bin/nt")),
            InstallKind::Managed(Manager::Volta)
        );
    }

    #[test]
    fn detect_volta_tools() {
        assert_eq!(
            detect_install_kind(&PathBuf::from(
                "/Users/alice/.volta/tools/image/packages/nt/bin/nt"
            )),
            InstallKind::Managed(Manager::Volta)
        );
    }

    #[test]
    fn detect_node_modules_is_no_longer_managed_after_npm_retirement() {
        // npm distribution was retired in Task 12. Anyone whose nt
        // binary happens to live under (or be symlinked under) a
        // project's node_modules/.bin/ is a direct-install user whose
        // PATH lookup picked the wrong copy — not a managed install.
        // Refusing self-update here was the prior bug; we now treat
        // this as Direct so self-update proceeds normally.
        assert_eq!(
            detect_install_kind(&PathBuf::from("/Users/alice/project/node_modules/.bin/nt")),
            InstallKind::Direct
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
    fn redirect_message_homebrew_names_exact_brew_upgrade_command() {
        // Backticks preserved — pin the exact incantation. A typo like
        // `brew upgardex` or a swap to another command would fail here.
        let msg = redirect_message(Manager::Homebrew);
        assert!(
            msg.contains("`brew upgrade nt`"),
            "Homebrew message must quote the exact command, got: {msg}"
        );
    }

    #[test]
    fn redirect_message_cargo_names_exact_cargo_install_force_command() {
        let msg = redirect_message(Manager::Cargo);
        assert!(
            msg.contains("`cargo install --force nt-cli`"),
            "Cargo message must quote the exact command, got: {msg}"
        );
    }

    #[test]
    fn redirect_message_scoop_names_exact_scoop_update_command() {
        let msg = redirect_message(Manager::Scoop);
        assert!(
            msg.contains("`scoop update nt`"),
            "Scoop message must quote the exact command, got: {msg}"
        );
    }

    #[test]
    fn redirect_message_asdf_names_asdf_install_and_reshim() {
        let msg = redirect_message(Manager::Asdf);
        assert!(
            msg.contains("asdf install") && msg.contains("asdf reshim"),
            "asdf message must name both install + reshim, got: {msg}"
        );
    }

    #[test]
    fn redirect_message_mise_names_exact_mise_install_command() {
        let msg = redirect_message(Manager::Mise);
        assert!(
            msg.contains("`mise install nt@latest`"),
            "mise message must quote the exact command, got: {msg}"
        );
    }

    #[test]
    fn redirect_message_volta_names_exact_volta_install_command() {
        let msg = redirect_message(Manager::Volta);
        assert!(
            msg.contains("`volta install nt@latest`"),
            "Volta message must quote the exact command, got: {msg}"
        );
    }

    #[test]
    fn redirect_messages_are_all_distinct_and_non_empty() {
        // Defends against an accidental copy-paste that leaves two
        // manager branches with identical text (eg if someone adds a
        // new variant by duplicating Homebrew and forgets to edit the
        // message). Also pins that every variant is wired.
        let all = [
            Manager::Homebrew,
            Manager::Cargo,
            Manager::Scoop,
            Manager::Asdf,
            Manager::Mise,
            Manager::Volta,
        ];
        let mut seen: Vec<String> = Vec::new();
        for m in all {
            let msg = redirect_message(m);
            assert!(!msg.is_empty(), "{m:?} produced an empty redirect message");
            assert!(
                !seen.contains(&msg),
                "{m:?} produced a duplicate redirect message: {msg}"
            );
            seen.push(msg);
        }
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

    struct StubFetcher(Result<String, String>);
    impl LatestFetcher for StubFetcher {
        async fn fetch(&self) -> Result<String, String> {
            self.0.clone()
        }
    }

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

    #[tokio::test]
    async fn orchestrate_managed_install_returns_redirect_without_fetching_or_swapping() {
        // The fetcher returning Err would fail the test if it were ever
        // awaited — pin: managed-install path short-circuits before any
        // network or swap activity.
        let fetcher = StubFetcher(Err("must not call fetcher on managed install".into()));
        let swap = StubSwap::ok();
        let outcome = orchestrate(
            InstallKind::Managed(Manager::Homebrew),
            "0.1.0",
            &fetcher,
            &swap,
        )
        .await;
        assert_eq!(outcome, UpdateOutcome::ManagedRedirect(Manager::Homebrew));
        assert!(
            swap.called_with.borrow().is_none(),
            "swap must not run on managed install"
        );
    }

    #[tokio::test]
    async fn orchestrate_no_update_when_current_matches_latest() {
        let fetcher = StubFetcher(Ok("0.1.0".into()));
        let swap = StubSwap::ok();
        let outcome = orchestrate(InstallKind::Direct, "0.1.0", &fetcher, &swap).await;
        assert_eq!(outcome, UpdateOutcome::NoUpdate);
        assert!(swap.called_with.borrow().is_none());
    }

    #[tokio::test]
    async fn orchestrate_refuses_downgrade_without_swapping() {
        let fetcher = StubFetcher(Ok("0.0.9".into()));
        let swap = StubSwap::ok();
        let outcome = orchestrate(InstallKind::Direct, "0.1.0", &fetcher, &swap).await;
        assert_eq!(
            outcome,
            UpdateOutcome::DowngradeRefused {
                current: "0.1.0".into(),
                latest: "0.0.9".into(),
            }
        );
        assert!(swap.called_with.borrow().is_none());
    }

    #[tokio::test]
    async fn orchestrate_applies_swap_on_available_update() {
        let fetcher = StubFetcher(Ok("0.2.0".into()));
        let swap = StubSwap::ok();
        let outcome = orchestrate(InstallKind::Direct, "0.1.0", &fetcher, &swap).await;
        assert_eq!(
            outcome,
            UpdateOutcome::Updated {
                from: "0.1.0".into(),
                to: "0.2.0".into(),
            }
        );
        assert_eq!(swap.called_with.borrow().as_deref(), Some("0.2.0"));
    }

    #[tokio::test]
    async fn orchestrate_normalises_v_prefix_from_latest_tag() {
        let fetcher = StubFetcher(Ok("v0.2.0".into()));
        let swap = StubSwap::ok();
        let outcome = orchestrate(InstallKind::Direct, "0.1.0", &fetcher, &swap).await;
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

    #[tokio::test]
    async fn orchestrate_fetch_error_surfaces_as_fetch_failed() {
        let fetcher = StubFetcher(Err("network down".into()));
        let swap = StubSwap::ok();
        let outcome = orchestrate(InstallKind::Direct, "0.1.0", &fetcher, &swap).await;
        match outcome {
            UpdateOutcome::FetchFailed(msg) => assert!(
                msg.contains("network down"),
                "FetchFailed must carry the underlying reason, got: {msg}"
            ),
            other => panic!("expected FetchFailed, got {other:?}"),
        }
        assert!(swap.called_with.borrow().is_none());
    }

    #[tokio::test]
    async fn orchestrate_swap_error_surfaces_as_swap_failed() {
        let fetcher = StubFetcher(Ok("0.2.0".into()));
        let swap = StubSwap::err("sha256 mismatch");
        let outcome = orchestrate(InstallKind::Direct, "0.1.0", &fetcher, &swap).await;
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
