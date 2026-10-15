---
name: cut-releases-promptly
description: "Cut a release the moment its milestone's code work is done; move an owner-side or upstream-blocked straggler to the next milestone instead of holding the cut"
metadata:
  type: feedback
---

On another of the owner's projects two milestones had every code issue
closed, and the session held both cuts for owner-side items and an upstream
release. The owner asked why they were not cut.

**Why:** a milestone is a delivery promise for the work this repository
controls. An owner action or an upstream release is not that work, and
holding a finished release for it delays everything behind it.

**How to apply:** when a milestone's remaining open issues are all owner-side
or blocked upstream, move them to the next milestone with a comment saying
why, and cut at once (`.claude/rules/issue-workflow.md` §Milestones =
releases). Releases go in order, so a held release holds every later one.
When a cut is late, ask whether to skip the stale version rather than cutting
each in turn. Linked: [[milestones-0-0-x]], [[release-tag-is-mine]].
