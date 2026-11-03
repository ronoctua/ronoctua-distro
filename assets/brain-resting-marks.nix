# THE MARKS THAT MAY BE DRAWN AT REST, `<module>/<mark>` — the owner's
# approval, one by one.
#
# The modeline at rest is silent: a mark exists only when it says something
# (AGENTS.md, "Linhas, cores e cursor"). A mark published in `Tone::Quiet`
# says only that all is well, so the brain's core drops it unless its
# manifest declares `resting = true` (`Manifest::refusal_of`), and the loader
# (`nix/brain-modules.nix`) lets a mark declare that only when it is listed
# here. AGENTS.md names the same marks beside the rule they are the exception
# to, and `resting-marks-are-the-approved-ones` keeps the two lists one.
#
# 2026-10-05: the LLM accounts — one mark per account, its usage told by the
# tone, so it is there while the account is quiet too. 2026-11-02: and Claude
# Code on an API key, its tone telling whether its credits ran out; the three
# now LEAD their account's set. And in the set, an agent that is done — still
# there to go back to — and the count of the agents past the ninth place,
# whose tone is the loudest of theirs and is quiet when they are all done.
[
  "agent-accounts/agent-done"
  "agent-accounts/agent-more"
  "agent-accounts/claude"
  "agent-accounts/claude-api"
  "agent-accounts/codex"
]
