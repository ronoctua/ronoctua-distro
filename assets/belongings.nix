# WHAT THE BELONGINGS KEEP — the one place it is declared.
#
# The belongings (`LEADER s v`) keep the person's configuration, encrypted, in a
# private git repository they name, and put it back on a new installation. This
# table says WHAT goes there and, for every choice, WHY: `nix/belongings.nix`
# (with `nix/scripts/gen-belongings.py`) turns it into the Rust table
# `ronoctua-belongings` includes (`RONOCTUA_BELONGINGS_RS`), and refuses a field
# it does not know or a reason with no words.
#
# # Where a path lives
#
# Every path is LOGICAL: its first word is the root it lives under, and the rest
# is relative to that root —
#
#   config/…   `$XDG_CONFIG_HOME/ronoctua` (`ronoctua-dev` in the `.#dev` loop)
#   state/…    `$XDG_STATE_HOME/ronoctua`
#   home/…     the person's home
#
# — so a machine whose roots are elsewhere, or the development loop's second
# copy of the system, restores into ITS roots. The repository's index carries
# the same logical paths, never a machine's absolute ones.
#
# # The four lists
#
#   trees    kept WHOLE, minus what `never` names
#   files    kept one by one, outside the trees
#   never    what is not kept, by a name pattern (`*.pid`) or a path; a `hard`
#            one is not kept even when the person adds it by hand
#   stores   the CENSUS: every preference file a `Store` of this tree writes,
#            with what the lists above make of it and why. The check
#            `belongings-manifest-covers-the-stores` refuses a store missing
#            here, and a test of the crate refuses a verdict here that the
#            lists do not produce — so a new preference file is a decision
#            somebody made, not one that fell out of a glob.
#
# Every `reason` is the id of a sentence (`belongings-reason-<id>`, in every
# language): the page "What is kept" shows it beside the path.
{
  # THE LINE PAST WHICH A PATH A PERSON ADDS IS SAID TO BE LARGE, in megabytes
  # (10⁶ bytes, the unit the screen writes sizes in): the repository is git,
  # and a whole video folder in it is a clone nobody waits for. Said, never
  # refused.
  largeMB = 100;

  trees = {
    # EVERY PREFERENCE OF THE DISTRO: theme, language, keyboards, the footer's
    # modes, the player's sources and keys, the brain modules' choices.
    "config" = "preference";
  };

  files = {
    # The programs the person installed and removed: on a new machine, the
    # list of what to bring back.
    "state/software/history.jsonl" = "programs";
  };

  never = [
    # A lock or a running process's number means nothing on another machine.
    {
      name = "*.pid";
      reason = "draft";
    }
    # A draft: `Store` writes `.<file>.<pid>.tmp` and renames it.
    {
      name = ".*";
      reason = "draft";
    }
    # THE COMPOSITOR'S AND THE TERMINAL'S FRAGMENTS, made from `theme.json`,
    # `display.json` and `pointers.json` by the machine that reads them.
    {
      name = "scroll-*.conf";
      reason = "generated";
    }
    {
      name = "ghostty-*.conf";
      reason = "generated";
    }
    {
      name = "*.kdl";
      reason = "generated";
    }
    # HOW THIS MACHINE REACHES THE REPOSITORY: its deploy key, its token, the
    # address — a new machine gets its own access, and a key in the repository
    # it opens would be a key anybody with the passphrase could use to push.
    {
      path = "config/belongings-keys";
      reason = "access";
      hard = true;
    }
    {
      path = "config/belongings.json";
      reason = "access";
      hard = true;
    }
    # The belongings' own record of what was synced, and the local clone.
    {
      path = "state/belongings";
      reason = "own-state";
      hard = true;
    }
    # SYNCTHING'S IDENTITY: two machines with the same device id collide and
    # take each other's connections. Out by default; a person who adds it by
    # hand asked for it.
    {
      path = "home/.local/state/syncthing";
      reason = "identity";
    }
  ];

  stores = {
    "config/atuin.json" = {
      kept = true;
      reason = "preference";
    };
    "config/audio.json" = {
      kept = true;
      reason = "preference";
    };
    # The reminders a person set, birthdays among them: theirs to bring back.
    "config/brain/reminders/reminders.json" = {
      kept = true;
      reason = "preference";
    };
    # The calculator's answers kept, which a click brings back.
    "config/calc.json" = {
      kept = true;
      reason = "preference";
    };
    "config/camera.json" = {
      kept = true;
      reason = "preference";
    };
    "config/clipcascade.json" = {
      kept = true;
      reason = "preference";
    };
    "config/consent.json" = {
      kept = true;
      reason = "preference";
    };
    "config/display.json" = {
      kept = true;
      reason = "preference";
    };
    # The folder each purpose of the file picker was left in.
    "config/fast-access.json" = {
      kept = true;
      reason = "preference";
    };
    "config/file-picker.json" = {
      kept = true;
      reason = "preference";
    };
    "config/gestures.json" = {
      kept = true;
      reason = "preference";
    };
    "config/index.json" = {
      kept = true;
      reason = "preference";
    };
    "config/keyboards.json" = {
      kept = true;
      reason = "preference";
    };
    "config/modeline.json" = {
      kept = true;
      reason = "preference";
    };
    "config/osk.json" = {
      kept = true;
      reason = "preference";
    };
    "config/player.json" = {
      kept = true;
      reason = "preference";
    };
    "config/player-favourites.json" = {
      kept = true;
      reason = "preference";
    };
    # The website page's addresses played lately.
    "config/player-websites.json" = {
      kept = true;
      reason = "preference";
    };
    "config/pointers.json" = {
      kept = true;
      reason = "preference";
    };
    "config/software.json" = {
      kept = true;
      reason = "preference";
    };
    "config/studio.json" = {
      kept = true;
      reason = "preference";
    };
    "config/studio/scenes.json" = {
      kept = true;
      reason = "preference";
    };
    "config/syncthing.json" = {
      kept = true;
      reason = "preference";
    };
    "config/toolkits.json" = {
      kept = true;
      reason = "preference";
    };
    "config/tor.json" = {
      kept = true;
      reason = "preference";
    };
    "config/world.json" = {
      kept = true;
      reason = "preference";
    };
    "config/belongings.json" = {
      kept = false;
      reason = "access";
    };
    "state/player.json" = {
      kept = false;
      reason = "machine";
    };
    "state/player-ambience.json" = {
      kept = false;
      reason = "machine";
    };
    "state/player-peers.json" = {
      kept = false;
      reason = "machine";
    };
    "state/player-recent.json" = {
      kept = false;
      reason = "history";
    };
    # What VirusTotal said of this machine's files, by their hashes, and the
    # uploads under way: of the files here, and of nowhere else.
    "state/resizer-last.json" = {
      kept = false;
      reason = "machine";
    };
    "state/scans.json" = {
      kept = false;
      reason = "machine";
    };
    "state/world-cache.json" = {
      kept = false;
      reason = "cache";
    };
  };
}
