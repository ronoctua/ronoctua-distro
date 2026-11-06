# The LLM accounts module's own texts: its name and the line under it, the
# hints of its marks — each account's lead and its agents' — its page, each
# account with how full it is and the two lines a mark changes colour at, and
# the questions and refusals of the lines typed there. The accounts' VIEW
# speaks in its own words (`agents-…`).
brain-task-agent-accounts = LLM accounts
brain-task-agent-accounts-does = Shows each of your LLM accounts on the modeline, even at rest: the account's glyph, whose colour says how much of its limits is used (for an API key, whether its credits ran out), and one glyph for each agent working in it. Usage is checked again only after the account is used, never on a timer, and asking Anthropic needs your permission.
brain-task-agent-accounts-brief = Shows each LLM account's usage, and the agents working in it, on the modeline.
agent-accounts-mark-claude = Claude Code account “{ $label }”: its colour says how much of its limits is used, and its agents stand beside it — a click opens it
agent-accounts-mark-codex = Codex account “{ $label }”: its colour says how much of its limits is used — a click opens it
agent-accounts-mark-claude-api = Claude Code account “{ $label }”, on an API key: it turns to danger when Claude Code says its credits ran out, and its agents stand beside it — a click opens it
agent-accounts-mark-agent-working = An agent is working in “{ $label }” — a click takes you to it
agent-accounts-mark-agent-waiting = An agent in “{ $label }” is waiting for you — a click takes you to it
agent-accounts-mark-agent-done = An agent in “{ $label }” is done — a click takes you to it
agent-accounts-mark-agent-failed = An agent in “{ $label }” stopped on an error — a click takes you to it
agent-accounts-mark-agent-more = { $count ->
        [one] One more agent of this account, in the loudest of their colours
       *[other] { $count } more agents of this account, in the loudest of their colours
    } — a click opens the account
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
agent-accounts-row-agents-unwatched = The agents are not being watched: one that starts, waits or ends is not seen on the modeline
agent-accounts-note-claude = Claude Code
agent-accounts-note-claude-api = Claude Code, on an API key
agent-accounts-note-codex = Codex
agent-accounts-note-no-mark = No set: the modeline has room for the first nine accounts only
agent-accounts-unknown = Not known yet
agent-accounts-credits-ok = Not out of credits
agent-accounts-credits-out = Out of credits
agent-accounts-row-warn = A warning from (%)
agent-accounts-row-danger = Danger from (%)
cmd-agent-accounts-warn-at = Set how full a window has to be for its mark to warn
cmd-agent-accounts-danger-at = Set how full a window has to be for its mark to turn to danger
ask-agent-accounts-warn-at = A warning from what per cent (1–100)
ask-agent-accounts-danger-at = Danger from what per cent (1–100)
