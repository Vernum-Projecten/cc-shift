---
# SPDX-FileCopyrightText: 2026 Vernum Projecten B.V.
# SPDX-License-Identifier: Apache-2.0
name: mermaid-diagrams
description: "ARCHITECTURE.md may carry mermaid diagrams; render every fence with mermaid-cli before a pull request, and keep semicolons out of sequence messages"
metadata:
  type: feedback
---

The owner likes a design explained with mermaid diagrams as well as text,
because a picture is faster to check than prose. GitHub renders them. In
`ARCHITECTURE.md` the obvious candidates are the switch (status line
reading, threshold, moving the login, windows following) and automatic
selection (read usage, rank, switch).

**Why:** on another of the owner's projects a diagram shipped that did not
parse (a semicolon in a sequence message ends the statement), and GitHub
showed a parse error where the picture should be.

**How to apply:** extract every ```` ```mermaid ```` fence and render it with
`npx -y -p @mermaid-js/mermaid-cli mmdc -p <puppeteer.json> -i x.mmd -o x.svg`,
with the puppeteer config's `executablePath` pointing at the installed Google
Chrome. Use `<br/>` for line breaks in labels, never `\n`; keep `;` out of
sequence messages; quote any label carrying parentheses, colons or braces.
