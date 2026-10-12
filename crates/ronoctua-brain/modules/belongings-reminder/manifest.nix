# THE BELONGINGS, REMINDED — a watch, and a mark while there is something to
# do about them.
#
# The belongings themselves (`LEADER s v`: the view, the queue's CLI, the
# repository) are not this module's; it reads the files they leave and says
# on the modeline when a sync is due, when a sync waits for decisions or
# failed, and when this machine has not restored them yet. And it sends the
# PLAIN part — the person's declarations and brain modules, which need no
# passphrase — by itself, once it has stayed quiet. Why each is so is in
# `why.md`, beside this.
{
  what = "Remind of a sync of the belongings when it is due; mark the modeline while decisions wait, after a failed sync, and before the first; send the plain part once it stays quiet.";

  needs = {
    root = false;
    network = false;
    # THE BELONGINGS' FILES, as their crate reads them (`prefs`: no `age`).
    boundaries = [ "ronoctua-belongings" ];
  };

  # A WATCH, and no run of its own: the sync is the person's (it asks the
  # passphrase), so the brain can only remind — at `last + period`, with a
  # clock armed for that moment alone, and when the files it reads move.
  schedule = null;

  # THE ONE UNIT IT STARTS, declared beside this (`module.nix`): the plain
  # part alone (`sync-plain`), which asks nothing — no passphrase, no
  # identity — 30 seconds after the plain roots last moved.
  units = {
    plain = "ronoctua-belongings-plain.service";
  };

  # A BRIEFCASE for the belongings, and what happens to it: a clock when it is
  # due, a pencil while decisions wait, a cross after a failure, and the arrow
  # that brings it down onto a new machine.
  glyphs = {
    due = "md-briefcase_clock";
    conflicts = "md-briefcase_edit";
    failed = "md-briefcase_remove";
    restore = "md-briefcase_download";
  };

  # ONE MARK AT A TIME, and only while it says something (invariant 14). A
  # click on any opens the belongings asking the passphrase (`command:` in the
  # mark's act — a command of the footer's, not an action of this module).
  marks = {
    due = {
      glyph = "due";
      hint = "belongings-reminder-mark-due";
    };
    conflicts = {
      glyph = "conflicts";
      hint = "belongings-reminder-mark-conflicts";
    };
    failed = {
      glyph = "failed";
      hint = "belongings-reminder-mark-failed";
    };
    restore = {
      glyph = "restore";
      hint = "belongings-reminder-mark-restore";
    };
  };

  # WHAT IT RELIES ON AND DOES NOT OWN: the crate whose files it reads, and
  # the module that declares the CLI its unit runs.
  uses = [
    "crates/ronoctua-belongings"
    "nix/modules/belongings.nix"
  ];

  namedBy = { };
}
