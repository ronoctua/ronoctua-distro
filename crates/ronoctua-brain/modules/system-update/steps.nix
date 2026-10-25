# Everything this distro knows how to update, in one sequence.
#
# ONE PLACE. Every command that upgrades, refreshes or reclaims anything on this
# machine is declared here and generated from here (`sequence.nix`, which runs
# `gen-updates.py` beside it): the Rust table the view will draw, the two shell
# sequences the units run, the list of text keys both languages have to carry,
# and the refusal of an invalid field — all at build time, in the same shape as
# `assets/notifications.nix` and `assets/tokens/*.nix`.
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
#      WHAT IS DECLARED IS NOT AN INSTALL BY HAND (invariant 6 as the owner
#      amended it, 2026-10-05): the update brings in what the distro or the
#      person DECLARED, and nothing else — the rebuild builds it, and the
#      one step that reconciles (`reconcile-declared`) brings the declared
#      Android apps from the machine's copy and touches only its own ledger.
#      No argument of it names a program: what it installs is the
#      declaration's, read by the program, never this table's.
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
#               the two text keys are `<module>-step-<id>` and
#               `<module>-step-<id>-does` (the module is this folder's name, and
#               its `i18n/` carries both in every language), the line in
#               `/etc/ronoctua/updates.conf` is `<id> on|off`, and the report
#               the view reads is keyed by it.
#   does        gather | migrate | move-inputs | apply-configuration |
#               realize-parts | upgrade-software | refresh-cache |
#               reclaim-space | reconcile-declared | publish
#               What class of thing this step is, and it is load-bearing: there
#               is exactly ONE `move-inputs` and exactly ONE
#               `apply-configuration`, `move-inputs` comes before it, and every
#               `upgrade-software` and every `reclaim-space` comes after it. The
#               generator enforces all five. `upgrade-software` is the person's
#               own programs (invariant 6's dated exception): it moves what is
#               installed to a newer version and never adds one.
#               And the 2026-10-05 laws: `gather` is the person's and comes
#               before the root half, and nothing else of the person's does;
#               `migrate` is root's and comes before `move-inputs`;
#               `reconcile-declared` is the person's, after
#               `apply-configuration`; `publish` is the person's and last.
#               And the 2026-10-23 law: exactly one `realize-parts`, root's,
#               after `apply-configuration`, before every `reclaim-space`, and
#               `carry-on`.
#   root        true  -> the system unit, as root, from a store path
#               false -> the person's user unit, as them, with their PATH
#   package +   the store path. `package` is the attribute in `nixpkgs`,
#   program     `program` the file under its `bin/`. Never a bare name where
#               root runs it: a unit's PATH is minimal and the failure would
#               appear far from the cause.
#   unit        instead of package+program: start a systemd unit that already
#               exists. Root only — starting a unit as root asks nobody.
#   rehearsal   with `unit` only, and optional: the unit a REHEARSAL starts in
#               its place — one that runs the same program changing nothing.
#               Without it a rehearsal prints the `systemctl start` and runs
#               nothing, which is all it can honestly do for a unit; with it the
#               rehearsal asks the program itself what it would do.
#   binary      instead of package+program: a name looked up on the person's own
#               PATH. Non-root only, off by default only, and it must appear in
#               `needs` as well, so a missing tool is a SKIP and not a failure.
#   run         the arguments, whole and fixed. Nothing is ever appended at run
#               time; there is no shell; the set of commands this machine can
#               run through here is finite and readable in this file.
#   rehearse    with a program only, and optional: the arguments a REHEARSAL
#               runs the same program with — the form of it that changes
#               nothing (`apply --rehearse`). Every
#               argument goes through the refusals `run`'s do. Without it a
#               rehearsal prints the command and runs nothing, which is all it
#               can honestly do for a program that has no such form.
#   declines    with a program only, and optional: the refusals (`refused
#               <word>`, the program's last line) that are NEWS and not a
#               failure. Recorded `declined-<word>`, and the sequence goes on.
#   needs       a LIST of preconditions, ALL of which must hold. A term is
#               `{ <kind> = <value>; }` with exactly one kind:
#                 always         nothing can make this step inapplicable
#                 binary         this name is on PATH
#                 directory      this directory exists
#                 writable-file  this file exists and can be written
#                 network        NetworkManager says the machine is connected
#                 file           this file exists
#                 power          on mains, or the battery at this many per
#                                cent or more, or the person's yes to this run
#                                on battery — one floor for the whole table
#                 free-space     { at; percent; cap-gib; }: that share of the
#                                filesystem holding `at` is free, never more
#                                than the cap; below it the store's garbage
#                                is collected FIRST, then the step is
#                                refused. Root only.
#                 not-held       the machine's record names no hold
#               An absolute path everywhere; `$HOME/`, `$XDG_DATA_HOME/`,
#               `$XDG_CONFIG_HOME/`, `$XDG_STATE_HOME/` and `$XDG_CACHE_HOME/`
#               are allowed as well, and only in a non-root step: root's `$HOME`
#               is `/root`, and a step that swept it would sweep the wrong home.
#   on-failure  stop | carry-on
#   enable      whether the distro ships it on. `/etc/ronoctua/updates.conf`,
#               written from `ronoctua.update.steps`, overrules this per machine.
#
# THE MACHINE DIRECTORY is not written here: it has one name, in
# `nix/machine.nix` (`machine-dir-has-one-name`), and every step that names it
# — the lock it moves, the flake it rebuilds — reads it from there.
let
  machineDir = (import ../../../../nix/machine.nix).machineDir;

  # THE BATTERY FLOOR, in per cent: on battery below it, the steps that
  # compile — the migration, the rebuild — and the one
  # that moves the lock the rebuild builds from are skipped,
  # unless the person said yes to THIS run (the doors ask with the no first;
  # `ronoctua-update --execute --on-battery` from a terminal). A rebuild of
  # this distro is twenty minutes of every core, which takes a third of a
  # laptop's charge; forty leaves a machine that finishes it with room to
  # spare. One number for the table (the generator refuses two): the doors
  # ask below it, and two would be two questions.
  batteryFloor = 40;

  # THE ROOM A REBUILD WANTS, in the shape `assets/cache.nix` gives every size:
  # a share of the disk under the store, capped. The NEARLY-FULL line, the
  # one the brain marks the modeline at — below the store's floor, so Nix's
  # own `min-free` has already collected what it could by then, and the
  # precondition collects once more before refusing.
  cache = import ../../../../assets/cache.nix;
  roomForARebuild = {
    at = "/nix/store";
    percent = cache.diskWarnPercent;
    cap-gib = cache.diskWarnCapGiB;
  };

in
{
  steps = [
    # THE 2026-10-05 WAVE: the person's half that GATHERS before the root
    # half (fetch, pins, stage), after its anchor.
    # ── anchor: L19 ──

    # =================================================================
    # THE PERSON'S HALF, FIRST — GATHERING what the root half will read
    # =================================================================
    #
    # The person declares programs on any of their machines, and the
    # declarations travel in the plain part of their belongings
    # (`plain/person/`). Before root moves anything, the person's half brings
    # in what the OTHER machines declared, moves the pins forward and stages
    # the result for root's applier — so the rebuild below builds what the
    # person has declared everywhere, and not last week's. All three run as
    # the person: the repository's access, the session's consent and the
    # staging folder are theirs, and root has none of them.
    #
    # `carry-on` all three: a machine that cannot reach the repository, or
    # whose pins do not move today, still updates with the declarations it
    # already has.

    # G1. THE PLAIN PART OF THE BELONGINGS, PULLED: no passphrase, the sealed
    # part untouched, every conflict left as this machine has it (conflicts
    # never block; the view says there are some and opens the belongings).
    # A machine with no repository configured skips it, and says so.
    {
      id = "person-fetch";
      does = "gather";
      root = false;
      package = "belongings";
      program = "ronoctua-belongings";
      run = [
        "plain"
        "pull"
        "--json"
      ];
      needs = [
        { file = "$XDG_CONFIG_HOME/ronoctua/belongings.json"; }
        { network = true; }
      ];
      on-failure = "carry-on";
      enable = true;
    }

    # G2. THE PINS, MOVED FORWARD: an AppImage that follows its releases, an
    # Android app that follows F-Droid — each to the version and the digest
    # it has now, written into the declaration. And, while the person's
    # options say to follow them (the owner's decision, 2026-10-20), the
    # distro's AppImages that follow a repository (the Crafting Apps), past
    # the pins the distro tested, into their `software/distro-pins.json`;
    # with the option off, that file goes. No argument names a program: the
    # table says what follows, the program reads it from the system's
    # report. Without the person's folder there is nothing to move.
    # `github-not-let` is news: GitHub is not in their consent, and the
    # distro's AppImages keep the pins they had.
    {
      id = "person-pins";
      does = "gather";
      root = false;
      package = "software";
      program = "ronoctua-software";
      run = [
        "pins"
        "refresh"
      ];
      declines = [ "github-not-let" ];
      needs = [
        { directory = "$XDG_STATE_HOME/ronoctua/person"; }
        { network = true; }
      ];
      on-failure = "carry-on";
      enable = true;
    }

    # G2b. THE DECLARATIONS, SIGNED HERE: what G1 brought and G2 moved is
    # the person's plain part as it is now, and root takes from a staging
    # only what the person signed (`ronoctua-person apply`, against the key
    # the machine pinned). With the identity the session keeps, the moved
    # pins arrive signed; without it the step declines (`no-passphrase`):
    # what the person's other machines signed, which G1 brought with its
    # signature, still arrives, and only what changed HERE unsigned (the
    # moved pins, a change made here) waits — root keeps its copy of that
    # file until this machine signs it. Nothing here ever asks. No network.
    {
      id = "person-sign";
      does = "gather";
      root = false;
      package = "belongings";
      program = "ronoctua-belongings";
      run = [ "sign" ];
      declines = [
        "no-passphrase"
        "empty"
      ];
      needs = [ { file = "$XDG_CONFIG_HOME/ronoctua/belongings.json"; } ];
      on-failure = "carry-on";
      enable = true;
    }

    # G3. THE DECLARATIONS, HANDED TO ROOT: the person's copy put where root's
    # narrow applier (`ronoctua-person-apply.service`) takes it, the unit
    # started as the software centre starts it, and its verdict read — so the
    # root half builds what the person's signature covers now
    # (what G1 brought signed, and what G2b signed here), and not the copy
    # the machine had. Validated and swapped into the machine directory
    # whole — data, never Nix code. `busy` is news: another of root's
    # programs (the software centre's rebuild) held the machine, and this
    # run builds what it had. A rehearsal only checks that it would be taken
    # (`--check`).
    {
      id = "person-stage";
      does = "gather";
      root = false;
      package = "software";
      program = "ronoctua-software";
      run = [
        "person"
        "stage"
      ];
      rehearse = [
        "person"
        "stage"
        "--check"
      ];
      declines = [ "busy" ];
      needs = [ { directory = "$XDG_STATE_HOME/ronoctua/person"; } ];
      on-failure = "carry-on";
      enable = true;
    }

    # =================================================================
    # THE ROOT HALF
    # =================================================================
    #
    # It runs under `ronoctua-person hold` (`module.nix`): root's lock of the
    # person's data, held from the migration to the end of the half and
    # handed to every step as its standard input — so an apply the person
    # starts meanwhile waits instead of slipping its declarations under the
    # rebuild.

    # 0. A MACHINE STILL IN THE OLD LAYOUT BECOMES ITS OWN FLAKE.
    #
    # Before 2026-10-05 a machine carried a COPY of the distro in its machine
    # directory; the template's flake has the distro as an input instead
    # (`nix/machine.nix`). `ronoctua-machine migrate` builds the new layout
    # beside the old one and swaps them only when the new one builds — a
    # failure leaves the machine exactly as it was. The old layout is known
    # by a file only it has (the distro's own `base.nix`), and a machine
    # without it skips the step and says so.
    #
    # `stop`: every step after this one speaks the new layout, and running
    # them against the old one would move a lock that is not the machine
    # flake's. `nothing-to-migrate` is news and not a failure: the program
    # looked at the folder and found the new layout already there.
    {
      id = "machine-migrate";
      does = "migrate";
      root = true;
      package = "machine";
      program = "ronoctua-machine";
      run = [ "migrate" ];
      declines = [ "nothing-to-migrate" ];
      needs = [
        { file = "${machineDir}/nix/modules/base.nix"; }
        { power = batteryFloor; }
        { free-space = roomForARebuild; }
      ];
      on-failure = "stop";
      enable = true;
    }

    # 1. THE ONLY STEP THAT MOVES THE SYSTEM FORWARD.
    #
    # Everything after it applies, refreshes, reclaims or moves the person's
    # own programs; this is the one that moves the pinned inputs forward, and
    # therefore the one that can bring a kernel that does not boot. That risk is not answered here and it does not
    # need to be: the way back is the previous generation, which the boot menu
    # already offers, and which `old-generations` below always keeps — with the
    # booted one — and `store-collect-garbage` cannot touch.
    #
    # `carry-on` AND NOT `stop`, which is the less obvious of the two. Failing
    # here leaves the lock untouched and the machine exactly as it was — but the
    # machine directory may still have moved (somebody edited `host.nix`), and
    # the running generation may already be older than what the machine
    # directory describes. Applying that is still worth doing, so the sequence
    # says the update failed and goes on to apply what is there.
    #
    # The `writable-file` precondition is what makes this safe on the ISO, where
    # the machine directory is a read-only symlink into the store: the step is skipped and
    # says so, instead of failing with a permission error nobody expected.
    #
    # ONE INPUT, THE DISTRO's — `nix flake update <distro>`, never the whole
    # lock. The machine flake's other inputs are the distro's own (copied
    # from its lock, which `lib.machine` refuses to see changed), so moving
    # the distro moves everything the release was tested with, and nothing
    # it was not.
    #
    # AND NOT WHILE THE MACHINE IS HELD (`ronoctua-machine hold <rev>`): the
    # record says which revision it stays on, and the step is skipped and
    # says so. The rebuild after it still applies what the folder holds.
    #
    # AND ONLY WHEN THE REBUILD COULD BE TRIED: the rebuild's battery floor
    # and its room on the store's disk are this step's too (the generator
    # refuses a `move-inputs` without them). The machine directory describes
    # the system that runs; a lock moved on a run whose rebuild is then
    # skipped for the battery or the disk would name a distro nobody built,
    # and the next apply — the software centre's, the rescue's — would build
    # and switch to all of it, unverified, on that battery or that disk.
    {
      id = "flake-update";
      does = "move-inputs";
      root = true;
      # `nix flake update <distro>` through the machine's program, which
      # first puts a machine the installer AIMED at its own tree back on the
      # release (`ronoctua_machine::aim`): a `path:` input updated stays the
      # same path.
      package = "machine";
      program = "ronoctua-machine";
      run = [ "update-distro" ];
      needs = [
        { writable-file = "${machineDir}/flake.lock"; }
        { not-held = true; }
        { power = batteryFloor; }
        { free-space = roomForARebuild; }
        { network = true; }
      ];
      on-failure = "carry-on";
      enable = true;
    }

    # 2. AND IT IS THE MACHINE'S OWN APPLY.
    #
    # `ronoctua-machine apply` is the one build-then-switch of this
    # repository: the rescue console's rebuild runs it too, and the software
    # centre's apply without moving the inputs. It BUILDS first and switches
    # only what built — a failed build puts the lock back on the system that
    # runs, so the machine directory always describes the running system —
    # and after the switch it arms the boot trial: the next boot tries the
    # new system once, and comes back to the last good one by itself if it
    # does not start (`ronoctua-boot-bless.service`).
    #
    # A CHILD OF THIS HALF, and not a unit started from it: root's lock of
    # the person's data reaches it on its standard input, so no apply of the
    # person's declarations swaps them while it builds. (A declared package
    # is a part: the system never builds one, and its realizer evaluates it
    # alone after the switch.) Its rehearsal measures on a copy what the update would download
    # and build, and changes nothing.
    #
    # `did-not-boot` IS NEWS AND NOT A FAILURE: after a trial that did not
    # start, the same system is refused until the distro moves on — there is
    # nothing new to apply, and the rest of the update goes on.
    #
    # It needs the battery floor (twenty minutes of every core) and room on
    # the store's disk, garbage collected first; short of either it is
    # skipped and says which.
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
      package = "machine";
      program = "ronoctua-machine";
      run = [ "apply" ];
      rehearse = [
        "apply"
        "--rehearse"
      ];
      declines = [ "did-not-boot" ];
      needs = [
        { writable-file = "${machineDir}/flake.lock"; }
        { power = batteryFloor; }
        { free-space = roomForARebuild; }
      ];
      on-failure = "stop";
      enable = true;
    }

    # 2b. THE PARTS OF THE SYSTEM THAT WAS JUST APPLIED (`nix/modules/parts.nix`).
    #
    # What needs a download no public cache holds, what is not free and what
    # only the full profile carries is a part: never in the system's closure,
    # realized by its own unit, kept in its own profile. The switch above
    # already started their realizing (the trigger runs at every switch);
    # this step only brings the new plan into the records and ROOTS the
    # parts' derivations, synchronously — the collection below would take
    # them otherwise — and never waits for a download.
    #
    # `carry-on`, always (`realize-parts`'s law): a part that does not arrive
    # keeps its previous generation, and holds back nothing.
    {
      id = "parts-plan";
      does = "realize-parts";
      root = true;
      package = "parts";
      program = "ronoctua-parts";
      run = [ "plan" ];
      rehearse = [
        "plan"
        "--rehearse"
      ];
      declines = [ "busy" ];
      needs = [ { always = true; } ];
      on-failure = "carry-on";
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

    # 5. THE SYSTEM'S OLD GENERATIONS, BY COUNT — AND ONLY THE SYSTEM'S.
    #
    # It starts `ronoctua-system-generations.service` (`nix/modules/store.nix`),
    # which keeps the newest N, the profile's current one, the running system,
    # the BOOTED one, the one the last switch went AWAY from and every one the
    # boot menu lists — so the generation this sequence just switched away
    # from (even one the person had rolled back to, older than the newest N),
    # and the one whose kernel is known to boot, both stay: the way back is not
    # deleted in the same breath as the step forward. It touches no other
    # profile, and the person's own Nix history least of all.
    #
    # It was `nix-collect-garbage --delete-older-than 30d`, shipped off, and
    # that command is the thing this distro no longer runs anywhere: as root it
    # deletes the old generations of EVERY profile, the person's included.
    #
    # BEFORE the collection, so the collection frees what the trimmed
    # generations held. `carry-on`: a trim that failed leaves the garbage to
    # collect all the same.
    #
    # AND A REHEARSAL REHEARSES IT TOO (the owner's D5, 2026-09-30). The trim
    # is the one step whose rehearsal can say something the unit's name
    # cannot: WHICH generations would go. So the rehearsal starts the trim's
    # own rehearsal unit (`nix/modules/store.nix`), the same program with the
    # same profile, count and menus and without `--execute` — it reads the
    # profile and the boot menus for real, prints what it would remove, and
    # removes nothing. A trim that would fail (a menu it cannot read, a profile
    # that is not the system's) fails the rehearsal the same way.
    {
      id = "old-generations";
      does = "reclaim-space";
      root = true;
      unit = "ronoctua-system-generations.service";
      rehearsal = "ronoctua-system-generations-rehearsal.service";
      needs = [ { always = true; } ];
      on-failure = "carry-on";
      enable = true;
    }

    # 6. GARBAGE ONLY.
    #
    # This is `nix-collect-garbage` with no `--delete-older-than` and no `-d`,
    # and that is the whole decision: it deletes what nothing references —
    # never a generation, never anything a root reaches. What generations go
    # was decided by the step above, by count, and nowhere else. Between two
    # updates Nix's own floor (`min-free`, `nix/modules/store.nix`) collects the
    # same garbage whenever a build finds the disk short.
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

    # 7. AND THEN DEDUPLICATE WHAT IS LEFT.
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

    # 8b. WHAT IS DECLARED, BROUGHT IN — AND NOTHING ELSE.
    #
    # The rebuild put the person's declared programs in their profile; what
    # is left is what the system cannot do for them: the Android apps a
    # declaration lists (installed from the store's apk into the person's
    # Android), and the imperative copy a declared program now replaces (the
    # same program in the Nix profile, an AppImage, a web app's launcher)
    # taken out so the two do not shadow each other. `ronoctua-software
    # reconcile` touches only its own ledger
    # (`$XDG_STATE_HOME/ronoctua/software/declared-applied.json`): what it
    # did not put there, it does not take away. It reads what the machine
    # applied, so a machine with no report skips it and says so.
    #
    # FIRST of the person's half, so the upgrades after it move programs the
    # reconciliation already left in their final place.
    {
      id = "person-reconcile";
      does = "reconcile-declared";
      root = false;
      package = "software";
      program = "ronoctua-software";
      run = [ "reconcile" ];
      needs = [ { file = "/etc/ronoctua/person-applied.json"; } ];
      on-failure = "carry-on";
      enable = true;
    }

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

    # 19. FLATPAK RUNTIMES NO APP NEEDS ANY MORE, in the person's installation.
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

    # 20. THE NIX PROFILE'S OLD GENERATIONS — OFF, AND ON PURPOSE.
    #
    # `nix profile wipe-history` on the PERSON's own profile. The owner's
    # guarantee is that nothing automatic removes what the person's Nix relies
    # on, and a generation of their profile is exactly that: the way back to a
    # program they had. The one count limit he granted is on the SYSTEM's
    # generations (step 5), never on the person's. The software view still
    # offers the same tidy as a deliberate act, which is the person's to take;
    # and a machine whose person wants it every week turns it on in one line,
    # `ronoctua.update.steps.nix-profile-history = true;`.
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
      enable = false;
    }

    # 21. WHAT `appimage-run` UNPACKED for AppImages that are gone or were
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
    # THE 2026-10-05 WAVE: the person half's last steps (reconcile,
    # publish), and the machine directory's one name, after their anchors.
    # ── anchor: L03 ──
    # ── anchor: L19 ──

    # =================================================================
    # AND LAST, WHAT THIS MACHINE DID, PUBLISHED
    # =================================================================
    #
    # The person's other machines learn from the plain part of the
    # belongings what this one runs: its record (the last good system, the
    # boot trial, a hold, how the last apply ended) and whatever its
    # declarations became. Last, so what is pushed is what the run ended with.

    # P1. THE MACHINE'S RECORD, copied into the person's plain part as
    # `machines/<hostname>.json` — validated and written in its one
    # canonical form, and left alone when nothing changed (so an update that
    # changed nothing pushes nothing). Its rehearsal reads and compares the
    # same way and writes nothing.
    {
      id = "person-record";
      does = "publish";
      root = false;
      package = "person";
      program = "ronoctua-person";
      run = [ "record" ];
      rehearse = [
        "record"
        "--rehearse"
      ];
      needs = [ { directory = "$XDG_STATE_HOME/ronoctua/person"; } ];
      on-failure = "carry-on";
      enable = true;
    }

    # P2. THE PLAIN PART, PUSHED: no passphrase, the sealed part untouched; a
    # machine that pushed in the meantime is fetched and merged first, and a
    # clash is left as this machine has it (the next sync with the person
    # decides). A machine with no repository skips it and says so.
    {
      id = "person-publish";
      does = "publish";
      root = false;
      package = "belongings";
      program = "ronoctua-belongings";
      run = [
        "plain"
        "push"
        "--json"
      ];
      needs = [
        { file = "$XDG_CONFIG_HOME/ronoctua/belongings.json"; }
        { network = true; }
      ];
      on-failure = "carry-on";
      enable = true;
    }

    # P3. WHAT THIS MACHINE BUILT, PUSHED TO THE PERSON'S BINARY CACHE (the
    # 2026-10-11 wave) — their own Cachix cache, never the distro's, which
    # has none. Only where the person named one (the machine's file, which
    # `nix/modules/binary-cache.nix` writes from their validated options),
    # only after an update that SWITCHED the machine since the last push,
    # only with their token (sealed, `0600`) and with every host it reaches
    # let: the four are news, never a failure, and it never asks. A PUBLIC
    # cache receives the distro's own outputs and nothing of the person's,
    # never an unfree program; a private one the system's closure — and
    # neither what `cache.nixos.org` already serves (`ronoctua_cachix::push`).
    # Its rehearsal says what would go, and pushes nothing. The machine's
    # file is asked by the program and not by a `file` precondition: the
    # step's news says which of the four held it back, and a precondition
    # would say only that a file is missing.
    {
      id = "binary-cache-push";
      does = "publish";
      root = false;
      package = "binary-cache";
      program = "ronoctua-cachix";
      run = [ "push" ];
      rehearse = [
        "push"
        "--rehearse"
      ];
      declines = [
        "no-cache"
        "no-token"
        "not-let"
        "nothing-new"
      ];
      needs = [ { network = true; } ];
      on-failure = "carry-on";
      enable = true;
    }
  ];
}
