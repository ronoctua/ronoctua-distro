# Everything this distro knows how to update, in one sequence.
#
# ONE PLACE. Every command that upgrades, refreshes or reclaims anything on this
# machine is declared here and generated from here: the Rust table the view will
# draw, the two shell sequences the units run, the list of text keys both
# languages have to carry, and the refusal of an invalid field — all at build
# time, in the same shape as `assets/notifications.nix` and `assets/tokens/*.nix`.
#
# Written as a script instead, "which ones are there?" would be a grep through
# `nix/modules/*.nix`, and the answer would be wrong: the rebuild lives in
# `rescue.nix`, the garbage collection in `memory.nix`, the tldr cache in
# `cli.nix`, `fc-cache` inside a Rust crate, and `nix flake update` was a shell
# ALIAS. Five files, five conventions, and nothing that runs them in order.
#
# THE ORDER IS THE LIST'S OWN ORDER, and there is no `order` field on purpose.
# Two sources for one sequence is two things to keep in step, and the one that
# loses is always the one nobody reads. `assets/notifications.nix` already says
# it of its filters — "ORDER MATTERS: the FIRST one that catches decides" — and
# the reasoning is the same here.
#
# THE INVARIANT THIS FILE WALKS INTO, IN THE OPEN
# ------------------------------------------------
# The owner asked for the language managers too — rust, go, js, npm. Invariant 6
# is "nothing imperative: if it is not in the flake, it does not exist", and
# `nix/modules/dev.nix` opens by naming those managers as the thing this distro
# refuses: each keeps a compiler under $HOME, and then "which rustc is this"
# depends on which shell was opened.
#
# Both halves are kept, and this is how:
#
#   1. Every step declares a PRECONDITION. A step whose precondition is false is
#      SKIPPED AND SAID SO — never skipped in silence, which is the only way a
#      declared step can be honest about a machine it does not fit.
#   2. NO STEP MAY INSTALL ANYTHING. Updating what is already there is not
#      imperative package management; putting it there is. The generator refuses
#      any argument that contains `install`, every install verb it knows, every
#      shell metacharacter and every way of spelling "run this through a shell"
#      — so there is no argument left that could smuggle one in.
#   3. A step that names a BARE BINARY — one the flake does not provide — is
#      REFUSED unless it is off by default. That is not a convention: the
#      generator will not build a table where such a step ships on, because a
#      default that depends on something outside the flake is invariant 6 broken
#      by the distro itself. Turning one on is one line in `configuration.nix`,
#      on the machine where the owner put the tool there by hand — the same
#      shape as the `~/.shellrc.local` escape hatch `dev.nix` already sanctions.
#
# ROOT FIRST, THEN THE PERSON, AND NEVER INTERLEAVED
# --------------------------------------------------
# Two managers run this: a system unit for what needs root, and the person's own
# user unit for what does not. Interleaving them would cost a round trip through
# polkit at every switch, and a sequence whose order changes when somebody flips
# a `root =` is a sequence nobody can predict. The generator refuses it.
#
# The split is not a workaround, it is the point: a tool somebody installed by
# hand lives on THEIR `PATH`, in THEIR home, and the only process that can find
# it is one running as them. That is also why no language manager can ever run
# as root here — the generator refuses that too.
#
# THE FIELDS
# ----------
#   id          kebab-case, English (invariant 18). It is the id everywhere:
#               the two text keys are `update-<id>` and `update-does-<id>`, the
#               line in `/etc/ronoctua/updates.conf` is `<id> on|off`, and the
#               report the view reads is keyed by it.
#   does        move-inputs | apply-configuration | upgrade-software |
#               refresh-cache | reclaim-space
#               What class of thing this step is, and it is load-bearing: there
#               is exactly ONE `move-inputs` and exactly ONE
#               `apply-configuration`, `move-inputs` comes before it, and every
#               `upgrade-software` and every `reclaim-space` comes after it. The
#               generator enforces all five. `upgrade-software` is the person's
#               own programs (invariant 6's dated exception): it moves what is
#               installed to a newer version and never adds one.
#   root        true  -> the system unit, as root, from a store path
#               false -> the person's user unit, as them, with their PATH
#   package +   the store path. `package` is the attribute in `nixpkgs`,
#   program     `program` the file under its `bin/`. Never a bare name where
#               root runs it: a unit's PATH is minimal and the failure would
#               appear far from the cause.
#   unit        instead of package+program: start a systemd unit that already
#               exists. Root only — starting a unit as root asks nobody.
#   binary      instead of package+program: a name looked up on the person's own
#               PATH. Non-root only, off by default only, and it must appear in
#               `needs` as well, so a missing tool is a SKIP and not a failure.
#   run         the arguments, whole and fixed. Nothing is ever appended at run
#               time; there is no shell; the set of commands this machine can
#               run through here is finite and readable in this file.
#   needs       a LIST of preconditions, ALL of which must hold. A term is
#               `{ <kind> = <value>; }` with exactly one kind:
#                 always         nothing can make this step inapplicable
#                 binary         this name is on PATH
#                 directory      this directory exists
#                 writable-file  this file exists and can be written
#                 network        NetworkManager says the machine is connected
#               An absolute path everywhere; `$HOME/`, `$XDG_DATA_HOME/`,
#               `$XDG_CONFIG_HOME/`, `$XDG_STATE_HOME/` and `$XDG_CACHE_HOME/`
#               are allowed as well, and only in a non-root step: root's `$HOME`
#               is `/root`, and a step that swept it would sweep the wrong home.
#   on-failure  stop | carry-on
#   enable      whether the distro ships it on. `/etc/ronoctua/updates.conf`,
#               written from `ronoctua.update.steps`, overrules this per machine.
{
  steps = [
    # =================================================================
    # THE ROOT HALF
    # =================================================================

    # 1. THE ONLY STEP THAT MOVES THE SYSTEM FORWARD.
    #
    # Everything after it applies, refreshes, reclaims or moves the person's
    # own programs; this is the one that moves the pinned inputs forward, and
    # therefore the one that can bring a kernel that does not boot. That risk is not answered here and it does not
    # need to be: the way back is the previous generation, which the boot menu
    # already offers, and which `store-collect-garbage` below is written so as
    # not to delete.
    #
    # `carry-on` AND NOT `stop`, which is the less obvious of the two. Failing
    # here leaves the lock untouched and the machine exactly as it was — but the
    # working tree of `/etc/nixos` may still have moved (somebody edited
    # `host.nix`), and the running generation may already be older than what
    # `/etc/nixos` describes. Applying that is still worth doing, so the sequence
    # says the update failed and goes on to apply what is there.
    #
    # The `writable-file` precondition is what makes this safe on the ISO, where
    # `/etc/nixos` is a read-only symlink into the store: the step is skipped and
    # says so, instead of failing with a permission error nobody expected.
    {
      id = "flake-update";
      does = "move-inputs";
      root = true;
      package = "nix";
      program = "nix";
      run = [
        "flake"
        "update"
        "--flake"
        "/etc/nixos"
      ];
      needs = [
        { writable-file = "/etc/nixos/flake.lock"; }
        { network = true; }
      ];
      on-failure = "carry-on";
      enable = true;
    }

    # 2. AND IT IS THE REBUILD THAT ALREADY EXISTS.
    #
    # `nix/modules/rescue.nix` declares `ronoctua-rescue-rebuild.service`, and it
    # is the only `nixos-rebuild switch` in this repository. A second one here
    # would be a second copy of `--flake /etc/nixos#<person>` — the attribute
    # named after the person, which `rescue-flake-target-exists` exists to keep
    # true. One copy, one check, one thing to get wrong.
    #
    # Started as root, so nothing is asked of anybody: polkit is only consulted
    # when the caller is not root, which is why the person's half starts the
    # system unit and the system unit starts this.
    #
    # `stop`, and the reason is concrete rather than tidy. A failed switch leaves
    # the store full of the new generation's half-built dependencies; the very
    # next step is a garbage collection, which would delete every one of them and
    # make the next attempt recompile the workspace from nothing. Stopping also
    # puts the failure where it can be seen, instead of under four more lines
    # that all say "ok".
    {
      id = "system-rebuild";
      does = "apply-configuration";
      root = true;
      unit = "ronoctua-rescue-rebuild.service";
      needs = [ { writable-file = "/etc/nixos/flake.lock"; } ];
      on-failure = "stop";
      enable = true;
    }

    # 3. THE SYSTEM'S FLATPAKS, AFTER THE REBUILD.
    #
    # The system installation holds what the machine declares through
    # nix-flatpak (`nix/modules/flatpak.nix`) — the Flathub remote, and a
    # runtime or an app only if one is ever declared. The person's apps are in
    # their own installation and move in the person's half, below.
    #
    # `flatpak` DIRECTLY and not through our CLI, because nothing here needs
    # what the CLI adds: there is no consent to remember and no lane to share
    # with a screen, since the screen only ever touches `--user`. `update`
    # moves what is installed and installs nothing; `-y` answers the one
    # question it may ask (a runtime that moved to a new branch) the way the
    # update already decided.
    #
    # AFTER the rebuild, so it is the Flatpak the rebuild just installed, and
    # `carry-on`: a Flathub that does not answer is not a reason to leave the
    # store full of garbage. The directory is where the system installation
    # keeps its repository — on a machine with no Flatpak support there is
    # none, and the step says it does not apply.
    {
      id = "flatpak-system";
      does = "upgrade-software";
      root = true;
      package = "flatpak";
      program = "flatpak";
      run = [
        "update"
        "--system"
        "--noninteractive"
        "-y"
      ];
      needs = [
        { directory = "/var/lib/flatpak/repo"; }
        { network = true; }
      ];
      on-failure = "carry-on";
      enable = true;
    }

    # 4. THE ANDROID SYSTEM IMAGE — OFF.
    #
    # Waydroid's own `upgrade`, run by our CLI because it has to wait: the
    # images cannot be replaced under a running Android, so the CLI checks the
    # session first and says so rather than stopping somebody's app. It is a
    # root step because `/var/lib/waydroid` is root's (`nix/modules/android.nix`).
    #
    # OFF: it rewrites the system a person's Android apps run on, a gigabyte
    # at a time, and that is a decision rather than a schedule — one line,
    # `ronoctua.update.steps.android-images = true;`, on the machine that
    # wants it. The directory only exists once Android was prepared.
    {
      id = "android-images";
      does = "upgrade-software";
      root = true;
      package = "software";
      program = "ronoctua-software";
      run = [
        "upgrade"
        "android-system"
      ];
      needs = [
        { directory = "/var/lib/waydroid/images"; }
        { network = true; }
      ];
      on-failure = "carry-on";
      enable = false;
    }

    # 5. GARBAGE ONLY, AND THE GENERATIONS STAY.
    #
    # This is `nix-collect-garbage` with no `--delete-older-than`, and that is
    # the whole decision: the sequence has just switched to a generation whose
    # kernel has never booted, and deleting the older ones in the same breath is
    # deleting the way back. `nix/modules/base.nix` already sweeps 30 days
    # weekly, on a day when nothing has just changed — which is when that sweep
    # belongs. The 30-day one is declared below and ships OFF.
    {
      id = "store-collect-garbage";
      does = "reclaim-space";
      root = true;
      package = "nix";
      program = "nix-collect-garbage";
      run = [ ];
      needs = [ { always = true; } ];
      on-failure = "carry-on";
      enable = true;
    }

    # 6. AND THEN DEDUPLICATE WHAT IS LEFT.
    #
    # AFTER the collection and not before: optimising a path that is about to be
    # deleted is work thrown away. `auto-optimise-store` is already on
    # (`base.nix`), so this only ever finds what was written before it was, or by
    # something that bypassed it — usually a handful of paths, sometimes a lot
    # after a big rebuild.
    {
      id = "store-optimise";
      does = "reclaim-space";
      root = true;
      package = "nix";
      program = "nix";
      run = [
        "store"
        "optimise"
      ];
      needs = [ { always = true; } ];
      on-failure = "carry-on";
      enable = true;
    }

    # 7. THE ONE THAT DELETES THE WAY BACK — OFF.
    #
    # Same command `nix/modules/memory.nix` offers as `@generations`, and the
    # same reasoning that put it behind its own key there: it removes the older
    # generations, so after it the boot menu has nothing to fall back to. In a
    # sequence that has just installed a new kernel, that is the one thing that
    # must not happen by default.
    #
    # Turn it on for a machine short of disk that is willing to pay that: one
    # line, `ronoctua.update.steps.old-generations = true;`.
    {
      id = "old-generations";
      does = "reclaim-space";
      root = true;
      package = "nix";
      program = "nix-collect-garbage";
      run = [
        "--delete-older-than"
        "30d"
      ];
      needs = [ { always = true; } ];
      on-failure = "carry-on";
      enable = false;
    }

    # 8. AND THE JOURNAL — ALSO OFF, AND FOR A SMALLER REASON.
    #
    # The journal is the only record of what this very sequence did. Vacuuming it
    # at the end of the run is throwing away the answer to "why did the rebuild
    # fail?" a few minutes after acquiring it. `LEADER s m` offers it as a
    # deliberate act, which is where it belongs.
    #
    # `directory` and not `always`: with a volatile journal there is no
    # `/var/log/journal`, and the step should say it does not apply rather than
    # succeed at doing nothing.
    {
      id = "journal-vacuum";
      does = "reclaim-space";
      root = true;
      package = "systemd";
      program = "journalctl";
      run = [ "--vacuum-size=256M" ];
      needs = [ { directory = "/var/log/journal"; } ];
      on-failure = "carry-on";
      enable = false;
    }

    # =================================================================
    # THE PERSON'S HALF — their PATH, their home, no privilege at all
    # =================================================================

    # 9. THE FONT CACHE, AFTER the rebuild and not before.
    #
    # A rebuild can change the font set — `nix/fonts.nix` builds five derivations
    # and 27 variants — and a cache built before it describes the previous
    # generation. `-rf` is the same pair the rescue console uses
    # (`crates/ronoctua-rescue/src/action.rs`): rebuild from scratch, ignore the
    # timestamps, because a stale cache whose mtime looks fresh is exactly the
    # case that brings somebody here.
    {
      id = "font-cache";
      does = "refresh-cache";
      root = false;
      package = "fontconfig";
      program = "fc-cache";
      run = [ "-rf" ];
      needs = [ { always = true; } ];
      on-failure = "carry-on";
      enable = true;
    }

    # 10. THE TLDR PAGES.
    #
    # `nix/modules/cli.nix` already updates them weekly, as `nobody` — which
    # builds a cache in a home nobody reads. Run as the person, it lands in
    # theirs. `network`, because it downloads: without it the step waits for a
    # timeout and then reports a failure, where the honest answer is that this
    # machine is not on a network right now.
    {
      id = "tldr-cache";
      does = "refresh-cache";
      root = false;
      package = "tealdeer";
      program = "tldr";
      run = [ "--update" ];
      needs = [ { network = true; } ];
      on-failure = "carry-on";
      enable = true;
    }

    # =================================================================
    # THE PERSON'S OWN PROGRAMS — what they installed, moved forward
    # =================================================================
    #
    # Invariant 6's dated exception (`AGENTS.md`, 2026-09-26): the person
    # installs programs for themselves, and the update keeps them current. Four
    # steps, one per source, and every one of them is `ronoctua-software
    # upgrade <source>` — the SAME program and the same per-source lock the
    # software view's own "update" takes, so a run started by the brain and a
    # button pressed on the screen can never move one source at once.
    #
    # `upgrade` MOVES ONLY WHAT THE PERSON HAS: it reads what is installed and
    # never adds a program, and the generator refuses every argument that could
    # say otherwise, with no exception for these.
    #
    # EACH ONE'S DIRECTORY EXISTS ONLY WHEN THAT SOURCE HAS SOMETHING, so a
    # machine where nobody ever installed an AppImage skips the AppImage step
    # and says so, instead of running a program to find nothing.

    # 11. THE NIX PROFILE: `nix profile upgrade`, against the `nixpkgs` the rebuild
    # just pinned in the registry (`nix/modules/base.nix`). The CLI splits the
    # free packages from the ones the person consented to as not free — those
    # need `--impure`, and only for the packages that were agreed to.
    {
      id = "nix-profile";
      does = "upgrade-software";
      root = false;
      package = "software";
      program = "ronoctua-software";
      run = [
        "upgrade"
        "nix"
      ];
      needs = [
        { directory = "$XDG_STATE_HOME/nix/profiles"; }
        { network = true; }
      ];
      on-failure = "carry-on";
      enable = true;
    }

    # 12. THE PERSON'S FLATPAKS: the `--user` installation, apps and runtimes
    # together, and Flathub's catalogue with them.
    {
      id = "flatpak-user";
      does = "upgrade-software";
      root = false;
      package = "software";
      program = "ronoctua-software";
      run = [
        "upgrade"
        "flatpak"
      ];
      needs = [
        { directory = "$XDG_DATA_HOME/flatpak/repo"; }
        { network = true; }
      ];
      on-failure = "carry-on";
      enable = true;
    }

    # 13. THE APPIMAGES in `~/Applications`, each against the releases of the
    # place it came from. OFF here, and `nix/modules/software.nix` turns it on
    # where the machine has `appimage-run`: a machine that cannot run one has
    # nothing to update.
    {
      id = "appimages";
      does = "upgrade-software";
      root = false;
      package = "software";
      program = "ronoctua-software";
      run = [
        "upgrade"
        "appimage"
      ];
      needs = [
        { directory = "$HOME/Applications"; }
        { network = true; }
      ];
      on-failure = "carry-on";
      enable = false;
    }

    # 14. THE APPS INSIDE ANDROID, from F-Droid, every one checked against the
    # signer and the digest the index names. OFF here for the reason the
    # AppImages are; `nix/modules/software.nix` turns it on with Waydroid.
    {
      id = "android-apps";
      does = "upgrade-software";
      root = false;
      package = "software";
      program = "ronoctua-software";
      run = [
        "upgrade"
        "android"
      ];
      needs = [
        { directory = "$XDG_DATA_HOME/waydroid"; }
        { network = true; }
      ];
      on-failure = "carry-on";
      enable = false;
    }

    # =================================================================
    # THE LANGUAGE MANAGERS — declared, off, and each with its reason
    # =================================================================
    #
    # None of these exists on a machine that only does what this distro says it
    # does. They are here because the owner asked for them by name and because a
    # feature that silently drops half a request is worse than one that says
    # where the line is: each ships OFF, each names a bare binary, each is
    # skipped out loud when that binary is not there, and none of them can ever
    # run as root. The generator enforces all four.
    #
    # WHAT IS DELIBERATELY ABSENT, and why, because the absences are the part
    # that would otherwise become a grep:
    #
    #   go            has no update command. Updating a Go tool means installing
    #                 it again (`go install …@latest`), which is the one thing
    #                 no step here may do. There is nothing to declare.
    #   rustup self   replaces the `rustup` binary in place. Under Nix that
    #     / bun       binary is in the store and the store is read-only, so the
    #     upgrade     step could only ever fail. A step that cannot work is worse
    #                 than an absent one: it teaches people to ignore a red line.
    #   cargo         `cargo install-update` is an installer by name and by
    #     install-    behaviour — it re-installs every binary it manages. The
    #     update      generator refuses the word, and it is right to.

    # RUST TOOLCHAINS. `rustup update` refreshes the channels already installed
    # under `~/.rustup`; it does not add one. The directory precondition is what
    # makes that true in practice: with no `~/.rustup` there is nothing to
    # refresh, and the step says so.
    {
      id = "rust-toolchains";
      does = "refresh-cache";
      root = false;
      binary = "rustup";
      run = [ "update" ];
      needs = [
        { binary = "rustup"; }
        { directory = "$HOME/.rustup"; }
        { network = true; }
      ];
      on-failure = "carry-on";
      enable = false;
    }

    # NODE'S GLOBAL PACKAGES. `npm update --global` moves what is already in the
    # global prefix; it installs nothing new. `nodejs_22` comes from the flake,
    # but a global prefix under `$XDG_DATA_HOME` does not — which is precisely
    # what the directory precondition tests.
    {
      id = "node-global";
      does = "refresh-cache";
      root = false;
      binary = "npm";
      run = [
        "update"
        "--global"
      ];
      needs = [
        { binary = "npm"; }
        { directory = "$XDG_DATA_HOME/npm/lib/node_modules"; }
        { network = true; }
      ];
      on-failure = "carry-on";
      enable = false;
    }

    # THE SAME FOR PNPM, which keeps its own global store and does not share
    # npm's. Two managers, two directories, two steps — one step testing one
    # directory and running the other manager is how a skip becomes a lie.
    {
      id = "pnpm-global";
      does = "refresh-cache";
      root = false;
      binary = "pnpm";
      run = [
        "update"
        "--global"
      ];
      needs = [
        { binary = "pnpm"; }
        { directory = "$XDG_DATA_HOME/pnpm"; }
        { network = true; }
      ];
      on-failure = "carry-on";
      enable = false;
    }

    # PYTHON'S TOOLS, through `uv`. `uv` itself comes from the flake
    # (`dev.nix`), and the tools it manages do not: they live in
    # `$XDG_DATA_HOME/uv/tools`, one venv each, put there by hand. `upgrade
    # --all` moves the ones that are there and adds none.
    {
      id = "python-tools";
      does = "refresh-cache";
      root = false;
      binary = "uv";
      run = [
        "tool"
        "upgrade"
        "--all"
      ];
      needs = [
        { binary = "uv"; }
        { directory = "$XDG_DATA_HOME/uv/tools"; }
        { network = true; }
      ];
      on-failure = "carry-on";
      enable = false;
    }

    # 19. DOOM EMACS, RESOLVED AGAINST WHAT WAS JUST INSTALLED.
    #
    # The owner asked for it: when the machine updates itself it should bring
    # the editor's packages with it. It belongs here and not in a task of its
    # own because it is a CONSEQUENCE of the rebuild — `nix/modules/emacs.nix`
    # writes `init.el`, `packages.el` and `config.el` from the repository on
    # every rebuild, and until `doom sync` runs, the framework is still
    # resolving the previous ones.
    #
    # `binary` AND NOT `package`, which the generator then forces off by
    # default. Both halves of that are right:
    #
    #   - `ronoctua-doom-sync` IS from the flake, but naming it as a package
    #     would put a store path in the script on EVERY machine, and with it
    #     Emacs and the fifteen programs `doom doctor` wants — about 0.6 GiB,
    #     into a minimal profile that has no editor at all. `binary` is looked
    #     up by name, so a machine without the Emacs module simply does not
    #     find it and says so.
    #   - and a bare binary must not ship ON (invariant 6). It does not: what
    #     turns it on is `nix/modules/emacs.nix`, through
    #     `ronoctua.update.steps`, which is the module that KNOWS Emacs is
    #     installed. So a machine with the editor has this on and a machine
    #     without it has it off, and neither of them was guessed at.
    #
    # THE FRAMEWORK'S DIRECTORY IS THE GATE. `$XDG_CONFIG_HOME/emacs` is where
    # Doom itself lives — cloned by hand, once, which `nix/modules/emacs.nix`
    # calls the only command on this system that reaches outside the flake.
    # Without it the step skips, which is what makes this "keep an installed
    # Doom in step" and never "install one".
    {
      id = "doom-sync";
      does = "refresh-cache";
      root = false;
      binary = "ronoctua-doom-sync";
      run = [ ];
      needs = [
        { binary = "ronoctua-doom-sync"; }
        { directory = "$XDG_CONFIG_HOME/emacs"; }
        { network = true; }
      ];
      on-failure = "carry-on";
      enable = false;
    }

    # =================================================================
    # AND WHAT THE PERSON'S PROGRAMS LEFT BEHIND — last
    # =================================================================
    #
    # After every upgrade, because an upgrade is what leaves the leftovers:
    # the runtime an app stopped using, the profile generation it replaced.
    # `ronoctua-software tidy <source>` and never the managers' own commands:
    # Flatpak's is a removal verb with a flag, which the generator refuses by
    # name, and it is right to — the refusal is what keeps every removal out
    # of this sequence, and the CLI is what knows which leftovers nothing uses.

    # 20. FLATPAK RUNTIMES NO APP NEEDS ANY MORE, in the person's installation.
    {
      id = "flatpak-unused";
      does = "reclaim-space";
      root = false;
      package = "software";
      program = "ronoctua-software";
      run = [
        "tidy"
        "flatpak"
      ];
      needs = [ { directory = "$XDG_DATA_HOME/flatpak/repo"; } ];
      on-failure = "carry-on";
      enable = true;
    }

    # 21. THE NIX PROFILE'S OLD GENERATIONS. Each one keeps the programs it had
    # alive in the store, and a generation older than the CLI's window is a
    # way back nobody is going to take — the software view's "go back" reaches
    # the recent ones, which this leaves.
    {
      id = "nix-profile-history";
      does = "reclaim-space";
      root = false;
      package = "software";
      program = "ronoctua-software";
      run = [
        "tidy"
        "nix"
      ];
      needs = [ { directory = "$XDG_STATE_HOME/nix/profiles"; } ];
      on-failure = "carry-on";
      enable = true;
    }

    # 22. WHAT `appimage-run` UNPACKED for AppImages that are gone or were
    # replaced. It unpacks each one into `$XDG_CACHE_HOME/appimage-run/<digest>`
    # on the first start and never removes it; the CLI removes the ones whose
    # AppImage no longer exists. OFF here, on where the machine has the support.
    {
      id = "appimage-cache";
      does = "reclaim-space";
      root = false;
      package = "software";
      program = "ronoctua-software";
      run = [
        "tidy"
        "appimage"
      ];
      needs = [ { directory = "$XDG_CACHE_HOME/appimage-run"; } ];
      on-failure = "carry-on";
      enable = false;
    }
  ];
}
