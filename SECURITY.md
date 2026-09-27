# Security Policy

## Reporting a vulnerability

If you suspect a security vulnerability in UPeg, **do not open a public
issue**. Report it through GitHub's
[private vulnerability reporting](https://docs.github.com/en/code-security/security-advisories/guidance-on-reporting-and-writing/privately-reporting-a-security-vulnerability):

1. Open the `5pecia1/UPeg` repository's **Security** tab.
2. Click **Report a vulnerability**.
3. Include a minimal reproduction, the affected scope, and — if known — the
   affected version or snapshot.

If that path is unavailable, contact the maintainer through the contact
channels on their GitHub profile. Never post vulnerability details in public
channels (issues, discussions, PRs).

## Response goal

- We aim to give an initial response within **7 business days**; this is not
  a guarantee.
- There is **no bug bounty** program.

## Scope

Security reports in these areas are especially welcome:

- The HTTP server and MCP (Model Context Protocol) server surfaces
- The Chrome extension (`chrome-ext/`)
- Tool execution paths (e.g. arbitrary code execution, sandbox escapes,
  path escapes)

Build-script and CI-configuration vulnerabilities are also of interest, but
the three areas above are the priority.

## Supported versions

Before the first versioned release, security fixes are provided for the
latest `main` snapshot. Once versioned releases begin, fixes are provided
for the **latest published release** only. Backports to older versions are
not guaranteed.
