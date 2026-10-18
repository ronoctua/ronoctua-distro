# THE PERSON'S LLM ACCOUNTS, one mark each on the modeline, coloured by how
# much of the account's limits is used.
#
# A watch and a run. The watch hears the accounts' numbers move (the limits
# Claude Code's status line keeps, the usage endpoint's last answer, Codex's
# session logs) and their agents work (the transcripts) — inotify, never a
# poll. The run is the usage unit (`ronoctua-llm refresh`): it counts the
# transcripts and asks the usage endpoint, and the brain starts it only after
# an account was used and the last start was a while ago — no clock at rest.
# Why each is so is in `why.md`, beside this.
{
  what = "Mark the modeline with each LLM account, coloured by how much of its limits is used; count and ask again after one was used.";

  needs = {
    root = false;
    # NOT A HOLD: the run asks the network only for a Claude Code account
    # whose host the person let, and keeps the refusal beside the last
    # numbers when it cannot; the count it also does needs no network, so a
    # machine offline still gets its tokens counted.
    network = false;
    # WHERE THE ACCOUNTS ARE AND WHAT THEY SAID comes from the one crate that
    # reads the agents' state (`ronoctua_llm`: the list, `watched`, the
    # limits and the endpoint's last answer); the lines a mark changes colour
    # at are a file of the person's (`ronoctua_store`); and the agents' trees
    # are heard through the one inotify door the brain already uses
    # (`ronoctua_files::notice`).
    boundaries = [
      "ronoctua-llm"
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

  # A ROBOT, two of them: no brand on the modeline (the owner's rule for the
  # accounts' marks). Claude Code's is the plain one, Codex's the industrial
  # one; the account's number is drawn beside it.
  glyphs = {
    claude = "md-robot_outline";
    codex = "md-robot_industrial_outline";
  };

  # ONE MARK PER ACCOUNT, at rest too — the owner's exception
  # (`assets/brain-resting-marks.nix`, AGENTS.md): the account is there while
  # it is quiet, and its colour says when it is not. The digit is the
  # account's place in the person's list, the same in the accounts' view, so
  # nine accounts at most carry one. A click opens the accounts' view on that
  # account (`agents-open <n>`).
  marks = {
    claude = {
      glyph = "claude";
      hint = "agent-accounts-mark-claude";
      instances = 9;
      resting = true;
    };
    codex = {
      glyph = "codex";
      hint = "agent-accounts-mark-codex";
      instances = 9;
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

  # WHAT IT RELIES ON AND DOES NOT OWN: the accounts' crate, and the NixOS
  # module that puts its CLI on the PATH — the program the unit runs.
  uses = [
    "crates/ronoctua-llm"
    "nix/modules/llm.nix"
  ];

  # THE ACCOUNTS' VIEW IS THIS MODULE'S OTHER HALF: it draws the same lines
  # with the same colours, so it asks this module for them (`lines`,
  # `tone_of`, the glyphs of its marks) instead of keeping a second copy
  # that could disagree. And the owner's list of the marks drawn at rest
  # names these two.
  namedBy = {
    "assets/belongings.nix" = [ "agent-accounts" ];
    "assets/brain-resting-marks.nix" = [ "agent-accounts" ];
    "crates/ronoctua-shell-agents/src/agents.rs" = "*";
  };
}
