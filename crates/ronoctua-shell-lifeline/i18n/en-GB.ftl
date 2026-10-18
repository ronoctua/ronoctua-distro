# The texts `ronoctua-shell-lifeline` says, in en-GB (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

# =====================================================================
# THE LINE THAT OUTLIVES THE FOOTER — when the interface itself falls
# =====================================================================
#
# The owner moved the pointer across the hint line and everything closed. He
# named two defects and only one of them was the crash: *"o outro é o erro de
# que a ronoctua-distro não sabe lidar com isso, ou seja, acaba deixando o
# usuário sem interface"*. These are the words of the answer to the second one.
#
# It is a second layer-shell row raised by systemd's `OnFailure=`, and it is not
# a second chrome (invariant 14): it is the footer's own row drawn by another
# process, alive exactly while there is no chrome at all.
lifeline-fell = The interface closed on its own.
lifeline-fell-and-came-back = The interface closed on its own, and came back.
# THE UNIT'S OWN WORDS, and they are DATA — what the machine said, the way a
# filename is. `{ $why }` is systemd's `Result=`: `core-dump`, `signal`,
# `oom-kill`, `exit-code`.
lifeline-why = { $unit } · { $why }
# AND THE UNIT ALONE, when systemd has nothing to say about it — a
# transient unit is gone by the time the line asks. A dot between one
# thing and nothing separates nothing.
lifeline-unit = { $unit }
lifeline-logs = see the logs
lifeline-restart = restart the interface
lifeline-ai = ask an AI
lifeline-close = close
lifeline-logs-opened = the journal is open in a terminal
lifeline-restarting = systemd was asked to start it again
lifeline-would-not-start = systemd would not start it; the logs say why
lifeline-no-journalctl = the journal would not open; journalctl is not on this machine
# THE NESTED LOOP HAS NO UNIT. `nix/footer-single.nix` starts the footer as an
# ordinary background child, so there is nothing for systemd to restart — and
# saying so beats answering about a unit it has never heard of.
lifeline-not-a-unit = there is no systemd unit here; the nested loop starts it by hand

## file info editing — everything the info panel can change

# The lifeline's "ask an AI", through Claude Code (`lifeline.rs`): what would
# leave is gathered, shown in a terminal, and asked about with "no" first.
lifeline-ai-gathering = gathering what would go: the unit, how it ended, its journal and its core dump
lifeline-ai-no-report = the report could not be written, and nothing left this machine
lifeline-ai-shown = what would go to Claude Code is open in a terminal; nothing has left yet — send it?
lifeline-ai-no = no, keep it here
lifeline-ai-yes = yes, ask Claude Code
lifeline-ai-kept = nothing left this machine, and the report is gone
lifeline-ai-opened = Claude Code is open in a terminal, with the report
lifeline-ai-no-claude = Claude Code is not on this machine; it comes with the development profile's agents
# THE PROMPT Claude Code starts with, in the person's language. `{ $path }`
# is the report this line wrote, readable by this person alone.
lifeline-ai-prompt = The interface of my computer (the systemd user unit { $unit }) failed. The file { $path } has the unit, how it ended, the last lines of its journal and its core dump. Read it, tell me what most likely happened, and what I can do about it.
lifeline-report-title = What happened to the interface
lifeline-report-unit = Unit
lifeline-report-result = How it ended
lifeline-report-journal = The last lines of its journal
lifeline-report-core = Its core dump
lifeline-report-nothing = (nothing)
