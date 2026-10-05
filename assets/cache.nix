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
# does not hold. The disk's own numbers came later, at the end of this file,
# for the same reason: Nix and Rust read them both, and they hold each other.
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

  # =======================================================================
  # THE DISK LOOKS AFTER ITSELF
  # =======================================================================
  #
  # The owner's words (2026-09-28): the distro handles the disk "sem precisar
  # de intervenções minhas", and "a solução deve ser global (deve servir para
  # outros computadores que também forem instalar essa distro)". So every size
  # below is a PERCENTAGE of the disk of whoever installs, never a number of
  # gigabytes measured on one laptop. And one guarantee rides with them:
  # nothing here removes what the person's Nix relies on. The single exception
  # is `systemGenerations`, and it always keeps the current, the running and
  # the booted system.

  # BELOW THIS MUCH FREE, SPACE IS MADE, in per cent of the filesystem.
  #
  # Nix's own `min-free` (`nix/modules/store.nix` writes it at boot, from this
  # and the size of the disk under `/nix/store`): a build or a download that
  # sees less free space collects garbage — only garbage, never a generation —
  # until `diskGoalPercent` is free. Ten per cent is 25 GB of a 256 GB disk and
  # 200 GB of a 2 TB one; both are more than one rebuild writes.
  diskFloorPercent = 10;

  # AND UNTIL THIS MUCH IS FREE, in per cent. Above the floor on purpose: a
  # collection that stopped AT the floor would start again at the next build.
  diskGoalPercent = 15;

  # AND NEVER MORE THAN THIS, in GiB — the floor's cap and the goal's.
  #
  # A share alone misfires on a big disk: ten per cent of 8 TB is 800 GB, so a
  # disk that holds mostly the person's own files would count as short with
  # hundreds of gigabytes free — Nix would collect garbage at every build, and
  # the brain would make room for nothing, again and again. journald caps its
  # own percentages the same way. Fifty is more than any rebuild writes; the
  # goal's cap stays above the floor's for the goal's own reason.
  diskFloorCapGiB = 50;
  diskGoalCapGiB = 75;

  # HOW MANY SYSTEM GENERATIONS STAY, counted from the newest.
  #
  # The one number here that removes something alive, and the owner's one
  # exception: only the SYSTEM profile (`/nix/var/nix/profiles/system`), and
  # the current, the running and the booted generation always stay on top of
  # these. The person's own profiles are never touched. Five, because the
  # update is weekly: about the month the old `--delete-older-than 30d` kept.
  # Never below two — the rescue console's rollback needs the previous one.
  # The boot menu lists at most this many, so every entry keeps its generation.
  systemGenerations = 5;

  # HOW LONG A BUILD LOG STAYS, in days (`/nix/var/log/nix/drvs`).
  #
  # Nothing reads a month-old build log, and nothing ever removed one. Thirty
  # is long enough to answer "why did last week's rebuild fail?".
  buildLogDays = 30;

  # THE MOST ALL CARGO BUILD DIRECTORIES UNDER THE CACHE MAY WEIGH, in per
  # cent of their filesystem. It has to stay below what the goal leaves
  # (`100 - diskGoalPercent`), or the cargo tree alone could hold the disk
  # under the floor.
  cargoCeilingPercent = 10;

  # A BUILD BY A COMPILER THAT IS NOT THE NEWEST, untouched this many days,
  # goes whole. Below `keepDays`: an old compiler's build is dead weight long
  # before an ordinary cache entry is.
  oldCompilerDays = 7;

  # A RELIEF WAITS THIS LONG after the last one before another, in minutes —
  # a rehearsal counts. An hour at least, the brain's activity-log rule: a
  # machine short of disk must not spend it on a relief every heartbeat. And
  # one that left the disk still short waits, besides, until the disk has
  # filled further: until then nothing new is there to free.
  reliefGapMinutes = 60;

  # BELOW THIS MUCH FREE, THE DISK IS NEARLY FULL, in per cent — and never
  # more than `diskWarnCapGiB`. While any filesystem the relief measures is
  # under it, the brain keeps a mark on the modeline (the owner asked for
  # exactly this, 2026-09-30).
  #
  # BELOW THE FLOOR, on purpose: under the floor the relief makes room by
  # itself, and a mark there would come and go with every relief that worked
  # — a modeline that speaks while nothing is asked of anybody. Under this
  # line the relief has done what it could, or is racing something faster
  # than it, and what fills the disk is the person's to choose. Five per cent
  # is 12 GB of a 256 GB disk; the cap keeps a big disk from being "nearly
  # full" with a hundred gigabytes free.
  diskWarnPercent = 5;
  diskWarnCapGiB = 20;

  # =======================================================================
  # THE PLAYER'S SUBTITLES, READ ONCE
  # =======================================================================
  #
  # THE MOST THE PLAYER'S DAEMON KEEPS OF THE SUBTITLE TRACKS IT READ, in MiB
  # of MEMORY and not of disk (`crates/ronoctua-player-engine/src/engine/
  # subtitles.rs`). A track inside a film is converted by ffmpeg, which reads
  # the WHOLE container to find it — seconds for a long film — and every lane
  # change merges the lanes again; so what was read is kept under its stamp
  # (the file, its size, when it last changed, the track) and read again only
  # when the file changes. Past this, the track read least recently goes
  # first. Counted in the bytes each came from (the file, or ffmpeg's ASS).
  #
  # Thirty-two: a film's dialogue is a hundred kilobytes, the heaviest typeset
  # ASS a few megabytes, and six lanes over four pictures is twenty-four
  # tracks — all of them, with room. The daemon dies with the session, and
  # nothing of this outlives it.
  playerSubtitlesMib = 32;

  # =======================================================================
  # THE PLAYER'S CACHE ON DISK
  # =======================================================================
  #
  # THE MOST THE PLAYER KEEPS IN ITS CACHE DIRECTORY, in MiB
  # (`$XDG_CACHE_HOME/ronoctua/player`, `ronoctua_broadcast::cache`): the
  # covers taken out of the files that play (MPRIS is handed a file, and a
  # picture inside a FLAC is not one), and what the directories answered —
  # the radio lists, the TV index, a theme's pages of films and its lives
  # (`engine/directory.rs`), each kept as long as `assets/channels.nix` says
  # it stays fresh and shown at once while a fresher one is asked.
  # Written by the player's daemon alone; past this, the file READ longest ago
  # goes first, after every write.
  #
  # ITS OWN CEILING, because the sweep above never reaches it: the sweep takes
  # whole entries of the cache's root, each as old as its youngest file, and
  # `~/.cache/ronoctua` is touched every day.
  #
  # Sixty-four: a cover is a few hundred kilobytes (a 3000 px JPEG, two or
  # three megabytes), the TV index about three, a country's radio list a
  # quarter of one — an album's worth of covers and every list, with room.
  playerMib = 64;

  # THE TRANSLATIONS A PROCESS KEEPS, in entries (`ronoctua_words::cache`):
  # the player's daemon translating the lyrics of the songs that come round
  # again, and the footer's search asking the same phrase as it is refined.
  # In memory, and each entry is a song's lyrics at most (MyMemory's ceiling is
  # five kilobytes of text a translation) — so the bound is about a megabyte,
  # in a process that lives for months. Past it, the entry used longest ago
  # goes. Asking again is not free: MyMemory counts the day's characters and
  # DeepL the month's.
  wordsCacheEntries = 256;

  # THE EXCHANGE RATES' AGE, in hours, before a currency typed into the
  # calculator asks for fresh ones (`$XDG_CACHE_HOME/ronoctua/rates.xml`,
  # `ronoctua_calc::rates`). The ECB publishes once a working day, in the
  # afternoon: twelve hours means a rate is at most a day behind, and a
  # calculator used all day asks at most twice.
  ratesStaleHours = 12;

  # =======================================================================
  # THE PLAYER'S CATALOGUE
  # =======================================================================
  #
  # THE MOST THE PLAYER'S LOCAL CATALOGUE MAY HOLD, in MiB: its two SQLite
  # files together (`$XDG_CACHE_HOME/ronoctua/player-catalog/`,
  # `ronoctua_broadcast::catalog`), every source's categories and items. The
  # catalogue enforces it itself — an ingest that would pass it is rolled back
  # and the source keeps its old rows; a catalogue found over it is emptied —
  # and it is apart from `playerMib`: a database is not a folder of files the
  # least recently read of which can go, and the LRU there would cut it in
  # half.
  #
  # Two hundred and fifty-six. Every listing is answered by an index that
  # carries what it sorts and filters by, so an item costs about 1.8 KiB
  # (measured: a provider's fifty thousand channels in two hundred groups,
  # seven in ten filed under a theme, take 90 MiB; the timing test's
  # catalogue — every item in three categories and six theme lists — 231
  # MiB for fifty-two thousand). So this is two big IPTV providers and every
  # default source at their bounds, with room.
  playerCatalogMib = 256;

  # THE VIDEO-METRICS THUMBNAILS, in MiB
  # (`$XDG_CACHE_HOME/ronoctua/video-metrics/thumbs`,
  # `ronoctua_video_metrics::thumbs`): YouTube's `mqdefault` picture of each
  # catalogued video, fetched by the scanner alone. Past this, the picture
  # written longest ago goes first and the catalogue forgets it; the next scan
  # fetches it again only for the best two hundred.
  #
  # ITS OWN CEILING, for the player's reason above: `~/.cache/ronoctua` is
  # touched every day, so the sweep never reaches what is under it.
  #
  # A hundred and twenty-eight: a thumbnail is ten to thirty kilobytes, so
  # this holds seven to eight thousand — every video of a few dozen channels'
  # months, and the best two hundred forty times over.
  videoMetricsThumbsMib = 128;

  # =======================================================================
  # THE SUPER INDEX OF FAST-ACCESS
  # =======================================================================
  #
  # `$XDG_CACHE_HOME/ronoctua/index`, written by `ronoctua-index.service`
  # alone (`nix/modules/index.nix`); these reach it as the unit's arguments
  # (`nix/index-args.nix`), never as constants in the Rust.
  #
  # THE MOST THE INDEX FOLDER MAY WEIGH, in MiB. Past it the CONTENTS of the
  # oldest files go first — the names stay, which is what the index is for.
  # A million names with a tenth of them read is 170 MiB (the bench's own
  # synthetic home); a thousand is room for a home five times that with its
  # documents read.
  searchIndexMiB = 1024;

  # A FILE LARGER THAN THIS, in MiB, is found by its name only: its text is
  # not read at all. A book or a thesis is a few MiB; past sixteen it is a
  # log or a dump, whose words are not how anybody looks for it.
  searchContentFileMiB = 16;

  # THE MOST TEXT KEPT OF ONE FILE, in KiB: what fast-access finds a file by
  # when the name does not say it. Half a megabyte is a long book's worth of
  # words; the rest of it adds postings, not answers.
  searchContentKiB = 512;

  # NEVER WALKED: a bare word is a NAME anywhere in a root, a word starting
  # with `/` a PATH. Build trees and caches are machine output a person does
  # not look for by name, and they churn: `target` alone rewrites thousands of
  # files a build, each an inotify event and a document.
  searchSkip = [
    ".git"
    "target"
    "node_modules"
    ".cache"
    "/nix"
  ];

  # THE UNIT'S MEMORY, in MiB (`MemoryMax`): the writer's arena is an eighth
  # of it, and the rest is the walk's notes (eight bytes a path), a document
  # being read (a sixteen-MiB `.docx` parsed) and tantivy's merges.
  searchMemoryMiB = 512;

  # THE GIT BROWSER'S OLD VERSIONS (`C-o`): a file as a commit had it, copied
  # read-only under `~/.cache/ronoctua/git-versions` to be opened — the most
  # copies kept, the oldest going first. A copy is opened once and read; a
  # few dozen is every version somebody compares in a sitting.
  gitVersionsMost = 32;

  # THE ARCHIVE LISTINGS A PROCESS KEEPS, in listings. Walking into an archive
  # lists it with its program (7zz, bsdtar, unrar), which on a big one is
  # seconds; kept, going up and back in again is free. Keyed by the archive's
  # path, size and modification time, so one rewritten in place is listed
  # again. Eight is the archives one goes back and forth between; each holds
  # the members' names and sizes, not their contents.
  archiveListings = 8;

  # WHAT THE MEMBERS OPENED OUT OF ARCHIVES MAY WEIGH, in MiB
  # (`~/.cache/ronoctua/archive`). Opening a member extracts it there first,
  # and after every extraction the oldest archive's folder goes until the rest
  # fit — the one just written is kept whole. The cache's age sweeps it too.
  archiveOpenedMib = 1024;

  # THE CALCULATOR'S HISTORY (`calc.json`), in answers kept: the newest this
  # many, and the oldest goes as a new one comes. Two hundred is weeks of sums
  # a person scrolls back through; past it a history is a log nobody reads,
  # and every door that writes the file rewrites all of it.
  calcHistoryEntries = 200;

  # THE CATALOGUE'S POSTERS, in MiB (`player-catalog/posters/`): a poster is
  # fetched only for an item a door shows, from a source whose posters switch
  # is on, and is a picture of a few tens of KiB — 64 MiB is a couple of
  # thousand, more than a person scrolls past in a week. Apart from
  # `playerCatalogMib` (a folder of files the least read of which leave first,
  # never a database cut in half) and from `playerMib` (the covers and the
  # directories' lists, which a scroll through posters would push out).
  playerPostersMib = 64;

  # THE PICTURES OF YOUTUBE'S RESULTS (`/y`), in MiB
  # (`~/.cache/ronoctua/fast-access/thumbs`): fetched from `i.ytimg.com` only
  # for a video a search shows, with the person's consent, each a few tens of
  # KiB — 64 MiB is a couple of thousand, weeks of searches. The oldest go
  # first.
  webThumbsMiB = 64;

  # WHAT FAST-ACCESS REMEMBERS OF WHAT WAS OPENED FROM IT (`/r`, and the
  # weight the index gives a result used before), in entries: the newest this
  # many. Two hundred is months of what one person opens by searching.
  fastAccessRecentMost = 200;

}
