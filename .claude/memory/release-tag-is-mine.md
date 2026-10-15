---
name: release-tag-is-mine
description: "The session cuts every release end to end (version bump PR, merge, signed tag, push, reading the release run), never handing the tag to the owner"
metadata:
  type: feedback
---

On another of the owner's projects a pull request body told the owner which
`git tag -s` and `git push` commands to run. The owner: "never do this okay!!
... you always run the tag never say that i need to do that".

**Why:** the owner wants the cut finished from the session like every other
step of a milestone; a hand-back is unfinished work.

**How to apply:** when a milestone empties, the next unit is the release:

1. A version-bump pull request that moves the `version` in `Cargo.toml` and
   every other file that states it.
2. Its merge, then `git tag -s vX.Y.Z -m vX.Y.Z` and `git push origin vX.Y.Z`
   from the session, then reading the release run, if one exists, and fixing
   what fails.
3. A new milestone for stragglers if needed, and the emptied milestone closed
   (`gh api -X PATCH repos/Vernum-Projecten/cc-shift/milestones/<n> -f
   state=closed`).

Docs and PR bodies never say "the owner runs" for a tag. See
[[cut-releases-promptly]].
