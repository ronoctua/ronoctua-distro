# What a cache is worth keeping, in one place.
#
# THE RULE THIS FILE EXISTS FOR: nothing cached stays for ever. The owner said
# it in those words — *"Todos os caches devem ter limite, jamais podem ficar no
# sistema para sempre"* — after asking how this distro's cache worked and being
# told the honest answer, which was that neither of the two on disk was ever
# swept. No timer, no tmpfiles rule, no walk anywhere in the tree.
#
# Measured on the owner's own machine the day this was written, because the
# numbers below are only defensible against a real one:
#
#   ~/.cache                       254 GB of `ronoctua-distro/target` alone,
#                                  36 GB of `yay`, 2 GB of Chromium
#   our two thumbnail caches       4,6 MB (ours) + 451 MB (the shared one)
#   files unread for 20 days       101 894, and 148 GiB
#
# The thumbnails were never the problem. That is why the sweep's reach is the
# whole of `$XDG_CACHE_HOME` and not a directory of ours: a rule that only
# governs what we wrote would have freed four megabytes.
#
# FOUR NUMBERS AND ONE PLACE. Two of them reach Rust (the brain's schedule and
# the file manager's renewal), two reach a systemd unit's arguments, and one —
# `freshDays` — has to stay smaller than another, which is a relationship that
# cannot survive being written twice. `nix/cache.nix` refuses the build if it
# does not hold.
{
  # HOW OLD A CACHED FILE MAY GET BEFORE IT IS SWEPT, in days.
  #
  # Counted from its LAST USE and not from its last write — see `nix/cache.nix`
  # for why the sweep takes the later of the two timestamps, and what a machine
  # mounted `noatime` would otherwise lose.
  #
  # Twenty is the owner's number and it is a good one: long enough that a
  # project picked up again after a fortnight still finds its build cache, short
  # enough that a hundred gigabytes of dead artefacts do not sit there for a
  # year. Measured at twenty on this machine: 148 GiB would go, and the oldest
  # thing still reachable in the browser's cache would be three weeks old.
  keepDays = 20;

  # HOW OFTEN THE SWEEP HAPPENS, in days.
  #
  # Five. It is not a fraction of `keepDays` on purpose: the sweep is cheap
  # (a walk and some `unlink`s, at idle priority) and running it more often
  # only means each run has less to do. Once a month would mean a month's
  # worth of garbage arriving in one night.
  everyDays = 5;

  # WHEN IN THE DAY, as one of the brain's named windows.
  #
  # `night` is 02:00 to 05:00 (`PartOfDay::Night`), which is where the owner's
  # "four in the morning" falls. It is a WINDOW and not an hour because that is
  # the vocabulary the brain already has, and its own file says why: an hour is
  # a decision about a machine and `at night` is a decision about a life.
  #
  # A machine that is asleep at four does not lose its turn — see the overdue
  # rule in `crates/ronoctua-brain/src/schedule.rs`.
  partOfDay = "night";

  # WHEN THE FILE MANAGER REMAKES A THUMBNAIL IT ALREADY HAS, in days.
  #
  # The grid draws what is in the cache AT ONCE, and then, for a picture older
  # than this, quietly decodes it again behind the one on screen and replaces
  # it. Nobody waits for anything.
  #
  # WHY IT EXISTS AT ALL, and the reason is a defect this repository met: the
  # rule for drawing a sound with no cover art changed, the code was right, and
  # the owner went on seeing waveforms — the pictures were already on disk under
  # a key that says which file and what size and nothing about what was drawn
  # into them. A renewal means every change of rule reaches the screen by
  # itself, within this many days, without anybody remembering to invalidate
  # anything.
  #
  # TEN, AND IT MUST STAY BELOW `keepDays`. That is the whole relationship: a
  # thumbnail one actually looks at is renewed at ten and so never reaches
  # twenty, and the sweep only ever takes pictures of folders nobody opens.
  freshDays = 10;
}
