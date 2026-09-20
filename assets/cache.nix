# What a cache is worth keeping, in one place.
#
# WHAT THE SWEEP LOOKS AT: the entries in the ROOT of the cache, each one whole.
#
# It walked every file and weighed each on its own, and the owner stopped it for
# a reason worth writing down: a cache is rarely a heap of independent files. It
# is an index and the things the index names, a manifest and its blobs, a
# database and its write-ahead log. Take the ones nobody read lately and leave
# the rest, and you have not freed a cache — you have left a program holding
# half of something, which is a class of failure nobody can debug from a
# directory listing.
#
# Nearly every program here keeps its cache in a directory of its own under the
# root, and that is what makes the coarse rule the SAFE one: the unit of
# deletion becomes the unit the program created. A directory is as old as its
# YOUNGEST file, so a cache anything still touches is never taken, however much
# of it is ancient.
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
#   whole root entries unused      52 directories and 2 files, 4,2 GiB
#     for 40 days
#
# The thumbnails were never the problem. That is why the sweep's reach is the
# whole of `$XDG_CACHE_HOME` and not a directory of ours: a rule that only
# governs what we wrote would have freed four megabytes.
#
# AND THE COARSE RULE FREES FAR LESS, which is the price of it being safe: 4,2
# GiB against the 150,3 the file-by-file rule would have taken. The difference
# is almost entirely the two biggest caches on the machine — a cargo tree and a
# package manager's downloads — which hold a hundred gigabytes of dead
# artefacts AND something touched this week, so they stay. That is the trade
# the owner asked for, said out loud: a program is never left holding half of
# its own cache.
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
  # FORTY, and it went up from twenty when the sweep started taking whole
  # DIRECTORIES: the age of a directory is its youngest file's, so the number
  # now governs "nothing in here has been touched in six weeks" rather than
  # "this one file is old". Measured on this machine at both — 150,3 GiB of
  # individual files at twenty days, 76,2 GiB of whole directories at forty.
  # Less taken, and nothing taken out from under a program still using the rest.
  keepDays = 40;

  # HOW OFTEN THE SWEEP HAPPENS, in days.
  #
  # Ten. It is not a fraction of `keepDays` on purpose: the sweep is cheap — a
  # walk and some `unlink`s at idle priority — and running it more often only
  # means each run has less to do. Once a month would mean a month's worth of
  # rubbish arriving in one night.
  everyDays = 10;

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

  # HOW LONG SOMETHING STAYS IN THE TRASH, in days.
  #
  # FOUR, and the owner asked for it in those words. The trash is NOT a cache —
  # what is in there is the only copy, which is why the sweep above does not go
  # near `$XDG_DATA_HOME` — so this is the one number here that decides
  # something irreversible, and it is small on purpose: four days is long enough
  # to notice a mistake and short enough that a bin nobody empties stops being a
  # second disk.
  #
  # AND IT IS COUNTED FROM `DeletionDate`, which the freedesktop spec already
  # writes into every `.trashinfo`. Not from the file's own timestamps: a
  # document last written a year ago and thrown away this morning has been in
  # the trash for a morning, and any rule reading its mtime would delete it
  # before the person had finished regretting it.
  trashDays = 4;

  # AND HOW OFTEN THAT IS CHECKED, in days.
  #
  # The same four. Here the two numbers SHOULD match, unlike the cache's: a file
  # is removed as soon as it is eligible, so a longer cadence would only mean
  # things living up to twice as long as the number a person reads.
  trashEveryDays = 4;
}
