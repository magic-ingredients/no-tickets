//! Fetch + sha256-verify + decompress the JSON Schema release bundle
//! from no-tickets-service GH Releases. Output goes to `$OUT_DIR` and
//! is `include_str!`'d into `lib.rs`.
//!
//! Bumping `SCHEMAS_VERSION` is the one-line change that tracks a new
//! schemas-v* release. The pinned hash is verified against the
//! sha256 sidecar published alongside the asset, so a retag-in-place
//! upstream fails the build cleanly.
//!
//! Auth: downloads via `gh release download`. The no-tickets-service
//! repo is currently private — gh's local credentials cover that
//! without requiring contributors to set `GITHUB_TOKEN` by hand.
//! When the repo goes public this can become a plain HTTPS GET against
//! the same URL pattern with no auth.

use std::env;
use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::process::Command;

use flate2::read::GzDecoder;
use sha2::{Digest, Sha256};

/// Pinned schemas-package version. Bumping this is the only change
/// needed to track a new no-tickets-service schemas release.
///
/// v0.2.2 adds `metadataSchema` as a top-level bundle entry (the
/// envelope-level `{ actor }` schema for opt-in actor attribution).
const SCHEMAS_VERSION: &str = "0.2.2";

const REPO: &str = "magic-ingredients/no-tickets-service";

fn main() {
    // Re-fetch only when this build script (and therefore the pin)
    // changes. Cargo handles Cargo.toml changes automatically.
    println!("cargo:rerun-if-changed=build.rs");

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR set by cargo"));

    let tag = format!("schemas-v{SCHEMAS_VERSION}");
    let gz_name = format!("schemas-v{SCHEMAS_VERSION}.json.gz");
    let sha_name = format!("{gz_name}.sha256");

    let gz_path = out_dir.join(&gz_name);
    let sha_path = out_dir.join(&sha_name);

    let status = Command::new("gh")
        .args([
            "release",
            "download",
            &tag,
            "-R",
            REPO,
            "-p",
            &gz_name,
            "-p",
            &sha_name,
            "-D",
            out_dir.to_str().expect("OUT_DIR path is valid UTF-8"),
            "--clobber",
        ])
        .status()
        .expect(
            "invoke `gh release download` — install gh CLI (https://cli.github.com) \
             and run `gh auth login` to access private release assets",
        );
    if !status.success() {
        panic!(
            "gh release download {tag} failed (exit {:?}). \
             Verify `gh auth status` and that {tag} exists at github.com/{REPO}/releases.",
            status.code(),
        );
    }

    // sha256 sidecar shape: "<hex>  <filename>\n" (shasum -a 256 -c format).
    let sha_file = fs::read_to_string(&sha_path)
        .unwrap_or_else(|e| panic!("read sha256 sidecar at {sha_path:?}: {e}"));
    let expected_hex = sha_file
        .split_ascii_whitespace()
        .next()
        .expect("sha256 sidecar has a leading hex digest")
        .to_ascii_lowercase();

    let gz_bytes =
        fs::read(&gz_path).unwrap_or_else(|e| panic!("read downloaded {gz_path:?}: {e}"));
    let mut hasher = Sha256::new();
    hasher.update(&gz_bytes);
    let actual_hex: String = hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    if actual_hex != expected_hex {
        panic!(
            "sha256 mismatch on {gz_name}: expected {expected_hex}, got {actual_hex}. \
             The release asset may have been retagged in place or the download was corrupted.",
        );
    }

    let mut decoder = GzDecoder::new(gz_bytes.as_slice());
    let mut bundle_json = String::new();
    decoder
        .read_to_string(&mut bundle_json)
        .expect("gunzip release bundle");
    let bundle_out = out_dir.join("event-types.bundle.json");
    fs::write(&bundle_out, &bundle_json)
        .unwrap_or_else(|e| panic!("write bundle JSON to {bundle_out:?}: {e}"));

    // Surface the pinned version to library code and tests.
    println!("cargo:rustc-env=NT_SCHEMAS_VERSION={SCHEMAS_VERSION}");
}
