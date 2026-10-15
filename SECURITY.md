# Security Policy

## Supported versions

Only the latest release and `main` get security fixes. A fix ships in a new
release, never as a back-port.

## Reporting a vulnerability

Report a vulnerability privately through GitHub: on the repository's
[Security tab](https://github.com/Vernum-Projecten/cc-shift/security), choose
**Report a vulnerability**. Do not open an issue, pull request or discussion
for it.

Include the version or commit, the steps to reproduce, and what an attacker
gains. Never include a real token, a Keychain item, an account email or the
content of a profile folder; describe them instead.

You get an acknowledgement within 7 days. Once a fix is released, the advisory
is published with credit to you, unless you ask to stay anonymous.

## Scope

cc-shift handles Claude Code's logins: it moves them between Keychain items on a
switch, refreshes the parked ones, and reads their access token to fetch
usage. It keeps a token in memory only, passes it on stdin, and never writes
one to a file or log. A report is in scope when cc-shift exposes a login,
session history or usage data to another user or process, leaves two live
copies of one login or moves the wrong one, or writes outside the places
ARCHITECTURE.md section 6 lists. A vulnerability in Claude Code itself goes to
Anthropic: <https://www.anthropic.com/responsible-disclosure-policy>.
