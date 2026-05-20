//! `nt update` — direct-download binary upgrade path.
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
use std::time::Duration;

use semver::Version;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(crate) enum InstallKind {
    Direct,
    Managed(Manager),
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(crate) enum Manager {
    Homebrew,
    Cargo,
    Scoop,
    Asdf,
    Mise,
    Volta,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum UpdateOutcome {
    NoUpdate,
    Updated {
        from: String,
        to: String,
    },
    ManagedRedirect(Manager),
    DowngradeRefused {
        current: String,
        latest: String,
    },
    /// The GitHub tag couldn't be parsed as semver (e.g. a release pipeline
    /// switched to calver tags like `2026-05-15`). Surfacing this
    /// explicitly avoids the silent "no update available" footgun where
    /// users on every prior version see `already the latest` forever.
    VersionParseError {
        latest: String,
    },
    FetchFailed(String),
    SwapFailed {
        target: String,
        reason: String,
    },
}

/// Structured failure modes for the GitHub Releases pre-flight. Mapped
/// to a user-facing string at the `run()` boundary; in-module callers
/// can match on the variant if they need to discriminate (e.g. the
/// rate-limit case from a transport error).
#[derive(Debug)]
pub(crate) enum FetchError {
    /// Generic non-2xx other than rate-limit.
    HttpStatus(reqwest::StatusCode),
    /// GitHub returned 403 with `X-RateLimit-Remaining: 0`. The optional
    /// reset epoch (`X-RateLimit-Reset`) is included when present so
    /// the user-facing message can name a wait time.
    RateLimited { reset_epoch: Option<u64> },
    /// Connection / DNS / timeout / TLS handshake failure.
    Network(String),
    /// Body was not valid JSON.
    ParseJson(String),
    /// JSON parsed but `tag_name` was missing, non-string, or empty.
    /// Empty-string tag_name is folded here rather than passed through
    /// — a release with no tag is observably broken upstream and
    /// shouldn't be reported as "no update available".
    InvalidTagName,
}

impl FetchError {
    fn user_message(&self) -> String {
        match self {
            Self::HttpStatus(s) => format!("github api returned {s}"),
            Self::RateLimited { reset_epoch } => match reset_epoch {
                Some(t) => format!(
                    "github api rate-limit hit (resets at unix epoch {t}). \
                     Set GITHUB_TOKEN or wait until the window resets, \
                     then re-run `no-tickets update`."
                ),
                None => "github api rate-limit hit. \
                     Set GITHUB_TOKEN or wait a few minutes, \
                     then re-run `no-tickets update`."
                    .to_string(),
            },
            Self::Network(msg) => format!("network error: {msg}"),
            Self::ParseJson(msg) => format!("parse github response: {msg}"),
            Self::InvalidTagName => {
                "github response had a missing, non-string, or empty tag_name".to_string()
            }
        }
    }
}

pub(crate) trait LatestFetcher {
    async fn fetch(&self) -> Result<String, String>;
}

pub(crate) trait SwapPerformer {
    fn apply(&self, target_version: &str) -> Result<(), String>;
}

/// GitHub owner/repo coordinates the production fetcher and the swap
/// backend point at. Kept here so the magic strings live in one place.
pub(crate) const GH_OWNER: &str = "magic-ingredients";
pub(crate) const GH_REPO: &str = "no-tickets";
pub(crate) const DEFAULT_GH_API_BASE: &str = "https://api.github.com";
const USER_AGENT: &str = "no-tickets-update";
/// Connect-phase timeout: TLS handshake + initial response. Captive
/// portals and dead routes typically resolve within this window.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// End-to-end timeout for the whole request. The GH `/releases/latest`
/// endpoint typically responds in <300ms; 30s leaves plenty of headroom
/// without letting a stuck connection hang `nt update` forever.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

pub(crate) fn detect_install_kind(exe_path: &Path) -> InstallKind {
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

pub(crate) fn redirect_message(manager: Manager) -> String {
    match manager {
        Manager::Homebrew => {
            "no-tickets was installed via Homebrew. Run `brew upgrade no-tickets` to update."
                .to_string()
        }
        Manager::Cargo => "no-tickets was installed via Cargo. \
             Run `cargo install --force no-tickets` to update."
            .to_string(),
        Manager::Scoop => {
            "no-tickets was installed via Scoop. Run `scoop update no-tickets` to update."
                .to_string()
        }
        Manager::Asdf => "no-tickets was installed via asdf. \
             Update via your asdf plugin (e.g. `asdf install no-tickets latest && asdf reshim`)."
            .to_string(),
        Manager::Mise => "no-tickets was installed via mise. \
             Update via `mise install no-tickets@latest` (or your mise tools manifest)."
            .to_string(),
        Manager::Volta => "no-tickets was installed via Volta. \
             Update via `volta install no-tickets@latest`."
            .to_string(),
    }
}

/// Strip a single leading `v` so GitHub tag-style strings parse as
/// semver, and drop any `+build.metadata` suffix.
///
/// Build metadata is dropped so comparisons follow the SemVer 2.0
/// spec ("Build metadata MUST be ignored when determining version
/// precedence"). The `semver` crate v1.x deviates from the spec and
/// orders `0.2.0+sha.abc` *above* `0.2.0`; stripping the suffix here
/// gets us spec-aligned semantics for free.
fn normalise_version(v: &str) -> &str {
    let no_v = v.strip_prefix('v').unwrap_or(v);
    match no_v.find('+') {
        Some(idx) => &no_v[..idx],
        None => no_v,
    }
}

/// Canonical GitHub release-tag form for a normalised version. Single
/// source of truth so the fetcher (which strips `v`) and the swap
/// (which re-adds `v` when telling `self_update` which tag to grab)
/// can't drift apart. If the release scheme ever changes
/// (`release-X.Y.Z`, etc.) this is the one line that moves.
pub(crate) fn to_release_tag(version: &str) -> String {
    format!("v{}", normalise_version(version))
}

pub(crate) fn version_parseable(v: &str) -> bool {
    Version::parse(normalise_version(v)).is_ok()
}

pub(crate) fn is_downgrade(current: &str, latest: &str) -> bool {
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

pub(crate) fn is_update_available(current: &str, latest: &str) -> bool {
    let (Ok(c), Ok(l)) = (
        Version::parse(normalise_version(current)),
        Version::parse(normalise_version(latest)),
    ) else {
        return false;
    };
    l > c
}

pub(crate) async fn orchestrate<F: LatestFetcher, S: SwapPerformer>(
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

    // If the latest tag isn't parseable as semver (e.g. release pipeline
    // switched to calver, or returned a malformed tag), surface that
    // explicitly. Falling through to `is_update_available` would silently
    // return NoUpdate forever — a real foot-gun if a release engineer
    // changes tag style without realising old binaries can't read it.
    if !version_parseable(&latest_raw) {
        return UpdateOutcome::VersionParseError { latest: latest_raw };
    }

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

/// Fetch the `tag_name` of the most recent **stable** release from
/// GitHub Releases. Drafts and prereleases are silently skipped — that
/// is GitHub's contract for the `/releases/latest` endpoint, not ours.
/// If the team ships an rc as the most recent activity, callers on the
/// prior stable correctly see "no update available" because the most
/// recent *stable* hasn't moved.
///
/// Returns the raw tag string (e.g. `"v0.1.5"`); callers normalise.
pub(crate) async fn fetch_latest_release(
    api_base: &str,
    owner: &str,
    repo: &str,
) -> Result<String, FetchError> {
    let url = format!("{api_base}/repos/{owner}/{repo}/releases/latest");
    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .build()
        .map_err(|e| FetchError::Network(format!("build http client: {e}")))?;
    let resp = client
        .get(&url)
        .header("accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| FetchError::Network(format!("{e}")))?;

    let status = resp.status();
    if !status.is_success() {
        // 403 + `X-RateLimit-Remaining: 0` is GitHub's documented
        // rate-limit signal. Surface it as its own variant so the user
        // sees an actionable "set GITHUB_TOKEN or wait" message rather
        // than the misleading generic "403 Forbidden" (which sounds
        // like an auth problem).
        if status == reqwest::StatusCode::FORBIDDEN
            && resp
                .headers()
                .get("x-ratelimit-remaining")
                .and_then(|v| v.to_str().ok())
                == Some("0")
        {
            let reset_epoch = resp
                .headers()
                .get("x-ratelimit-reset")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok());
            return Err(FetchError::RateLimited { reset_epoch });
        }
        return Err(FetchError::HttpStatus(status));
    }

    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| FetchError::ParseJson(format!("{e}")))?;
    let tag_name = body
        .get("tag_name")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or(FetchError::InvalidTagName)?
        .to_string();
    Ok(tag_name)
}

/// Production fetcher — hits `api.github.com` for the real GitHub
/// Releases endpoint.
struct GithubFetcher;

impl LatestFetcher for GithubFetcher {
    async fn fetch(&self) -> Result<String, String> {
        fetch_latest_release(DEFAULT_GH_API_BASE, GH_OWNER, GH_REPO)
            .await
            .map_err(|e| e.user_message())
    }
}

/// Production swap — delegates to the `self_update` crate.
///
/// **Note on integrity verification:** the `self_update` crate verifies
/// sha256 only when a `<asset>.sha256` companion file is published
/// alongside the binary in the GitHub release. That's a release-pipeline
/// contract (owned by Task 6 / cargo-dist config), not a runtime
/// guarantee enforced here — if a future release ships without the
/// sha256 file, this code will silently fall back to unverified download.
/// A separate task should add a CI check that fails the release workflow
/// when the sha256 companion is missing.
struct UpdateSwap;

impl SwapPerformer for UpdateSwap {
    fn apply(&self, target_version: &str) -> Result<(), String> {
        // `self_update` is synchronous and does its own GH API call.
        // We've already pre-flighted via our async fetcher and decided
        // an update is wanted — `self_update` will re-fetch and pick
        // the matching asset by target triple. `target_version_tag`
        // pins which tag it grabs so the pre-flight and the swap can't
        // disagree (TOCTOU between our fetch and the crate's fetch).
        self_update::backends::github::Update::configure()
            .repo_owner(GH_OWNER)
            .repo_name(GH_REPO)
            .bin_name("no-tickets")
            .current_version(env!("CARGO_PKG_VERSION"))
            .target_version_tag(&to_release_tag(target_version))
            .show_download_progress(true)
            .build()
            .map_err(|e| format!("configure: {e}"))?
            .update()
            .map(|_| ())
            .map_err(|e| format!("apply: {e}"))
    }
}

/// Pure mapping from outcome → exit code. Extracted so the table is
/// testable without touching `current_exe()` / network / swap.
pub(crate) fn outcome_to_exit_code(outcome: &UpdateOutcome) -> i32 {
    match outcome {
        UpdateOutcome::ManagedRedirect(_)
        | UpdateOutcome::NoUpdate
        | UpdateOutcome::Updated { .. } => 0,
        UpdateOutcome::DowngradeRefused { .. }
        | UpdateOutcome::VersionParseError { .. }
        | UpdateOutcome::FetchFailed(_)
        | UpdateOutcome::SwapFailed { .. } => 1,
    }
}

/// `nt update` subcommand entry point. Returns the process exit code.
pub async fn run() -> i32 {
    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("no-tickets update: cannot resolve own executable path: {e}");
            return 1;
        }
    };
    let install_kind = detect_install_kind(&exe);
    let current = env!("CARGO_PKG_VERSION");

    let outcome = orchestrate(install_kind, current, &GithubFetcher, &UpdateSwap).await;
    match &outcome {
        UpdateOutcome::ManagedRedirect(m) => println!("{}", redirect_message(*m)),
        UpdateOutcome::NoUpdate => {
            println!("no-tickets {current} is already the latest version.")
        }
        UpdateOutcome::Updated { from, to } => println!("no-tickets updated: {from} → {to}"),
        UpdateOutcome::DowngradeRefused { current, latest } => eprintln!(
            "no-tickets update: refusing to downgrade from {current} to {latest}. \
             Reinstall directly if a downgrade is intentional."
        ),
        UpdateOutcome::VersionParseError { latest } => eprintln!(
            "no-tickets update: latest release tag {latest:?} is not parseable as semver. \
             The release pipeline may have switched tag style; please report this."
        ),
        UpdateOutcome::FetchFailed(msg) => eprintln!("no-tickets update: {msg}"),
        UpdateOutcome::SwapFailed { target, reason } => {
            eprintln!("no-tickets update: swap to {target} failed: {reason}")
        }
    }
    outcome_to_exit_code(&outcome)
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
        // and corrupted brew's package database on update).
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
        // Refusing the update here was the prior bug; we now treat
        // this as Direct so update proceeds normally.
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
            msg.contains("`brew upgrade no-tickets`"),
            "Homebrew message must quote the exact command, got: {msg}"
        );
    }

    #[test]
    fn redirect_message_cargo_names_exact_cargo_install_force_command() {
        let msg = redirect_message(Manager::Cargo);
        assert!(
            msg.contains("`cargo install --force no-tickets`"),
            "Cargo message must quote the exact command, got: {msg}"
        );
    }

    #[test]
    fn redirect_message_scoop_names_exact_scoop_update_command() {
        let msg = redirect_message(Manager::Scoop);
        assert!(
            msg.contains("`scoop update no-tickets`"),
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
            msg.contains("`mise install no-tickets@latest`"),
            "mise message must quote the exact command, got: {msg}"
        );
    }

    #[test]
    fn redirect_message_volta_names_exact_volta_install_command() {
        let msg = redirect_message(Manager::Volta);
        assert!(
            msg.contains("`volta install no-tickets@latest`"),
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

        let tag = fetch_latest_release(&server.uri(), "magic-ingredients", "no-tickets")
            .await
            .expect("fetch should succeed");
        assert_eq!(tag, "v0.1.5");
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
            .and(header("user-agent", "no-tickets-update"))
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
        assert!(
            matches!(result, Err(FetchError::InvalidTagName)),
            "missing tag_name must surface as FetchError::InvalidTagName, got: {result:?}"
        );
    }

    #[tokio::test]
    async fn fetch_latest_release_returns_invalid_tag_name_on_empty_string() {
        // Empty-string tag_name was the sharpest mutation surface flagged
        // by the adversarial review: parses out of JSON fine, normalises
        // to empty, fails semver parse, orchestrate would silently
        // report NoUpdate. Pin that we reject it upstream as
        // InvalidTagName.
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "tag_name": ""
            })))
            .mount(&server)
            .await;

        let result = fetch_latest_release(&server.uri(), "foo", "bar").await;
        assert!(
            matches!(result, Err(FetchError::InvalidTagName)),
            "empty tag_name must surface as InvalidTagName, got: {result:?}"
        );
    }

    #[tokio::test]
    async fn fetch_latest_release_returns_invalid_tag_name_on_non_string() {
        // `tag_name` typed as null / number — should be rejected via the
        // same path as a missing field, not silently coerced.
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "tag_name": 42
            })))
            .mount(&server)
            .await;

        let result = fetch_latest_release(&server.uri(), "foo", "bar").await;
        assert!(
            matches!(result, Err(FetchError::InvalidTagName)),
            "non-string tag_name must surface as InvalidTagName, got: {result:?}"
        );
    }

    #[tokio::test]
    async fn fetch_latest_release_returns_parse_json_on_empty_body() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_string(""))
            .mount(&server)
            .await;

        let result = fetch_latest_release(&server.uri(), "foo", "bar").await;
        assert!(
            matches!(result, Err(FetchError::ParseJson(_))),
            "empty body must surface as ParseJson, got: {result:?}"
        );
    }

    #[tokio::test]
    async fn fetch_latest_release_returns_rate_limited_on_403_with_remaining_zero() {
        // GitHub's documented rate-limit signal: 403 with
        // `X-RateLimit-Remaining: 0`. Surface as a distinct variant so
        // the user-visible message can name a wait time and suggest
        // GITHUB_TOKEN, rather than the misleading generic 403.
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(
                ResponseTemplate::new(403)
                    .insert_header("x-ratelimit-remaining", "0")
                    .insert_header("x-ratelimit-reset", "1234567890"),
            )
            .mount(&server)
            .await;

        let result = fetch_latest_release(&server.uri(), "foo", "bar").await;
        match result {
            Err(FetchError::RateLimited { reset_epoch }) => {
                assert_eq!(reset_epoch, Some(1234567890));
            }
            other => panic!("expected RateLimited, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn fetch_latest_release_falls_back_to_http_status_on_403_without_remaining_header() {
        // A 403 without the rate-limit-remaining header (e.g. real
        // auth failure) must NOT be mis-classified as rate-limited.
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(403))
            .mount(&server)
            .await;

        let result = fetch_latest_release(&server.uri(), "foo", "bar").await;
        assert!(
            matches!(result, Err(FetchError::HttpStatus(s)) if s.as_u16() == 403),
            "403 without rate-limit header must stay HttpStatus, got: {result:?}"
        );
    }

    // ---- FetchError::user_message --------------------------------------

    #[test]
    fn fetch_error_user_message_rate_limited_with_reset_names_epoch_and_token() {
        let msg = FetchError::RateLimited {
            reset_epoch: Some(1234567890),
        }
        .user_message();
        assert!(msg.contains("1234567890"), "got: {msg}");
        assert!(msg.contains("GITHUB_TOKEN"), "got: {msg}");
        assert!(msg.contains("rate-limit"), "got: {msg}");
    }

    #[test]
    fn fetch_error_user_message_rate_limited_without_reset_still_actionable() {
        let msg = FetchError::RateLimited { reset_epoch: None }.user_message();
        assert!(msg.contains("GITHUB_TOKEN"), "got: {msg}");
        assert!(msg.contains("rate-limit"), "got: {msg}");
    }

    #[test]
    fn fetch_error_user_message_http_status_names_status_code() {
        let msg = FetchError::HttpStatus(reqwest::StatusCode::SERVICE_UNAVAILABLE).user_message();
        assert!(msg.contains("503"), "got: {msg}");
    }

    #[test]
    fn fetch_error_user_message_invalid_tag_name_is_descriptive() {
        let msg = FetchError::InvalidTagName.user_message();
        assert!(msg.to_lowercase().contains("tag_name"), "got: {msg}");
    }

    // ---- to_release_tag -------------------------------------------------

    #[test]
    fn to_release_tag_adds_v_prefix_when_missing() {
        assert_eq!(to_release_tag("0.1.0"), "v0.1.0");
    }

    #[test]
    fn to_release_tag_keeps_single_v_prefix_when_present() {
        // Normalises first, so a `v`-prefixed input doesn't end up `vv…`.
        assert_eq!(to_release_tag("v0.1.0"), "v0.1.0");
    }

    // ---- orchestrate: VersionParseError --------------------------------

    #[tokio::test]
    async fn orchestrate_surfaces_version_parse_error_for_unparseable_latest_tag() {
        // Calver-style tag from a release pipeline change — pin that we
        // surface this explicitly instead of falling through to a silent
        // NoUpdate (which would tell every old binary it's "already the
        // latest version" forever).
        let fetcher = StubFetcher(Ok("2026-05-15".into()));
        let swap = StubSwap::ok();
        let outcome = orchestrate(InstallKind::Direct, "0.1.0", &fetcher, &swap).await;
        assert_eq!(
            outcome,
            UpdateOutcome::VersionParseError {
                latest: "2026-05-15".into()
            }
        );
        assert!(
            swap.called_with.borrow().is_none(),
            "swap must not run when latest tag is unparseable"
        );
    }

    // ---- version comparison: prerelease, build metadata, calver --------

    #[test]
    fn update_available_treats_prerelease_as_below_release_of_same_version() {
        // semver spec: 0.2.0-rc.1 < 0.2.0. So upgrading from rc.1 to
        // the stable release is a valid update.
        assert!(is_update_available("0.2.0-rc.1", "0.2.0"));
    }

    #[test]
    fn update_not_available_when_latest_is_prerelease_of_current() {
        // ... and an rc of the *current* release isn't an update.
        assert!(!is_update_available("0.2.0", "0.2.0-rc.2"));
    }

    #[test]
    fn update_available_ignores_build_metadata() {
        // Build metadata (`+sha.abc`) doesn't participate in precedence
        // per the semver spec. `0.2.0+sha.abc` is equivalent to `0.2.0`.
        assert!(!is_update_available("0.2.0", "0.2.0+sha.abc"));
    }

    #[test]
    fn version_parseable_accepts_standard_semver() {
        assert!(version_parseable("0.1.0"));
        assert!(version_parseable("v0.1.0"));
        assert!(version_parseable("0.1.0-rc.1"));
        assert!(version_parseable("0.1.0+sha.abc"));
    }

    #[test]
    fn version_parseable_rejects_calver_and_garbage() {
        // Calver-style YYYY-MM-DD doesn't parse as semver because the
        // segments aren't numeric "major.minor.patch" with a strict
        // semver shape. Pin: this is the trigger for VersionParseError.
        assert!(!version_parseable("2026-05-15"));
        assert!(!version_parseable("release-0.1.0"));
        assert!(!version_parseable(""));
        assert!(!version_parseable("v"));
    }

    // ---- outcome_to_exit_code (run() table extracted) ------------------

    #[test]
    fn exit_code_zero_for_managed_redirect() {
        assert_eq!(
            outcome_to_exit_code(&UpdateOutcome::ManagedRedirect(Manager::Homebrew)),
            0
        );
    }

    #[test]
    fn exit_code_zero_for_no_update() {
        assert_eq!(outcome_to_exit_code(&UpdateOutcome::NoUpdate), 0);
    }

    #[test]
    fn exit_code_zero_for_updated() {
        assert_eq!(
            outcome_to_exit_code(&UpdateOutcome::Updated {
                from: "0.1.0".into(),
                to: "0.2.0".into(),
            }),
            0
        );
    }

    #[test]
    fn exit_code_one_for_downgrade_refused() {
        assert_eq!(
            outcome_to_exit_code(&UpdateOutcome::DowngradeRefused {
                current: "0.2.0".into(),
                latest: "0.1.0".into(),
            }),
            1
        );
    }

    #[test]
    fn exit_code_one_for_version_parse_error() {
        assert_eq!(
            outcome_to_exit_code(&UpdateOutcome::VersionParseError {
                latest: "2026-05-15".into()
            }),
            1
        );
    }

    #[test]
    fn exit_code_one_for_fetch_failed() {
        assert_eq!(
            outcome_to_exit_code(&UpdateOutcome::FetchFailed("network down".into())),
            1
        );
    }

    #[test]
    fn exit_code_one_for_swap_failed() {
        assert_eq!(
            outcome_to_exit_code(&UpdateOutcome::SwapFailed {
                target: "0.2.0".into(),
                reason: "sha256 mismatch".into(),
            }),
            1
        );
    }

    // ---- linuxbrew mutation-gap fix ------------------------------------

    #[test]
    fn detect_homebrew_per_user_linuxbrew_prefix() {
        // The pre-existing `/home/linuxbrew/.linuxbrew/` test path
        // matches BOTH the `/.linuxbrew/` and `/linuxbrew/.linuxbrew/`
        // OR-arms, so flipping `||` to `&&` between them survived
        // mutation testing. A per-user install at `$HOME/.linuxbrew/`
        // (no shared linuxbrew system user) exercises ONLY the
        // `/.linuxbrew/` arm and kills that mutant.
        assert_eq!(
            detect_install_kind(&PathBuf::from("/home/alice/.linuxbrew/bin/nt")),
            InstallKind::Managed(Manager::Homebrew)
        );
    }
}
