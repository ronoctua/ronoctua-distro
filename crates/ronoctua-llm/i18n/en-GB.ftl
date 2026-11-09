# The texts `ronoctua-llm` says, in en-GB (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).

# OUR STATUS LINE, when it has no line of the person's to chain
# (`statusline::said`): the account's windows from Claude Code's own answer —
# how much of each is used and when it starts over.
llm-line-five-hours = 5 hours: { $percent }% ({ $left })
llm-line-week = Week: { $percent }% ({ $left })
llm-line-left-minutes = starts over in { $minutes } min
llm-line-left-hours = starts over in { $hours } h { $minutes } min
llm-line-left-days = starts over in { $days } d { $hours } h
llm-line-joined = { $first } · { $then }
