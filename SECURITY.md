# Security Policy

## Reporting a vulnerability

If you've found a security vulnerability in the no-tickets binaries
(`nt`, `nt-mcp`), any package we publish (`@magic-ingredients/no-tickets`,
the `no-tickets` crate on crates.io), or the distribution path itself
(install.sh, our homebrew tap, our scoop bucket), please **report it
privately** rather than opening a public issue or pull request.

The preferred channel is GitHub's private vulnerability reporting:
**[Open a private advisory](https://github.com/magic-ingredients/no-tickets/security/advisories/new)**

GitHub will notify the maintainers, and the advisory thread keeps the
report private until disclosure is coordinated.

### What helps us triage

- The affected component and its version (for the binaries:
  `nt --version` output; for npm: `pnpm list @magic-ingredients/no-tickets`;
  for cargo: the crate version)
- The OS and install channel (curl / brew / cargo / scoop / direct)
- Reproduction steps or a proof-of-concept
- Your assessment of impact — what an attacker can do, what they
  need to start

## Supported versions

no-tickets is pre-1.0. Only the **latest release** receives security
patches.

| Version | Supported |
|---------|-----------|
| Latest  | ✅        |
| Earlier | ❌        |

How users get patches depends on the install channel:

| Channel | Update mechanism |
|---------|------------------|
| `curl … \| sh` or direct download | `nt self-update` (manual; no auto-update) |
| Homebrew | `brew upgrade no-tickets` (manual; runs as part of `brew upgrade`) |
| `cargo install` | `cargo install --force no-tickets` (manual) |
| Scoop | `scoop update no-tickets` (manual) |
| npm | `npm update @magic-ingredients/no-tickets` (manual) |

None of the channels auto-update on their own. Advisories will spell
out the exact update command for each channel.

## Safe-harbor and good-faith research

We support good-faith security research:

- **No legal action** will be taken against researchers who comply
  with this policy and avoid privacy violations, service disruption,
  or destruction of data during testing.
- **Don't access user data** beyond what your own account can
  legitimately reach; don't pivot from a vulnerability you find into
  systems beyond the bug's scope.
- **Coordinate before public disclosure.** If you want to publish a
  write-up, please wait until a patched release is out and an advisory
  has been published.

## Disclosure

The default flow:

1. Maintainers acknowledge receipt and start triage in the private
   advisory thread.
2. We work with the reporter to scope a fix and a disclosure timeline.
3. We release a patched version.
4. We publish the GitHub Security Advisory and, where appropriate,
   request a CVE ID via the GHSA → CVE workflow.

**Default disclosure window:** 90 days from initial report. We'll
publish the advisory once a patched version is available *or* the
90-day window elapses, whichever is first — extendable by mutual
agreement when a fix is genuinely complex. The reporter is credited
in the advisory unless they prefer to remain anonymous.

We don't operate a paid bug-bounty program at this stage.

## What's in scope

- Vulnerabilities in `nt` / `nt-mcp` themselves (memory safety,
  injection, auth bypass, privilege escalation caused by the tool,
  protocol confusion in MCP stdio, etc.)
- Vulnerabilities in our published packages
  (`@magic-ingredients/no-tickets`, the `no-tickets` crate)
- **Supply-chain attacks targeting our distribution** — a compromised
  homebrew formula, hijacked crates.io release, install.sh integrity
  issues, namespace squatting against `@magic-ingredients/*`, etc.
- Vulnerabilities in the GitHub Actions workflows that produce our
  releases (the release pipeline is part of the supply chain)

## What's out of scope

- Vulnerabilities in **upstream dependencies that we consume as-is**
  — please report those upstream. We'll bump the dependency once a
  patched version exists and assess any user impact.
- Attacks that require the attacker to **already have** root or
  physical access to the user's machine. (Local privilege escalation
  caused by running `nt` itself is *in scope* — the distinction is
  prerequisite, not result.)
- Theoretical issues without a practical exploit path.
- Functional bugs without a security impact — please use the public
  [issue tracker](https://github.com/magic-ingredients/no-tickets/issues).
