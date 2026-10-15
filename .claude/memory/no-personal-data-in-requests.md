---
name: no-personal-data-in-requests
description: "No worker sends the owner's email address or any personal contact to an external service, including a crates.io or API User-Agent header; use a neutral agent string"
metadata:
  type: feedback
---

On another of the owner's projects a worker checking crate versions sent a
crates.io API request whose User-Agent carried the owner's email address,
following crates.io's guidance to include contact details.

**Why:** the address is personal data, and a request header to a public API
is stored in that service's logs outside the owner's control. For `cc-shift` the
same goes for the email address of any of the accounts it manages.

**How to apply:** every external call (crates.io, GitHub raw content, a
documentation site) uses a neutral User-Agent such as `cc-shift-dev` and carries no
personal contact. Every worker brief that may query an external API says so.
A slip is reported to the owner, never hidden.
