# The texts `ronoctua-shell-software` says, in en-GB (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

## The window menu — the modeline's app name, clicked (LEADER w o)

# THE PERSON'S OWN PROGRAMS (LEADER i) — `software.rs`, its words
# (`software_words.rs`) and the queue's phases. A key is never text here:
# every sequence arrives as `{ $keys }`.
software-title = Software
group-software = programs
cmd-software-open = Your programs: search, install, try and remove
cmd-software-window = Your programs, in a window
cmd-software-trials = The programs being tried
cmd-software-maintenance = Updates, space and going back, for your programs
cmd-software-android = Android and its apps
cmd-software-launch-latest = Open what was installed last
software-down = down
software-up = up
software-enter = open
software-next-source = next source
software-previous-source = previous source
software-less = previous
software-more = next
software-install = install
software-try-it = try
software-launch = start
software-update = update
software-edit = edit
software-remove = remove
software-stop = stop
software-paste = paste
cmd-software-down = The next row
cmd-software-up = The row above
cmd-software-enter = Do what this row does; on a fact, copy it
cmd-software-next-source = Show the next source, or the next shelf of icons
cmd-software-previous-source = Show the previous source, or the previous shelf of icons
cmd-software-less = The previous value of this row
cmd-software-more = The next value of this row
cmd-software-install = Install it for you, from the source shown: declared, so all your computers have it — a trial is kept here only
cmd-software-try-it = Run it without installing
cmd-software-launch = Start the program
cmd-software-update = Update this one now
cmd-software-edit = Change this web app
cmd-software-remove = Remove it, after a question
cmd-software-stop = Stop it
cmd-software-paste = Paste what was copied into the search
cmd-software-web-new = A web app from an address
cmd-software-icon-path = An image of yours as the web app's icon
cmd-software-file-path = Install an .AppImage, .apk or .flatpakref file
software-type-on-a-row = To write here, press { $keys } on the row.
software-type-to-search = Type a name, or what it does, and the search starts by itself.
software-reading = Reading what is on this computer…
software-on = on
software-off = off
software-joined = { $first } · { $second }
software-source-all = every source
software-source-system = the system
software-search = Search
software-search-empty = type a name, or what it does
software-filter = Showing
software-heading-installed = Installed for you
software-note-yours = What is installed here is yours alone. What you declare also follows you: each of your computers installs it at its update.
software-note-none = Nothing installed by you yet. The system's own programs are in the launcher — { $keys }.
software-note-none-here = Nothing from this source.
software-heading-trying = Being tried
software-heading-looking-after = Looking after
software-updates = Updates, space and going back
software-updates-count = { $count ->
        [one] one available
       *[other] { $count } available
    }
software-up-to-date = up to date
software-heading-more = More
software-web-new = New web app
software-web-new-value = from an address
software-trials = Trials
software-trials-count = { $count ->
        [0] none running
        [one] one running
       *[other] { $count } running
    }
software-android-state = Android system
software-android-absent = not on this machine
software-android-not-prepared = not prepared
software-android-preparing = preparing
software-android-running = running
software-android-stopped = stopped
software-advanced = Advanced
software-advanced-value = sources, permissions, history
software-heading-search = Looking for a program
software-make-web-app = Make a web app of it
software-open-once = Open it once, without making one
software-install-file = Install this file
software-not-a-package-file = That is not an .AppImage, .apk or .flatpakref file.
software-note-type = Type a name, or what it does. An address makes a web app; a path to an .AppImage, .apk or .flatpakref installs that file.
software-heading-results = Found
software-heading-by-description = Found by what they do
software-note-nothing-found = Nothing by that name. Another spelling, or what it does, may find it.
software-heading-asked = Where it looked
software-note-offline = No network: showing what this computer already knows.
software-row-working = Now
software-queued = waiting in the queue
software-working-now = working
software-working = { $percent } % · { $phase }
software-row-launch = Open the program
software-no-window = no window: it runs in a terminal
software-install-with = with { $source }
software-recommended = recommended
software-size-download = { $size } to download
software-row-try = Try without installing
software-try-terminal = opens a terminal where { $program } can be typed
software-try-window = opens it in a window and keeps nothing
software-row-traces = What it may leave
software-row-network = Network while trying
software-row-keep = Keep it for good
software-keep-value = installs what was tried, with no new download
software-row-stop = Stop trying it
software-row-source = Source
software-also-in = also in { $sources }
software-row-version = Version
software-row-size = Size
software-row-update = Update
software-update-to = { $version } is ready
software-row-permissions = Permissions
software-permissions-value = what it may reach
software-row-edit = Change this web app
software-row-remove = Remove from this computer
software-heading-about = About it
software-heading-details = Details
software-row-reference = Its name in the source
software-row-publisher = Made by
software-row-licence = Licence
software-row-site = Site
software-row-needs = Needs
software-row-installed-size = Installed, it takes
software-row-permission = May reach
software-row-anti-features = F-Droid warns about
software-note-trial = Trying runs it now and installs nothing; it leaves the disk at the next clean-up.
software-heading-trials = What you are trying
software-try-new = Try something new
software-try-new-value = type its name
software-note-no-trials = Nothing is being tried.
software-trial-running = open for { $minutes } min
software-trial-closed = closed · on disk until the next clean-up
software-heading-web = A new web app
software-web-address = Address
software-web-no-address = none yet
software-web-name = Name
software-web-icon = Icon on the launcher
software-web-no-icon-yet = none chosen yet
software-web-profile = Chrome profile
software-web-category = Kind
software-web-fetching = Reading the site's name and icons…
software-web-host-name = the name stays the site's address until one is typed
software-web-create = Create it
software-web-save = Save the changes
software-web-once = Open it once, as it will look
software-web-remove = Remove this web app
software-web-no-icon = Choose an icon first: the launcher needs one.
software-note-web = A web app is the site in a window with nothing of Chrome's; its logins are your Chrome profile's.
software-heading-icon = Its icon
software-shelf = From
software-shelf-site = the site itself
software-shelf-dashboard = Dashboard Icons
software-shelf-collection = the distro's collection
software-shelf-file = a file of yours
software-icon-looking-for = Looking for
software-icon-typed = Choose an image from the files
software-icon-clipboard = Use the copied image
software-icon-files = Pick one in the file manager
software-note-drop = Or drop an image here.
software-icon-now = Chosen now
software-icon-chosen = chosen
software-note-no-icons = No icon by that name here.
software-heading-android = Android
software-note-android = Android apps run inside an Android system beside this one; it costs memory while it runs.
software-android-image = Image
software-note-gapps = Google Play asks for this machine to be registered once as an uncertified device; this page shows how once Android is prepared.
software-android-prepare = Prepare Android
software-note-preparing = Downloading and preparing Android's image — about a gigabyte, once. The queue shows it; no percentage is invented here.
software-heading-register = Registering for Google Play
software-note-uncertified = The Play Store only works after this device is registered on Google's page for uncertified devices: copy the id below and paste it there.
software-android-id = Device id
software-android-id-unknown = known once Android has run
software-android-register = Google's page
software-android-windows = Windows
software-android-one-per-app = one per app
software-android-one-for-all = one for all of Android
software-note-windows-next-session = A change of windows takes effect the next time Android starts.
software-heading-android-apps = Inside Android
software-note-no-android-apps = No app installed inside Android yet.
software-android-apk = Install an APK file
software-android-aurora = Google Play without an account
software-android-aurora-value = Aurora Store, from F-Droid
software-android-erase = Erase Android
software-cannot-undo = this cannot be undone
software-heading-updates = Updates
software-updates-all = Everything
software-web-always-latest = always the site's own version
software-follows-system = follows the system (nixpkgs { $rev })
software-look-again = Look for updates now
software-updates-auto = Automatically
software-auto-with = with the system's update — { $keys }
software-heading-space = Space
software-clean = Free space now
software-clean-generations = Old versions of your Nix list
software-clean-flatpak = Unused Flatpak runtimes
software-clean-appimage = AppImage leftovers
software-clean-trials = Tried programs
software-clean-trials-value = leave by themselves at the next clean-up
software-heading-generations = Going back (Nix)
software-note-no-generations = Your Nix list has no earlier version yet.
software-generation = Version { $n }
software-in-use = in use
software-older-versions = Older versions
software-heading-sources = Sources
software-online = Search online
software-online-off = off, with the search online
software-switch-flathub = Flathub
software-switch-fdroid = F-Droid
software-switch-appimagehub = AppImageHub and GitHub
software-switch-dashboard = Dashboard Icons, for web apps
software-nix-index = Nix index
software-prefer = Recommend first
software-ask-above = Ask before downloading
software-ask-above-size = { $size } or more
software-ask-never = never
software-heading-trial-defaults = When trying
software-default-traces = What a trial may leave
software-default-network = Network in the sealed trial
software-heading-web-defaults = New web apps
software-web-profile-default = Chrome profile they use
software-profile-last-used = the one Chrome used last
software-heading-kept = What is kept
software-consents = Non-free packages allowed
software-consents-count = { $count ->
        [0] none
        [one] one
       *[other] { $count }
    }
software-history = History
software-history-value = the last few hundred things done
software-heading-permissions-list = Flatpak permissions
software-heading-dark = Dark Flatpaks
software-dark-gtk4 = GTK 4 and libadwaita
software-dark-follows = follow the system's choice
software-dark-gtk3 = GTK 3
software-dark-gtk3-value = dark, through the system's Adwaita-dark
software-dark-gtk3-missing = light: the Adwaita-dark extension is not installed
software-dark-qt = Qt and KDE
software-dark-qt-value = the session's Qt settings kept out
software-dark-qt-leaks = the session's Qt settings reach them
software-dark-forced = Forcing a theme
software-dark-none-forced = no app forces GTK_THEME
software-dark-forces = forces GTK_THEME — see its permissions
software-note-gtk-theme = GTK_THEME makes a Flatpak lose its own style; nothing here sets it, and an app that carries it is shown in red.
software-heading-permissions = What { $app } may reach
software-note-no-permissions = It asks for nothing beyond its own window.
software-note-permissions-restart = A change reaches the app the next time it starts.
software-heading-asks = What it asks for
software-gtk-theme-forced = forced into it: it loses its own style
software-sure-reset = Put { $app } back to what it asks for? Every change made here goes.
software-yes-reset = Yes, put it back
software-sure-unset-gtk-theme = Take GTK_THEME away from { $app }? It gets its own style back, and nothing here can put GTK_THEME back.
software-yes-unset-gtk-theme = Yes, take it away
software-heading-consents = Non-free packages you allowed
software-note-consents = Nix installs these although their licence is not free, and updates them with that same yes.
software-note-no-consents = None yet.
software-allowed = allowed
software-heading-history = What was done
software-note-no-history = Nothing done yet.
software-sure-remove = Remove { $app }?
software-yes-remove = Yes, remove it and keep what it saved
software-yes-remove-data = Yes, and erase what it saved too
software-sure-web-remove = Remove the web app { $app }? The site and your login stay in Chrome.
software-yes-remove-web = Yes, remove the web app
software-sure-unverified = Nothing vouches for what { $app } downloads. Install it anyway?
software-yes-trust = Yes, I trust whoever made it
software-sure-arm = { $app } is made for phone chips only and may not run here. Install it anyway?
software-yes-arm = Yes, install it anyway
software-sure-rollback = Go back to version { $n } of your Nix list?
software-yes-rollback = Yes, go back
software-sure-clean = Free what nothing uses? Old versions go, and going back to them stops being possible.
software-yes-clean = Yes, free it
software-sure-android = Prepare Android { $image }? It downloads once and stays until you erase it.
software-yes-android = Yes, prepare it
software-sure-erase-android = Erase Android, its apps and everything they saved?
software-yes-erase = Yes, erase it
software-sure-foreign = { $file } was checked by no catalogue and runs with your permissions. Install it?
software-sure-leave = Leave without saving?
software-yes-leave = Yes, throw the changes away
software-no-such-level = “{ $words }” names no page of the programs.
software-no-copied-image = No image was copied.
software-image-not-kept = The copied image could not be written to a file.

## file picker — the one picker of a file, a folder or a name to save under

file-picker-filter-packages = Programs to install
software-pick-package = A file to install — an AppImage, an APK or a Flatpak reference

## file info editing — everything the info panel can change

software-consent-flathub = Flathub's search, its programs' pages and their pictures.
software-consent-fdroid = F-Droid's index, its icons and the packages it serves.
software-consent-appimagehub = The AppImage catalogue, and the GitHub releases it points at.
software-consent-dashboard = The icon index a web app's picture is chosen from.
cmd-software-sign = Sign a change of what you declared with your belongings' passphrase, so it reaches this machine and your others now
ask-software-sign = Passphrase of the belongings, to sign this change
software-declared-door = Declared by you
software-declared-door-declined = Declined by you
software-declared-count = { $count ->
        [one] one program
       *[other] { $count } programs
    }
software-declared-count-out = { $count ->
        [one] one left out here
       *[other] { $count } left out here
    }
software-declared-declined-count = { $count ->
        [one] one kept uninstalled
       *[other] { $count } kept uninstalled
    }
software-declared-none-yet = nothing yet
software-declared-none-declined-yet = nothing
software-declared-note = What you declare goes to every computer of yours: each update installs it and keeps it, and it travels with your belongings.
software-declared-not-reported = This computer's system was built before it knew your declarations: the next update applies them.
software-declared-none = Nothing declared yet. Install only for you, on a program's page, declares it.
software-declared-android-pending = Some declared Android apps wait for Android to start, and go in then.
software-declared-heading-pins = Downloads
software-declared-refresh = Look for newer downloads
software-declared-refresh-value = the next update builds them, after trying each
software-declared-refresh-one = Look for a newer download
software-declared-refresh-one-value = the next update builds it, after trying it
software-declared-file-declared = your declared programs
software-declared-file-declined = your declined programs
software-declared-unreadable-newer = The file of { $file } was written by a newer distro, and is left as it is: update this computer to read it.
software-declared-unreadable-broken = The file of { $file } does not read, and is left as it is: the next sync of your belongings may mend it.
software-declared-gone = It is no longer in your declaration.
software-declared-row-here-now = Here
software-declared-row-said = What the system said
software-declared-row-why = Why
software-declared-row-on = Goes to
software-declared-all-computers = all your computers
software-declared-everywhere = on all your computers
software-declared-row-pin = Pinned version
software-declared-row-download = Download
software-declared-row-edited = Last changed
software-declared-edited-on = { $when }, on { $machine }
software-declared-heading-fixes = What can be done
software-declared-row-program = The program
software-declared-program-value = opens its page
software-declared-note-waiting = The next update installs it here: nothing has to be done.
software-declared-note-arriving = It arrives on its own, after the system, as a part: the parts' page says how it stands.
software-declared-row-also-here = Also on this computer
software-declared-note-source-off = This computer has no support for its source: a reduced profile has no Android and no AppImages.
software-declared-note-update-me = Update this computer: a newer distro wrote this declaration.
software-declared-row-replace = Find another
software-declared-replace-value = searches for programs like it
software-declared-row-forget = Remove for good
software-declared-forget-value = here, and from your declaration
software-declared-note-declined = The distro's programs you keep uninstalled, here or on every computer of yours: each update leaves them out.
software-declared-none-declined = Nothing declined: every program of the distro is installed.
software-declared-heading-distro = The distro's programs
software-declared-distro-installed = installed with the system
software-declared-row-group = Group
software-declared-not-on-this-machine = This computer's profile does not carry it.
software-declared-row-declined = Declined
software-declared-row-bring-back = Bring it back
software-declared-bring-back-value = on every computer, and here at once
software-declared-bring-back-here = { $others ->
        [0] here at once
        [one] here at once; your other computer keeps it out
       *[other] here at once; your { $others } other computers keep it out
    }
software-declared-back-elsewhere = Each computer that declined it brings it back itself; from here, it comes back on all of them at once.
software-declared-row-bring-back-everywhere = Bring it back on every computer
software-declared-row-decline-here = Keep it uninstalled here
software-declared-row-decline-everywhere = Keep it uninstalled on all your computers
software-declared-row-for-me = Install only for you
software-declared-row-here = Install on this computer only
software-declared-here-value = not declared: your other computers do not get it
software-declared-row-keep = Keep it on all your computers
software-declared-keep-value = declared: each of your computers installs it at its update
software-declared-row-declared = Declared
software-declared-sure-forget = Remove { $app } for good?
software-declared-forget-here = It leaves this computer and your declaration.
software-declared-forget-elsewhere = Your other computers remove it at their next update.
software-declared-forget-data = What it saved stays, unless you choose to erase it.
software-declared-yes-forget = Yes, remove it for good
software-declared-sure-decline-here = Keep { $app } uninstalled on this computer?
software-declared-sure-decline-everywhere = Keep { $app } uninstalled on all your computers?
software-declared-decline-rebuild = This computer is rebuilt now, without it.
software-declared-decline-others = Your other computers leave it out at their next update.
software-declared-decline-back = Declined by you brings it back.
software-declared-yes-decline = Yes, keep it uninstalled
software-declared-sure-bring-back-everywhere = Bring { $app } back on every computer?
software-declared-back-everywhere-others = Every computer that declined it installs it again at its next update — a computer you no longer have included, whose decline nothing else could undo.
software-declared-back-everywhere-here = This computer does not change: it never left it out.
software-declared-yes-bring-back-everywhere = Yes, bring it back everywhere
software-declared-registry-broken = This computer's list of the distro's programs does not read, so none of them is shown here. The next update of this computer writes it again.
software-declared-registry-newer = This computer's list of the distro's programs was written by a newer distro, so none of them is shown here: update this computer.
software-declared-returning = No longer declined here: this computer's next update brings it back.
software-declared-web-edit = It is declared: saving changes your declaration, and your other computers take the change at their next update.
