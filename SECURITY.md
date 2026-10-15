# Security Policy

## Supported versions

Only the latest release and `main` get security fixes. A fix ships in a new
release, never as a back-port.

## Reporting a vulnerability

The repository is private, so GitHub's private vulnerability reporting is not
available. Report a vulnerability directly to the maintainer, @rubentalstra,
and do not open an issue, pull request or discussion for it.

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
