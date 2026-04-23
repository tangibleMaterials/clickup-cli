# Security Policy

## Supported versions

Only the latest release line receives security fixes. Older minor versions are not backported.

| Version | Supported |
| --- | --- |
| 0.8.x   | ✅ |
| < 0.8   | ❌ |

## Reporting a vulnerability

**Please do not report security issues in public GitHub issues, discussions, or the AUR comment thread.**

Use GitHub's private vulnerability reporting:

1. Go to <https://github.com/nicholasbester/clickup-cli/security/advisories/new>
2. Describe the issue, the impact, and a reproduction if you have one
3. I'll respond within 72 hours to acknowledge

If GitHub's private channel isn't an option, reach out via the email on my GitHub profile and we can move to a private channel.

## Scope

In scope:

- Vulnerabilities in `clickup` CLI binary or the MCP server shipped with it
- Credential leakage, token logging, insecure defaults
- Issues in the release pipeline that could let a third party inject code into published artifacts (crates.io, npm, AUR, Homebrew, GitHub Releases)

Out of scope:

- Vulnerabilities in the ClickUp API itself — report to ClickUp directly
- Vulnerabilities in upstream Rust dependencies — Dependabot handles those automatically; a direct advisory to the upstream crate is more effective
- Social-engineering attacks against the maintainer

## Response

- **Acknowledge:** within 72 hours
- **Severity assessment:** within 7 days
- **Fix in a patch release:** as soon as a viable fix is ready, typically within 14 days for high/critical severity
- **Coordinated disclosure:** after a fix ships on crates.io / npm / GitHub Releases, the advisory is published via GitHub with credit to the reporter unless they prefer to stay anonymous

Thanks for helping keep clickup-cli safe.
