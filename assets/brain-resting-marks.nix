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
# tone, so it is there while the account is quiet too.
[
  "agent-accounts/claude"
  "agent-accounts/codex"
]
