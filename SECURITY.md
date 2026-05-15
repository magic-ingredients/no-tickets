# Security Policy

## Reporting a vulnerability

If you've found a security vulnerability in `nt`, `nt-mcp`, or the
no-tickets npm SDK, please **report it privately** rather than opening
a public issue or pull request.

The preferred channel is GitHub's private vulnerability reporting:
**[Open a private advisory](https://github.com/magic-ingredients/no-tickets/security/advisories/new)**

GitHub will notify the maintainers, and the advisory thread keeps the
report private until disclosure is coordinated.

### What helps us triage

- The affected binary or package (`nt`, `nt-mcp`, or
  `@magic-ingredients/no-tickets`) and its version (`nt --version`
  output for the binaries)
- The OS and install channel (curl / brew / cargo / direct)
- Reproduction steps or a proof-of-concept
- Your assessment of impact (what an attacker can do, what they need
  to start)

## Supported versions

no-tickets is pre-1.0. Only the **latest release** receives security
patches — please upgrade before reporting an issue against an older
version.

| Version | Supported |
|---------|-----------|
| Latest  | ✅        |
| Earlier | ❌        |

`nt self-update` (for install.sh / direct-download installs) and your
package manager (`brew upgrade no-tickets`, `cargo install --force
no-tickets`, etc.) update to the latest release in-place.

## Disclosure

We coordinate disclosure with the reporter. The default flow is:

1. Maintainers acknowledge receipt and start triage in the private
   advisory thread.
2. If the report is in scope, we work with the reporter to scope a
   fix and agree on a disclosure window.
3. We release a patched version and publish the advisory after the
   patched version is available.

We don't operate a formal bug-bounty program, but credit is given in
the advisory unless the reporter prefers anonymity.

## Out of scope

The following aren't in scope for this policy:

- Vulnerabilities in third-party dependencies — please report
  upstream. We'll bump the dependency once a patched version exists.
- Issues requiring physical access to a user's machine or root on
  the host already.
- Theoretical issues without a practical exploit path.
- Functional bugs without security impact — please use the public
  [issue tracker](https://github.com/magic-ingredients/no-tickets/issues).
