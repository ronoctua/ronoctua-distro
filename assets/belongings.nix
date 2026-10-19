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

  # ── the 2026-10-05 wave: format 2 ─────────────────────────────────────
  #
  # THE PREDEFINED ENCRYPTED TARGETS (`~/.ssh`, git, GnuPG, the shell's own
  # file, the Emacs configuration), by id: `paths` (logical); `skip`, what is
  # never kept below them, each with its reason — a name pattern matches a
  # name at ANY depth, and one written `/name` only the first name below the
  # target's path (its root); `carry`, paths below a path that are kept
  # although a `skip` names a folder they are in, each name a pattern
  # (`straight/versions/*.el`); `private` (restored 0700/0600); `merge` (per
  # file name or `default`: `choose` or `union`); `offers` (`ssh-signing`,
  # `pam-u2f`).
  # ── anchor: L10 ──
  # Kept ENCRYPTED like the rest of the sealed part, on every machine, by
  # default. Each can be taken out on one machine ("not here": its paths go
  # into the person's `excluded`, the repository's copies stay for the others)
  # or on all of them ("nowhere": `requests.json` → the index's `dropped`, the
  # repository's copies deleted, the other machines forget theirs and keep
  # the files). THE ACCESS TO THE REPOSITORY NEVER TRAVELS among them: the
  # deploy key is refused by what it IS, under any name (`crate::sift`).
  #
  # A target never overlaps a tree, a file, a plain root, a path `never`
  # names or another target, and never a file Home Manager makes
  # (`belongings-targets-avoid-home-manager`): those are the system's.
  targets = {
    # THE PERSON'S SSH: keys, configuration, known hosts. Sockets and
    # multiplexing masters are not files of anybody's. A key's parts mean
    # nothing apart, so a change on both sides is a choice; a known host is a
    # fact per line, so both sides' lines are kept.
    ssh = {
      paths = [ "home/.ssh" ];
      skip = {
        "*.sock" = "socket";
        "control-*" = "connection";
        "S.*" = "socket";
      };
      private = true;
      merge = {
        default = "choose";
        known_hosts = "union";
      };
      offers = [ "ssh-signing" ];
    };
    # GIT'S OWN CONFIGURATION, and the security keys a person registered for
    # the password (`pam_u2f` reads `~/.config/Yubico/u2f_keys`). The signers
    # a person trusts are facts per line, like known hosts.
    git = {
      paths = [
        "home/.gitconfig"
        "home/.config/git"
        "home/.config/Yubico/u2f_keys"
      ];
      merge = {
        allowed_signers = "union";
      };
      offers = [
        "ssh-signing"
        "pam-u2f"
      ];
    };
    # GNUPG: the keyring and the trust database. The agent's sockets, its
    # random seed and its locks are this machine's moment, never the
    # person's; a keyring's parts mean nothing apart.
    gnupg = {
      paths = [ "home/.gnupg" ];
      skip = {
        "S.*" = "socket";
        "random_seed" = "random-seed";
        "*.lock" = "lock";
        ".#lk*" = "lock";
      };
      private = true;
      merge = {
        default = "choose";
      };
    };
    # The shell's own file of the person's (`nix/modules/home.nix` seeds it
    # once and never writes it again): tokens and credentials live there.
    shell = {
      paths = [ "home/.shellrc.local" ];
    };
    # THE PERSON'S EMACS CONFIGURATION, in ONE place: `~/.config/emacs`
    # (`$XDG_CONFIG_HOME/emacs`). The distro writes none
    # (`nix/modules/emacs.nix`), so what is there is theirs; and on a machine
    # with none of Emacs's places, the daemon's unit makes that folder, empty,
    # before Emacs starts — so a first configuration is born where it is
    # carried. Emacs's other places (`~/.emacs.d`, `~/.emacs`, `~/.emacs.el`)
    # and a framework's user folder are not carried: a person who keeps one
    # adds it by hand, and the page says which place Emacs reads here when it
    # is not this one (`ronoctua_belongings::emacs`) — it never moves one.
    #
    # WHAT IT SKIPS is what Emacs and its package managers make from a
    # configuration. At any depth: the packages (`elpa`, `straight`,
    # `elpaca`, `quelpa`, `el-get`, `.local`, the tree-sitter grammars) and
    # the compiled (`eln-cache`, `*.elc`, `*.eln`, a module's `*.so`), the
    # caches, a git repository's own folder, and an editor's backups,
    # autosaves and locks — a package folder never travels wherever a
    # configuration puts it. At the root only (`/`), where Emacs writes them:
    # what it keeps of this machine's sessions — `no-littering`'s `var`,
    # `transient`, `url`, `tramp`, `auto-save-list`, `backups`, the server's
    # key, the minibuffer's `history`, `recentf`, `places`, a saved desktop,
    # an LSP session, a database a package keeps and rebuilds (`org-roam.db`,
    # forge's) — so a snippet of the person's named `url` deeper down
    # travels. `transient`'s saved choices and `straight`'s lockfiles are the
    # person's, and are carried out of the folders that hold them.
    #
    # Elisp merged by hunk can make a configuration that no longer loads: a
    # file both sides changed is a choice.
    emacs = {
      paths = [ "home/.config/emacs" ];
      skip = {
        "elpa" = "packages";
        "straight" = "packages";
        "elpaca" = "packages";
        "quelpa" = "packages";
        "el-get" = "packages";
        ".local" = "packages";
        "tree-sitter" = "packages";
        "eln-cache" = "compiled";
        "*.elc" = "compiled";
        "*.eln" = "compiled";
        "*.so" = "compiled";
        ".cache" = "cache";
        ".git" = "git-folder";
        ".#*" = "editor-draft";
        "#*#" = "editor-draft";
        "*~" = "editor-draft";
        "*.lock" = "lock";
        "/var" = "editor-state";
        "/transient" = "editor-state";
        "/url" = "editor-state";
        "/tramp" = "editor-state";
        "/auto-save-list" = "editor-state";
        "/backups" = "editor-state";
        "/server" = "editor-state";
        "/history" = "editor-state";
        "/recentf" = "editor-state";
        "/places" = "editor-state";
        "/.emacs.desktop" = "editor-state";
        "/.lsp-session-v1" = "editor-state";
        "/*.db" = "editor-state";
        "/*.sqlite" = "editor-state";
      };
      carry = [
        "transient/values.el"
        "transient/levels.el"
        "straight/versions/*.el"
      ];
      merge = {
        default = "choose";
      };
    };
  };

  # THE PLAIN PART: logical roots kept IN CLEAR under `plain/<at>` (the
  # person's declared data, their brain modules), each with its reason.
  # ── anchor: L09 ──
  # What the machine reads BEFORE anybody types a passphrase — the update
  # fetches the declarations, the installer stages them before
  # `nixos-install`, a machine publishes its record after a switch — so it
  # travels with no identity, and the page says that whoever reads the
  # repository reads it. It is SIGNED with the person's ML-DSA-65 key
  # (`plain/signed.json`, `plain/signed.sig`): a change of the repository's
  # is taken by itself only as that key signed it, and anything else is held
  # with its reason (`ronoctua_belongings::plain::HeldWhy`); each machine's
  # record is not signed, and nothing reads it as configuration. A plain
  # root beats a tree (`config/brain-modules` is
  # under `config`, and is never sealed too); the name patterns of `never`
  # still apply below it, so a draft stays out.
  plain = {
    # The programs the person declared and declined, each machine's record
    # and their options (`ronoctua-person`'s files), as the machine mirrors
    # them for the root applier.
    "state/person" = {
      at = "person";
      reason = "person-data";
    };
    # The person's own brain modules (`my-…`), where they write them.
    "config/brain-modules" = {
      at = "brain-modules";
      reason = "brain-module";
    };
  };
  # The largest file the plain part keeps, in KiB.
  plainMostKiB = 8192;

  # THE CHUNKS of the sealed part (FastCDC), in KiB: no repository file
  # reaches GitHub's limit, and an edit moves few chunks.
  # ── anchor: L08 ──
  chunk = {
    minKiB = 256;
    avgKiB = 1024;
    maxKiB = 4096;
  };

  # THE BUDGET, in MB (10⁶ bytes): a push of new chunks above `pushMB` goes in
  # batches (GitHub refuses a push over 2 GiB); the view warns past `warnMB`
  # and recommends compacting past `recommendMB` or when the history weighs
  # `historyOverTree` times the tree; a compaction keeps `keepDays` of history.
  budget = {
    pushMB = 1500;
    recommendMB = 1000;
    warnMB = 750;
    historyOverTree = 2;
    keepDays = 30;
  };

  # HOW LONG THE IDENTITY STAYS IN THE KERNEL'S KEYRING at most, in hours —
  # the session's end (logout, suspend) forgets it first.
  unlockHours = 12;

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
    # A hidden file: an editor's swap (`.theme.json.swp`), an older draft
    # (`.<file>.<pid>.tmp`).
    {
      name = ".*";
      reason = "draft";
    }
    # A DRAFT of `write_atomically` (`ronoctua-xdg`): `<file>.<pid>.new`,
    # renamed over the file once it is whole — a process killed mid-write
    # leaves one behind, and it is half of something.
    {
      name = "*.new";
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
    # The two lines (in per cent) at which an LLM account's mark turns to a
    # warning and to danger: chosen once, the same on every machine.
    "config/brain/agent-accounts/lines.json" = {
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
    # The person's LLM accounts — a name, a kind, a folder, the default of
    # each kind — and not what is IN those folders: a sign-in and its
    # transcripts stay on the machine they were made on. On another machine
    # the list comes back, the person signs in again, and the accounts' view
    # offers the status line's wrap where it is not yet (`statusline::wrapped`).
    "config/llm-accounts.json" = {
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
    # The TLS group each host agreed with THIS machine (`ronoctua_http::groups`),
    # for the security view's post-quantum section: what this machine heard,
    # made again by its own next requests.
    "state/tls-groups.json" = {
      kept = false;
      reason = "machine";
    };
    "state/world-cache.json" = {
      kept = false;
      reason = "cache";
    };
    # THE STORES THAT WERE WRITTEN BY HAND until the lenient store
    # (2026-10-05), and the files `write_atomically` puts under a root.
    "config/brain.json" = {
      kept = true;
      reason = "preference";
    };
    "config/files-sidebar.json" = {
      kept = true;
      reason = "preference";
    };
    "config/language.json" = {
      kept = true;
      reason = "preference";
    };
    "config/locale.json" = {
      kept = true;
      reason = "preference";
    };
    "config/notifications.json" = {
      kept = true;
      reason = "preference";
    };
    "config/power.json" = {
      kept = true;
      reason = "preference";
    };
    "config/theme.json" = {
      kept = true;
      reason = "preference";
    };
    # Ghostty's size and the compositor's screens and pointers, made from
    # `toolkits.json`, `display.json` and `pointers.json` on each machine.
    "config/ghostty-size.conf" = {
      kept = false;
      reason = "generated";
    };
    "config/scroll-display.conf" = {
      kept = false;
      reason = "generated";
    };
    "config/scroll-pointers.conf" = {
      kept = false;
      reason = "generated";
    };
    # The player's queue and where it was in it: this machine's playing.
    "state/queue.rqueue" = {
      kept = false;
      reason = "machine";
    };
  };
}
