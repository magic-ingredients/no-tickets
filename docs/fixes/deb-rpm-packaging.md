---
id: deb-rpm-packaging
title: "deb / rpm packaging for Linux server installs"
status: not_started
severity: minor
reported: 2026-05-20T00:00:00.000Z
resolved: null
---

# Fix: deb / rpm packaging for Linux server installs

Extracted from `cross-platform-cli-binary` Task 9 (superseded there in
favour of standalone tracking) so the work has its own scope, history,
and review surface.

## Context

`no-tickets` ships via `cargo-dist`'s installer set — shell, PowerShell,
Homebrew formula, `cargo install`. Linux server admins running Debian /
Ubuntu / RHEL / Fedora / Amazon Linux expect to install via `apt` or
`yum`/`dnf` against a managed repo, not via `curl … | sh`. Today, the
only Linux path is the shell installer (which works fine but isn't the
default for production-server provisioning toolchains).

This fix adds `deb` and `rpm` repositories so:

```bash
# Debian / Ubuntu
curl -fsSL https://apt.no-tickets.com/setup | sudo bash
sudo apt install no-tickets

# RHEL / Fedora / Amazon Linux
sudo dnf config-manager --add-repo https://yum.no-tickets.com/repo
sudo dnf install no-tickets
```

Out of `cargo-dist` 0.31.0's installer set — hand-rolled GitHub Actions
job that builds `.deb` / `.rpm` artifacts from each release and publishes
them to a static-hosted repo (GitHub Pages or a CDN).

## Demand signal — not yet present

Deferring real implementation until at least one user / org requests it.
The shell installer covers the same Linux servers today; package-manager
installs are nicer-to-have but don't unlock new audiences. Keep this fix
open for tracking and as the landing page when demand materialises.

## Tasks

### 1. Build `.deb` artifacts in the release pipeline
status: not_started

Add a `.github/workflows/build-debs.yml` (or extend `release.yml`) that
runs after `host` succeeds and produces `.deb` files for x86_64 and
aarch64 Linux targets.

Tooling options:
- `cargo-deb` — Cargo plugin, reads `[package.metadata.deb]` from
  Cargo.toml. Simplest if it covers our needs.
- `nfpm` — Go-based, generates both deb and rpm from one TOML/YAML
  config. Better if we want a single source of truth across formats.

Recommend `nfpm` so step 2 (rpm) is the same workflow with a different
output target.

### 2. Build `.rpm` artifacts in the release pipeline
status: not_started

If `nfpm` is the chosen tool from Task 1, this is a config-only addition:
add the rpm output target to the nfpm spec. If `cargo-deb` was used,
add `cargo-generate-rpm` as a separate step.

### 3. Host the apt + dnf repositories
status: not_started

Two options:

- **GitHub Pages** — static-host the repo metadata at `apt.no-tickets.com`
  and `yum.no-tickets.com` (CNAMEs to GitHub Pages). Cheapest; works for
  small-scale traffic. Tooling: `aptly` / `createrepo_c` to generate
  index files in CI.
- **Cloudflare R2 + Worker** — same pattern as `get.no-tickets.com`.
  Better if the repos get heavy traffic.

GitHub Pages is the day-one pick (matches the rest of our zero-
infrastructure ethos). Migrate to R2 if scale calls for it.

### 4. Document the install commands in docs/install.md
status: not_started

Once 1-3 ship, add a "Debian / Ubuntu" and "RHEL / Fedora" section to
`docs/install.md` next to Homebrew / PowerShell.

### 5. Signing strategy decision
status: not_started

Both apt and dnf support repo signing (GPG keys for apt, RPM-GPG for
dnf). Unsigned repos work but trigger warnings on modern distros; signed
repos require a managed key pair.

Decision: sign with a project GPG key, stored as a CI secret. Generate
once, document the fingerprint in `docs/install.md` so users can verify.
Defer key rotation policy until a real reason to rotate appears.

## Acceptance Criteria

- `apt install no-tickets` works on Debian 12, Ubuntu 24.04 LTS
- `dnf install no-tickets` works on Fedora 40, RHEL 9, Amazon Linux 2023
- Both repos signed and the public key documented
- A release tag automatically pushes new `.deb` / `.rpm` artifacts to
  the hosted repos
- `docs/install.md` documents the install commands and verification path
