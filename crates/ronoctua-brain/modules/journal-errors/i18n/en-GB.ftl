# The journal-errors module's own texts: its name and the line under it, its
# page, the hint of its mark, its actions and the words it refuses with.
brain-task-journal-errors = Errors in the journal
brain-task-journal-errors-does = Collects the errors the system and your programs write in the journal, and groups them: the same failure with different numbers becomes one row, with a count. A mark on the modeline counts the kinds you have not seen yet, and warnings stay apart, off the count. A piece of text you add as a filter hides the lines that contain it, and takes them off the count.
brain-task-journal-errors-brief = Groups the errors written in the system's journal and counts the ones you have not seen.
journal-errors-heading-journal = Following the journal
journal-errors-heading-run = Reading by hand
journal-errors-heading-unseen = Not seen yet
journal-errors-heading-earlier = Not seen yet, from an earlier boot
journal-errors-heading-warnings = Latest warnings
journal-errors-heading-seen = Seen
journal-errors-heading-ignored = Ignored
journal-errors-heading-filtered = Filters, and what they hid this boot
journal-errors-heading-filtered-idle = Filters that caught nothing this boot
journal-errors-about-section-journal = The journal cannot be followed right now, so the list below may be behind.
journal-errors-about-section-run = While this is off, nothing reads the journal: run it to catch up from where it stopped.
journal-errors-about-section-unseen = What went wrong this boot and you have not seen. Choose one to mark it seen; its other key ignores the kind.
journal-errors-about-section-earlier = What went wrong in an earlier boot and you have not seen. Choose one to mark it seen; its other key ignores the kind.
journal-errors-about-section-warnings = The latest plain warnings, kept apart and off the count. Choose one to ignore its kind; its other key filters by its line. The number is how many kinds.
journal-errors-about-section-seen = What you have seen. Choose one to ignore its kind from now on; its other key filters by its line.
journal-errors-about-section-ignored = Kinds you ignore, with how many times each was written since. Choose one to hear it again.
journal-errors-about-section-filtered = Filters that hid something this boot, your own, and the shipped ones you turned off. Choose one to turn it off, or on again; its other key lets you say why it exists. The number is how many lines were hidden this boot.
journal-errors-about-section-filtered-idle = Filters that ship with the system and hid nothing this boot. Choose one to turn it off; its other key lets you say why it exists.
journal-errors-legend-critical = Critical
journal-errors-legend-critical-about = A crash, or something the system itself calls critical.
journal-errors-legend-error = Error
journal-errors-legend-error-about = Something failed and said so in the journal.
journal-errors-legend-notable = Worth a look
journal-errors-legend-notable-about = A warning that says something ended badly, such as a unit that failed or a disk in trouble, or a password typed wrong. It counts on the mark.
journal-errors-legend-warning = Warning
journal-errors-legend-warning-about = A plain warning, kept in its own section and off the count.
journal-errors-fact-severity = Severity
journal-errors-fact-first = First seen
journal-errors-fact-this-boot = This boot
journal-errors-fact-whole = Whole message
journal-errors-fact-last-caught = Last line caught
journal-errors-row-none = No error in the journal from this boot, or from where this stopped
journal-errors-row-off = Off, so nothing is being read: run it by hand from the options
journal-errors-row-stopped = Stopped since
journal-errors-row-last-run = Last read, lines listed
journal-errors-run-reading = Reading…
journal-errors-run-from-boot = The start of this boot
journal-errors-run-nothing = Nothing new
journal-errors-run-cut = Too much to read: only the latest lines were taken
journal-errors-mark-unseen = { $count ->
        [one] One kind of problem in the journal not seen yet
       *[other] { $count } kinds of problem in the journal not seen yet
    }
ask-journal-errors-filter = Text to filter errors by: a line whose unit and message carry it is hidden and left off the count
ask-journal-errors-filter-line = Cut this line down to what should be hidden
ask-journal-errors-reason = Why this filter exists (leave it empty to clear)
journal-errors-why-gone = That error is no longer in the list
journal-errors-why-no-row = Choose an error on its page first: this acts on one row
journal-errors-why-empty = Type a snippet to filter by first: an empty one would hide every error
journal-errors-why-unreadable = A file of yours for this module does not parse, so it is left as it is: fix it or remove it first
journal-errors-why-reading = It is reading the journal already
cmd-journal-errors-seen = Mark this error seen
cmd-journal-errors-ignore = Ignore this kind of error
cmd-journal-errors-unignore = Hear this kind of error again
cmd-journal-errors-seen-all = Mark every error seen
cmd-journal-errors-filter = Filter errors by a snippet of text
cmd-journal-errors-filter-line = Filter by this line
cmd-journal-errors-unfilter = Turn this filter off
cmd-journal-errors-refilter = Turn this filter back on
cmd-journal-errors-reason = Say why this filter exists
cmd-journal-errors-follow = Follow the journal live
cmd-journal-errors-run = Run now
journal-errors-reason-duplicate-name = Two service files claim one name; the first one read wins and nothing is lost.
journal-errors-reason-stopping-echo = Written while the machine stops; a service that truly failed says so in its own line.
journal-errors-reason-systemctl-echo = systemctl repeating a failure systemd has already written in its own line.
journal-errors-reason-stopping-job = A device appeared while the machine was stopping, so the job it asked for was dropped.
journal-errors-reason-watchdog-close = The hardware watchdog is left running on purpose as the machine shuts down.
journal-errors-reason-lxcfs-stop = lxcfs ends this way every time it is stopped.
journal-errors-reason-kernel-note = A note from the kernel about itself or the firmware; nothing to do.
journal-errors-reason-firmware-tables = A mistake in the machine's firmware tables; the kernel steps over it.
journal-errors-reason-bluetooth-optional = This Bluetooth adapter declines an optional feature and works without it.
journal-errors-reason-bluetooth-race = A Bluetooth packet arrived just after its connection closed, and was dropped.
journal-errors-reason-probe = A driver asks this of every device as it starts, and goes on when refused.
journal-errors-reason-device-gone = The device went away while it was being looked at.
journal-errors-reason-bus-clash = A brief clash on the touchpad's bus; the next transfer goes through.
journal-errors-reason-says-ignoring = The program itself says it carries on without this.
journal-errors-reason-swapfile = A disk scrub skips the swap file, as it must.
journal-errors-reason-tmpfiles-socket = The temporary-files cleaner met a socket it cannot lock, and left it alone.
journal-errors-reason-interface-renamed = The interface got its final name while its settings were being written.
journal-errors-reason-vpn-no-address = A VPN tunnel has no hardware address, so IPv6 makes no local address from it.
journal-errors-reason-p2p-unused = The Wi-Fi card's peer-to-peer side, which nothing here uses.
journal-errors-reason-dns-fallback = The resolver switched to a simpler way of asking this server, and goes on working.
journal-errors-reason-no-accessibility = A program looked for the accessibility bus, which is not running here.
journal-errors-reason-portal-namespace = The portal could not read a process's namespace and goes on without it.
journal-errors-reason-pipewire-client = A client of PipeWire left without tidying up; the server cleans up after it.
journal-errors-reason-midi-profile = BlueZ letting go of a MIDI profile; nothing waits for this reply.
journal-errors-reason-pam-env = pam_env skips a variable spelt in the old way, and goes on.
journal-errors-reason-own-test = The project's own test of a clone that fails on purpose.
journal-errors-reason-build-test = A package's test crashing on purpose inside a build.
journal-errors-reason-polkit-subject = A program asked polkit for something and exited before the answer.
journal-errors-reason-sudo-probe = A program checking whether sudo needs a password; nothing was asked of you.
journal-errors-reason-parts-overlap = Two parts offer the same file; one stays in the view, as intended.
journal-errors-reason-boot-version = systemd dislikes how NixOS spells a boot entry's version, and accepts it anyway.
journal-errors-reason-no-bluetooth = Bluetooth is off, so PipeWire goes on without it.
journal-errors-reason-pipewire-priority = PipeWire could not raise its priority and runs at the normal one.
journal-errors-reason-not-a-modem = ModemManager looked at a device that is not a modem.
journal-errors-heading-options = Options
journal-errors-about-section-options = What this watch does besides its list: tell you of new errors in a notification, and mark the modeline while some are not seen. Choose one to turn it on or off.
journal-errors-option-notify = Notify me of new errors
journal-errors-option-mark = Mark on the modeline
journal-errors-on = on
journal-errors-off = off
cmd-journal-errors-notify = Turn the notification of new journal errors on or off
cmd-journal-errors-mark = Show or hide the journal errors' mark on the modeline
notif-journal-errors-burst-summary = { $count ->
    [one] An error in the journal
   *[other] { $count } errors in the journal
    }
notif-journal-errors-burst-body = { $more ->
    [0] { $lines }
   *[other] { $lines } …and { $more } more
    }
