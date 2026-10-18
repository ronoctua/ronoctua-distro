# The texts `ronoctua-shell-stick` says, in en-GB (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

group-stick = installer stick
cmd-stick-open = The installer on a USB stick: the latest version and your drives
cmd-stick-window = The installer on a USB stick, in a window
cmd-stick-down = The next row
cmd-stick-up = The previous row
cmd-stick-enter = Does what the row says: allow, download, choose the drive, answer
cmd-stick-reread = Asks GitHub for the latest version again, and reads the drives again when udisks did not answer
stick-down = down
stick-up = up
stick-enter = choose
stick-reread = ask again
stick-title = Installer stick
stick-heading-release = The installer
stick-heading-drives = Drives
stick-heading-write = The last write
stick-consent-for = Knowing the latest version and downloading the installer needs GitHub.
stick-asking = Asking GitHub for the latest version…
stick-retry = Ask again
stick-version = Version
stick-published = Published
stick-size = Size
stick-image = Image
stick-image-cached = downloaded and checked, in the cache
stick-image-absent = not downloaded yet
stick-fetch = Download only
stick-fetch-value = the image, to the cache, without writing a drive
stick-drives-reading = Reading the drives…
stick-drives-down = udisks is not answering, so the drives cannot be read.
stick-drives-none = No USB stick or memory card is plugged in.
stick-drive-ready = { $size } · can be written
stick-drive-small = { $size } · too small for this version
stick-erases = Writing erases the whole drive. You are asked first, and then for your password.
stick-unfit-bus = { $size } · not on USB nor a memory card
stick-unfit-system = { $size } · the system runs from it
stick-unfit-mounted = { $size } · mounted
stick-unfit-open = { $size } · an encrypted volume is open
stick-unfit-read-only = { $size } · read-only
stick-unfit-unnamed = { $size } · no stable name
stick-why-bus = Only a USB stick or a memory card is written: this drive is on another bus.
stick-why-system = The running system uses a volume on this drive, so it is never written.
stick-why-mounted = A volume on this drive is mounted. Unmount it in the file manager, then choose it again.
stick-why-open = An encrypted volume on this drive is open. Lock it in the file manager, then choose it again.
stick-why-read-only = The drive is read-only: a stick's or a card's switch may be set.
stick-why-unnamed = The drive has no stable name under /dev/disk/by-id, so a write could not be pinned to it.
stick-small-why = The installer does not fit on this drive. Choose a bigger one.
stick-no-release-yet = The latest version is not known yet, so there is nothing to write.
stick-sure = Write the installer { $version } to { $drive } ({ $size })?
stick-sure-note = Everything on the drive is erased, and that cannot be undone.
stick-no = No, leave the drive as it is
stick-yes = Yes, erase it and write the installer
stick-state = State
stick-phase-waiting = waiting for the download to finish
stick-phase-writing = writing: { $percent }%
stick-phase-verifying = reading it back: { $percent }%
stick-phase-done = written and read back: the stick is ready
stick-phase-failed = refused
stick-phase-given-up = given up
stick-refused-not-let = GitHub was not allowed, so nothing was asked.
stick-refused-offline = GitHub did not answer. Are you online?
stick-refused-download = The download did not finish, so nothing was written: GitHub stopped answering, or the cache ran out of room. The queue's row says which.
stick-refused-no-release = The latest release has no installer this view understands.
stick-refused-mismatch = What was downloaded does not match the release's checksums, so nothing was written.
stick-refused-not-writable = The drive could not be written: it was mounted, held, too small, or is not a stick.
stick-refused-changed = The drive changed after it was chosen, so nothing was written. Choose it again.
stick-refused-read-back-differs = The stick does not hold what was written to it. It may be failing, or smaller than it says.
stick-refused-failed = The write did not happen: the queue or the writer failed. The queue's row, or the journal of ronoctua-stick-write.service, says why.
stick-about-allow = Lets the three GitHub hosts, so the latest version can be asked for and downloaded
stick-about-retry = Asks GitHub for the latest version again
stick-about-fetch = Downloads and checks the image now, so a write later starts at once
stick-about-drive = Writes the installer over this whole drive, after a question
stick-about-unfit = Says why this drive is not written, and what to do
stick-about-no = Closes the question and writes nothing
stick-about-yes = Starts the writer with your password, then downloads, checks and writes
stick-drive-left = The drive went away, so the question was closed.
stick-queued-fetch = The image is downloading in the queue.
stick-queued-write = The writer is waiting: the image downloads in the queue, then the drive is written and read back.
stick-starting = Starting the writer…
stick-not-let-host = { $host } is not allowed: allow GitHub in this view first.
stick-busy-writing = A stick is being written now. Wait for it to finish (the last write, below) before writing another.
stick-busy-waiting = A write is already waiting for its image to download. Give it up in the queue to choose another drive.
stick-busy-stopped = A writer was still waiting for an image nobody was downloading; it was told to stop. Choose the drive again in a moment.
stick-no-writer = This machine has no stick writer installed.
stick-no-cache = There is no cache folder to download the image into.
