# THE PERSON'S LLM ACCOUNTS, one SET each on the modeline: the account's own
# glyph first, coloured by how much of its limits is used — or, for Claude
# Code on an API key, which has no limits, whether Claude Code said its
# credits ran out — and then one glyph per agent working in it.
#
# A watch and a run. The watch hears the accounts' numbers move (the limits
# Claude Code's status line keeps, the usage endpoint's last answer, Codex's
# session logs), their agents work (the transcripts), and each agent start,
# wait and end (the sessions the managed hook keeps, and what Claude Code
# tells one of our terminals of itself) — inotify, never a poll; the one
# clock beside it asks an agent outside our terminals whether it still
# lives, and only while one is drawn. The run is the usage unit (`ronoctua-llm refresh`): it counts the
# transcripts and asks the usage endpoint, and the brain starts it only after
# an account was used and the last start was a while ago — no clock at rest.
# Why each is so is in `why.md`, beside this.
{
  what = "Mark the modeline with a set per LLM account — its glyph, coloured by how much of its limits is used (or whether an API key's credits ran out), and one glyph per agent working in it; count and ask again after one was used.";

  needs = {
    root = false;
    # NOT A HOLD: the run asks the network only for a Claude Code account
    # whose host the person let, and keeps the refusal beside the last
    # numbers when it cannot; the count it also does needs no network, so a
    # machine offline still gets its tokens counted.
    network = false;
    # WHERE THE ACCOUNTS ARE AND WHAT THEY SAID comes from the one crate that
    # reads the agents' state (`ronoctua_llm`: the list, `watched`, the
    # limits, the endpoint's last answer and an API key's word on its
    # credits, the sessions the managed hook keeps and the one reader that
    # merges them into agents); what a program in one of our terminals says
    # of itself (OSC 7501) is its holder's state file, read through the
    # engine-free terminals crate (`ronoctua_terminals`) — never a socket;
    # the lines a mark changes colour at are a file of the person's
    # (`ronoctua_store`); and the agents' trees are heard through the one
    # inotify door the brain already uses (`ronoctua_files::notice`).
    boundaries = [
      "ronoctua-llm"
      "ronoctua-terminals"
      "ronoctua-store"
      "ronoctua-files"
    ];
  };

  # NO CALENDAR AND NO COUNTDOWN: the run is wanted after an account was used
  # (`wanted_now`), never by a date — and outside the gate: it is a count of
  # what was appended since the last one and, at most, one small request, and
  # a person working on battery is exactly who wants the numbers. A few
  # minutes of patience: the unit counts and asks, and is gone.
  schedule = {
    kind = "routine";
    calendar = false;
    partOfDay = "any";
    countdownSeconds = 0;
    runawayMinutes = 5;
    gated = false;
  };

  # ONE UNIT AND NO REHEARSAL: a rehearsal of "count and ask" would ask the
  # network all the same. Declared by `module.nix`, under this name.
  units = {
    run = "ronoctua-llm-usage.service";
  };

  # Its row says "after an account was used" where a calendar row says a
  # date; the rest are why a line typed on its page was not kept.
  whys = [
    "when-used"
    "not-a-percent"
    "not-in-order"
    "unreadable"
    "unkept"
  ];

  # EACH ACCOUNT LEADS WITH ITS COMPANY'S MARK, read again in dots
  # (`assets/icons/marks.nix`): Claude Code's spark, the "A" of Claude Code
  # on an API key, and Codex's ring — similar, not the same. The owner took
  # back the rule of no brand on the modeline (2026-11-02): the robots that
  # stood for the accounts said "an agent" and not whose, and an account's
  # set has to say whose it is with its first glyph. The agents are a glyph
  # per state, the tone saying it again: working, waiting for you, done.
  # Failing is the done glyph in danger — danger is a colour, not another
  # drawing.
  glyphs = {
    claude = "ours-claude";
    claude-api = "ours-claude-api";
    codex = "ours-codex";
    working = "md-progress_clock";
    waiting = "md-progress_question";
    done = "md-progress_check";
    more = "md-dots_horizontal";
  };

  # ONE SET PER ACCOUNT, at rest too — the owner's exception
  # (`assets/brain-resting-marks.nix`, AGENTS.md): the account is there while
  # it is quiet, and its colour says when it is not. The set is the
  # account's place in the person's list, the same in the accounts' view, so
  # nine accounts at most have one; its lead draws no digit, and a click on
  # it opens the accounts' view on that account (`agents-open <n>`). An API
  # key's turns to danger while Claude Code says its credits ran out, and is
  # quiet otherwise: a key has no window to fill.
  #
  # THE AGENTS stand in their account's set, one glyph each and no digit: a
  # place of nine an agent keeps for as long as it lives, the lowest free one
  # when it is first seen. A tenth takes the ninth place's agent with it into
  # `agent-more`, the count of the hidden ones in the loudest of their tones
  # — so one waiting for you is still seen. A done agent and the count rest
  # (approved, 2026-11-02): an agent that finished is still there to go back
  # to. A click on an agent goes to it (`agents-show <session>`).
  marks = {
    claude = {
      glyph = "claude";
      hint = "agent-accounts-mark-claude";
      sets = 9;
      leads = true;
      resting = true;
    };
    claude-api = {
      glyph = "claude-api";
      hint = "agent-accounts-mark-claude-api";
      sets = 9;
      leads = true;
      resting = true;
    };
    codex = {
      glyph = "codex";
      hint = "agent-accounts-mark-codex";
      sets = 9;
      leads = true;
      resting = true;
    };
    agent-working = {
      glyph = "working";
      hint = "agent-accounts-mark-agent-working";
      sets = 9;
      instances = 9;
    };
    agent-waiting = {
      glyph = "waiting";
      hint = "agent-accounts-mark-agent-waiting";
      sets = 9;
      instances = 9;
    };
    agent-done = {
      glyph = "done";
      hint = "agent-accounts-mark-agent-done";
      sets = 9;
      instances = 9;
      resting = true;
    };
    agent-failed = {
      glyph = "done";
      hint = "agent-accounts-mark-agent-failed";
      sets = 9;
      instances = 9;
    };
    agent-more = {
      glyph = "more";
      hint = "agent-accounts-mark-agent-more";
      sets = 9;
      resting = true;
    };
  };

  # THE TWO LINES A MARK CHANGES COLOUR AT, typed on its page: warning from
  # 80 per cent, danger from 95, as the distro ships them.
  actions = {
    warn-at = {
      asks = "ask-agent-accounts-warn-at";
    };
    danger-at = {
      asks = "ask-agent-accounts-danger-at";
    };
  };

  # WHAT IT RELIES ON AND DOES NOT OWN: the accounts' crate, the terminals'
  # (their state files, where OSC 7501 is kept), and the NixOS module that
  # puts the accounts' CLI on the PATH — the program the unit runs, and the
  # managed hook that writes the sessions.
  uses = [
    "crates/ronoctua-llm"
    "crates/ronoctua-terminals"
    "nix/modules/llm.nix"
  ];

  # THE ACCOUNTS' VIEW IS THIS MODULE'S OTHER HALF: it draws the same lines
  # with the same colours, so it asks this module for them (`lines`,
  # `tone_of`, the glyphs of its marks) instead of keeping a second copy
  # that could disagree. And the owner's list of the marks drawn at rest
  # names the three leads, the done agent and the count.
  namedBy = {
    "assets/belongings.nix" = [ "agent-accounts" ];
    "assets/brain-resting-marks.nix" = [ "agent-accounts" ];
    "crates/ronoctua-shell-agents/src/agents.rs" = "*";
    # The check that every view says what of its file it did not read names
    # the accounts' lines file, which this module keeps.
    "tests/partly-read-checks.nix" = [ "agent-accounts" ];
  };
}
