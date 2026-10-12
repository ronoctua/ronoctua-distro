# The LLM accounts module's own texts: its name and the line under it, the
# hints of its two marks, its page — each account with how full it is, and the
# two lines a mark changes colour at — and the questions and refusals of the
# lines typed there. The accounts' VIEW speaks in its own words (`agents-…`).
brain-task-agent-accounts = LLM accounts
brain-task-agent-accounts-does = One mark per LLM account on the modeline, at rest too, coloured by how much of its limits is used. After an account was used — never on a clock — it counts the account's tokens and asks Anthropic's usage endpoint again, when you allowed it.
agent-accounts-mark-claude = Claude Code account “{ $label }”: its colour says how much of its limits is used — a click opens it
agent-accounts-mark-codex = Codex account “{ $label }”: its colour says how much of its limits is used — a click opens it
agent-accounts-why-when-used = After an account was used
agent-accounts-why-not-a-percent = That is not a whole per cent from 1 to 100.
agent-accounts-why-not-in-order = The warning has to come before the danger.
agent-accounts-why-unreadable = The lines' file (lines.json) does not read; nothing was written over it.
agent-accounts-why-unkept = The lines (lines.json) could not be written.
agent-accounts-heading-accounts = Accounts, by the most used of their limits (%)
agent-accounts-heading-lines = When a mark changes colour
agent-accounts-row-none = No account yet: add one in LLM accounts
agent-accounts-row-unreadable = The list of accounts does not read; nothing is written over it, and no mark is drawn
agent-accounts-row-unwatched = Not being watched
agent-accounts-row-lines-unwatched = The lines are not being watched: one changed elsewhere does not reach the marks
agent-accounts-note-claude = Claude Code
agent-accounts-note-codex = Codex
agent-accounts-note-no-mark = No mark: the modeline has a digit for the first nine accounts only
agent-accounts-unknown = Not known yet
agent-accounts-row-warn = A warning from (%)
agent-accounts-row-danger = Danger from (%)
cmd-agent-accounts-warn-at = Set how full a window has to be for its mark to warn
cmd-agent-accounts-danger-at = Set how full a window has to be for its mark to turn to danger
ask-agent-accounts-warn-at = A warning from what per cent (1–100)
ask-agent-accounts-danger-at = Danger from what per cent (1–100)
