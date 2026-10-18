# The texts `ronoctua-shell-buffers` says, in en-GB (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

## The modeline's two left segments

# THE BUFFER'S NAME (the window title) and its MAJOR MODE (the kind of buffer).
# Both are addressed by an id that is ENGLISH and kebab-case (invariant 18); what
# reaches the screen is the value here. That separation is why `glifos` used to
# leak onto an English-language screen: the id WAS the label.
buffer-example = example
buffer-memory = memory
buffer-transfers = transfers
buffer-format = format
buffer-brain = brain

## The memory panel — LEADER s m

example-title = Several keyboards
example-note = Each keyboard moves its own selection and types into its own field, in its cursor's colour.
example-list = A list
example-form = A form
example-empty = empty
example-item-one = Morning
example-item-two = Noon
example-item-three = Afternoon
example-item-four = Evening
example-item-five = Night
example-item-six = Dawn
example-field-name = Name
example-field-note = Note
cmd-example-clear = Empty the form
cmd-memory-full = Full overview
cmd-memory-guard = Freeze protection
cmd-memory-tab = Switch RAM and disk
cmd-memory-scan-cancel = Stop the scan
cmd-memory-stop = Close what is focused
cmd-memory-sweep-store = Collect the store's garbage
cmd-memory-sweep-generations = Trim the system's old generations
cmd-memory-sweep-journal = Trim the journal
cmd-memory-tolerant = Loosen for this session
cmd-memory-default = Back to the default
cmd-memory-persist = Save as permanent
memory-title = Memory
memory-ram = RAM
memory-zram = zram
memory-swap = Swap
memory-pressure = Stall
memory-disk = Disk
memory-guard = Protection
memory-kernel = Kernel
memory-cached = File cache
memory-anonymous = Programs
memory-shared = Shared
memory-dirty = To be written
memory-desktop = Desktop
memory-apps = Applications
memory-system = System
memory-consumers = What is using it
memory-scanning = Scanning…
memory-nothing-yet = Nothing scanned yet
memory-asked = Request sent; the next reading shows the result.
memory-help-overview = m f full · m o protection · m s scan · m r refresh
memory-help-full = m v RAM/disk · m s scan · m k close · m g m n m j housekeeping
memory-help-guard = m t loosen · m d default · m p save · Esc goes back
memory-guard-description = When memory runs out, the machine can stop answering for minutes. This protection closes ONE application before that happens — never the desktop, which is outside the candidate set by construction.
memory-guard-detail = It measures how long things spend waiting on memory, not how many bytes are left: it acts only after a sustained stall, never after a spike.
memory-guard-armed = armed
memory-guard-off = not armed
memory-guard-unknown = cannot tell
memory-guard-strict = default
memory-guard-tolerant = tolerant
memory-guard-limit = Acts above
memory-guard-for = After
memory-guard-ceiling = Applications' ceiling
memory-guard-acted = Acted (this boot)
memory-guard-persisted = After a reboot
memory-guard-session-only = This change lasts until reboot. m p writes it into host.nix, and then a nixos-rebuild switch is still missing.
memory-guard-cost = In tolerant mode the machine may be slow for up to a minute before anything is closed.
memory-confirm-stop = Close what is focused? The same key again confirms; Esc cancels.
memory-confirm-sweep = Delete what is not in use? The same key again confirms; Esc cancels.
memory-confirm-persist = Write this profile into host.nix? The same key again confirms; Esc cancels.
memory-vram = Graphics
# WHY A DISK IS NOT ON OFFER TO FORMAT. The sentence is built on the host and
# crosses ready-made: the buffer has no language bundle (inv. 11).
format-needed-at = the system lives at { $at }
format-needed-fstab = /etc/fstab mounts this one at boot
format-needed-swap = it is the system's swap
format-refused-busy = { $name } is in use — close what is open on it and try again
format-refused-admin = { $name } belongs to the system, and permission cannot be asked for here
format-refused-other = could not format { $name }: { $why }
# THE FORMAT BUFFER — `LEADER s f`. Three questions, each answered with the
# arrows and Enter. The language is deliberately plain: the vfat row says what
# it is for, not what it is called. Somebody who knows it is FAT32 loses
# nothing; somebody who does not gets an answer instead of a word to look up.
format-title = Format a disk
format-empty = no disk here to format
format-hint-list = ↑↓ choose · Enter continue · Esc close
format-hint-choose = ↑↓ choose · type a name · Enter continue · Esc back
format-hint-confirm = ↑↓ choose · Enter answer · Esc back
format-whole = the whole disk
format-part = one part of the disk
format-blank = empty
format-choose-title = What will this disk be used for?
format-recipe-universal = opens on any computer — pendrive, camera, television
format-recipe-large = the same, but takes large files (over 4 GB)
format-recipe-windows = for use on Windows
format-recipe-linux = for use on Linux
format-recipe-modern = for Linux, keeping snapshots of the disk
format-recipe-missing = XFS is not here: this distro does not carry the program that writes it
format-name = name
format-name-limit = up to 11 letters or numbers
format-confirm-title = This erases everything on it
format-confirm-whole = the whole disk will be erased, with every part of it
format-confirm-part = this part of the disk will be erased
format-confirm-no = no, go back
format-confirm-yes = yes, erase and format
format-working = writing to the disk — do not switch off or pull it out
format-done = done
# THE DETAIL LINE — the same buffer answering a second person.
#
# The rows above are for somebody who does not know what FAT32 is and should not
# have to. These are for somebody who does: the real name, the limits, and what
# the gesture is going to do to the disk. One line, under the cursor, in the
# dimmest colour there is — a beginner never has to read it, and an expert never
# has to guess.
format-tech-universal = vfat (FAT32) · up to 4 GiB per file · no owner or permissions
format-tech-large = exFAT · no practical file size limit · no owner or permissions
format-tech-windows = NTFS · Windows owner and permissions · written by ntfs-3g
format-tech-linux = ext4 · Unix owner and permissions · journalled
format-tech-modern = Btrfs · subvolumes, snapshots, checksums and compression
format-tech-missing = XFS · mkfs.xfs is not installed in this distro
format-tech-part = only this partition is rewritten; the disk's table does not change
format-tech-whole = a fresh partition table (dos up to 2 TiB, gpt above) and ONE partition
format-tech-needed = mounted by the running system · udisks would refuse, and this screen does not even offer
format-tech-nothing = no disk under the cursor
transfers-answer = answer
# What the operation IS.
transfers-work-copy = copy
transfers-work-move = move
transfers-work-delete = delete
transfers-work-shred = overwrite
transfers-work-trash = move to the trash
transfers-work-restore = put back from the trash
transfers-work-compress = compress
transfers-work-extract = extract
transfers-work-measure = measure
# Where it is.
transfers-stage-weighing = weighing
transfers-stage-running = under way
transfers-stage-paused = held
transfers-stage-done = done
transfers-stage-failed = failed
transfers-stage-cancelled = given up
# Who moves it. For now only this machine's disk.
transfers-engine-local = disk
transfers-window-title = transfers
transfers-window-empty = nothing in the queue
# The engines still to come, and the network operations. They are here already
# because the buffer names each one as a constant — and a word with no
# translation fails the check instead of showing up on screen.
transfers-engine-fetch = link
transfers-engine-media = media
transfers-engine-torrent = torrent
transfers-engine-atlas = region
transfers-work-fetch = download
transfers-work-media = download video
transfers-work-audio = download audio
transfers-work-subtitles = download subtitles
transfers-work-torrent = torrent
# The person's own programs: an order of the software center, run by its own
# program. The row's first half is the program's NAME, so the work is a verb
# with no object of its own.
transfers-engine-software = programs
cmd-brain-run = Run it now
cmd-brain-rehearse = Rehearse it
cmd-brain-reset-steps = Reset steps
cmd-brain-interval = How often
cmd-brain-window = Change time
cmd-brain-switch = Switch it off
cmd-brain-keys = Every key
cmd-brain-log = Live log
cmd-brain-row-also = The row's other action
brain-nothing = Nothing is set up yet
brain-tasks = Tasks
brain-activity = Activity
brain-no-activity = Nothing has happened yet
brain-no-log = Nothing logged yet
brain-schedule = Schedule
brain-steps = Steps
brain-history = History
# WHAT THE HEADER TEACHES about the page's own keys. `cmd-*` covers the rest.
brain-do-open = Open it
brain-do-back = Back
brain-do-pane = Other half
brain-do-change = Change this
brain-do-keys = Every key
about-brain-do-open = Open the task under the cursor: its schedule, its steps and what it did the last times.
about-brain-do-pane = Take the keyboard to the other half of the page.
about-brain-do-change = Change the setting or the step under the cursor.
about-brain-do-back = Go back to the list of every task.
about-brain-keys = Open this line to see every key the page answers to, or close it again.
about-brain-run = Run this task now, whatever its schedule says.
about-brain-rehearse = Rehearse this task: every step is walked and nothing on the machine changes.
about-brain-switch = Switch this task off, or on again; switched off, it never runs by itself.
about-brain-interval = Step to the next cadence this task runs at.
about-brain-window = Step to the next part of the day this task may run in.
about-brain-reset-steps = Put every step of this task back as the distro ships it.
about-brain-proceed = Start the task that is counting down now, without waiting for the count.
about-brain-abort = Stop the task that is running or counting down.
about-brain-log = Show the brain's own log as it is written — every tick, what it decided, every warning and error — or go back.
about-brain-row-also = Do the other thing the row under the cursor offers: on an error not yet seen, ignore its type from now on.
# THE SCHEDULE'S ROWS. Each is a field's NAME, so each is a noun phrase — and
# the value beside it is the answer, never half of a sentence that started in
# the label.
brain-enabled = Runs on its own
brain-on = Yes
brain-off = No, only when you ask
brain-every = Runs every
brain-days = days
brain-part = Runs between
brain-next = Next run
brain-soon = Starts in
brain-run-now = Run it now
brain-rehearse-it = Rehearse it
brain-rehearsal = Rehearsing now
brain-never = Never
brain-flake-owned = In the flake
brain-changed = Changed by you
# THE FOOTER'S FIELDS.
brain-last-run = Last run
# THE TASKS. `brain-task-<id>` names one and `<id>-does` says what it does —
# the second is a paragraph in the footer, so it has room to be a whole
# explanation rather than a line.

# WHERE A TASK IS RIGHT NOW. A short phrase each: they sit in a column twelve
# cells wide on the home page.
brain-standing-idle = Waiting
brain-standing-queued = In the queue
brain-standing-running = Running
# AND HOW A RUN ENDED. `failed` is the honest word for a run that did not
# finish; whether that was a fault or a decision is the reason's job.
brain-outcome-skipped = Did not run
brain-outcome-done = Worked
brain-outcome-failed = Failed
brain-outcome-held = Waited
# WHAT IS IN THE WAY, in the column beside a task's name.
brain-hold-ram = Memory busy
brain-hold-cpu = Processor busy
brain-hold-battery = On battery
brain-hold-busy = One at a time
brain-hold-network = No network
brain-hold-asleep = Asleep
# AND THE SAME THING AS A WARNING, on the line under the header — the one
# place on this screen that is allowed to alarm, and only while something is
# actually in the way.
#
# A WHOLE SENTENCE PER REASON and not one sentence with the reason put into it:
# a `Run` carries a key and nothing else (invariant 11). It is also the better
# of the two — "the processor is too busy" says the thing, "held: cpu" makes
# somebody guess.
brain-held-ram = Nothing will start while this much of the memory is in use. It goes on by itself when the machine is quieter.
brain-held-cpu = Nothing will start while the processor is this busy. It goes on by itself when the machine is quieter.
brain-held-battery = Nothing will start on battery. Plug the machine in, or change that in brain.json.
brain-held-other = Something is in the way. The task it is holding says what.
brain-why-ram = The memory was busy
# WHICH PART OF THE DAY a task may start in. The hours are the same digits in
# every language; the words around them are not.
brain-part-any = any hour
brain-part-night = 02:00 and 05:00
brain-part-morning = 08:00 and 12:00
brain-part-afternoon = 13:00 and 18:00
brain-part-evening = 19:00 and 23:00

## Bluetooth — the radio, the devices and what each one can do (LEADER s b)

# THE KEYBOARDS — what a key can be made into, and what went wrong.
keyboard-trouble-file = the file of keyboard rules could not be read or written
keyboard-trouble-no-place = this machine has no place for keyboard rules — they are written on the installed system
keyboard-trouble-refused = keyd would not take these rules, so nothing changed
keyboard-trouble-restart = the rules were written, but keyd did not restart
keyboard-trouble-mode = the keyboards could not switch, so they stay as they were
keyboard-mode-together = together, as one
keyboard-mode-independent = each on its own
keyboard-independent-note = Each keyboard types through its own cursor, with its own layout, keys and leader.
keyboard-unit = One keyboard
keyboard-unplugged = not plugged in
keyboard-cursor = cursor
keyboard-types-through = Types through cursor
keyboard-layout = Layout
keyboard-variant = Variant
keyboard-leader-key = Leader key
keyboard-repeat-rate = Repeat rate
keyboard-repeat-delay = Repeat delay
keyboard-machines = the machine's
keyboard-compositors = the compositor's

## Screens — the monitors, their pictures and their light (LEADER s d)

# THE KEYBOARDS (`LEADER s k`) — which ones this machine has, and what each key
# on each one means. Remapping is per keyboard because keyd matches devices;
# the layout and the repeat rate are the compositor's, one for all of them.
buffer-keyboard = Keyboards
cmd-keyboard-next = Keyboards: the next row
cmd-keyboard-previous = Keyboards: the row before
cmd-keyboard-list = Keyboards: back to the list
keyboard-title = Keyboards
keyboard-one = What each key does
keyboard-none = the kernel is reporting no keyboard at all
keyboard-built-in = part of this machine
keyboard-attached = plugged or paired in
keyboard-changed = changed
keyboard-untouched = nothing changed
keyboard-becomes = does
keyboard-the-key = the key
keyboard-unchanged = itself
keyboard-holds = held one way, tapped the other
keyboard-global = The layout, the compose key and the repeat rate are the compositor's — one of each, for every keyboard.
keyboard-same-model = Two keyboards of this model answer to one rule: keyd tells them apart by make and model, and these share both.
keyboard-is-the-leader = This is the leader. Moving it takes the key that opens everything, on this keyboard.
about-transfers-answer = Take the answer the cursor is on.
about-brain-act = Do what the notice in front offers: the first action of the module that raised it.
about-brain-dismiss = Put the notice in front away. It comes back only if its module raises it again.
about-brain-snooze = Put the notice in front off, for as long as its first snooze says.
brain-outcome-noted = Noted
brain-notices = Notices
brain-actions = What it offers
brain-watching = Watching
brain-activity-last = Last:
brain-activity-first = First
brain-activity-times = Times
brain-since = Since

## file info editing — everything the info panel can change

transfers-work-archive-add = add to the archive
transfers-work-archive-remove = take out of the archive
transfers-work-archive-test = test the archive
transfers-work-archive-convert = convert the archive
format-encrypt-title = Encrypt it?
format-encrypt-no = no — whoever holds the disk can read it
format-encrypt-yes = yes — it opens only with a passphrase
format-tech-encrypt-no = the filesystem is written in the clear
format-tech-encrypt-yes = LUKS2 · the passphrase is asked twice, on this window's own line
format-encrypt-differ = the two passphrases differ; nothing was written
format-encrypt-let-go = no passphrase was given; nothing was written
format-encrypted = encrypted
ask-format-encrypt-passphrase = A passphrase for the disk
ask-format-encrypt-again = The same passphrase again
transfers-engine-scan = VirusTotal
# The queue's words for the containers' jobs (`ronoctua-transfers`).
transfers-engine-container = containers
# The transfer queue's words for a sync.
transfers-engine-belongings = belongings
example-code = A piece of code
transfers-engine-incus = Incus
transfers-work-incus = Incus
transfers-engine-stick = installer stick
transfers-work-stick-write = write the installer to a stick

## THE SECURITY CENTRE'S KEYS, and the passwordless window's count.

transfers-engine-seal = encryption
transfers-work-seal = encrypt
transfers-work-unseal = decrypt
