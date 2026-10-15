# Security Policy

## Supported versions

Only the latest release and `main` get security fixes. A fix ships in a new
release, never as a back-port.

## Reporting a vulnerability

Report a vulnerability privately through GitHub Security Advisories:
<https://github.com/Vernum-Projecten/cs/security/advisories/new>. Do not open
a public issue, pull request or discussion for a security problem.

Include the version or commit, the steps to reproduce, and what an attacker
gains. Never include a real token, a Keychain item, an account email or the
content of a profile folder; describe them instead.

You get an acknowledgement within 7 days. Once a fix is released, the advisory
is published with credit to you, unless you ask to stay anonymous.

## Scope

cs reads no credentials of its own design: Claude Code stores each profile's
login in the macOS login Keychain. A report is in scope when cs exposes a
profile's login, session history or usage data to another user or process,
starts Claude Code with the wrong profile, or writes outside
`~/.claude-profiles/`. A vulnerability in Claude Code itself goes to
Anthropic: <https://www.anthropic.com/responsible-disclosure-policy>.
