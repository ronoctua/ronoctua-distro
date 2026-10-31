# The texts `ronoctua-shell-update` says, in en-GB (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

brain-needs-root = Runs as root
# THE UPDATE, WATCHED — the modeline's mark, the leader's group for the view
# that watches a run, and the countdown's two answers that view teaches too.
#
# The mark shows only while something runs and goes on its own. The update's
# commands, its steps and the view's own words are its brain module's texts,
# in that module's folder.
group-update = update

## The machine's life (`ronoctua-machine`): why an apply, a migration, a bless or a hold did not do what it was asked (`machine-refused-<why>`).

machine-refused-not-root = Only root changes the machine: nothing was done.
machine-refused-busy = Another change to this machine is running: try again when it ends.
machine-refused-nothing-to-migrate = This machine already has its own flake: there is nothing to migrate.
machine-refused-build-failed = The new system did not build. Nothing changed: the machine runs what it ran, and its folder says so again. A download it could not fetch is named on the parts' page, with the ways to bring it in.
machine-refused-switch-failed = The switch to the new system failed. The next start tries it once, and comes back to the last good system if it does not start.
machine-refused-next-boot-failed = Setting the new system for the next restart failed. Nothing that runs changed; the boot menu may not list the new system, and the next restart may start the one this machine runs.
machine-refused-did-not-boot = This is the system that did not start when it was tried, or that was rolled back. The machine stays on the one it runs until a newer revision of the distro arrives.
machine-refused-last-good-would-fall-out = One more system would push the last good one out of the boot menu. Restart first, so the system on trial can be confirmed.
machine-refused-held = The machine is held at another revision than the one its folder names: nothing was built.
machine-refused-unreadable = A file of the machine's folder cannot be read.
machine-refused-failed = A program the machine needed failed; the journal says which.

## How the boot trial stands (`machine-trial-<outcome>`).

machine-trial-pending = The new system starts for the first time at the next restart. If it does not start, the machine comes back to the last good one by itself.
machine-trial-good = The new system started well: it is the last good one now.
machine-trial-failed = The new system did not start, or was rolled back: the machine is back on the last good one.
machine-trial-boot-identical = The new system starts exactly like the last good one: no restart is needed to trust it.

## Why the new system has no boot trial, until it has started once (`machine-no-trial-<why>`).

machine-no-trial-no-efi-variables = This machine's boot variables cannot be written, so the new system has no way back but the boot menu.
machine-no-trial-not-in-menu = The boot menu does not name the new system or the last good one in a way the trial can use, so the new system has no way back but the boot menu.
machine-no-trial-no-last-good = No system is known to have started well on this machine yet, so the new system has no way back but the boot menu.
# THE PERSON'S BINARY CACHE — a page of the update's view (`c`, `LEADER s u c`),
# in both doors, and what the installation and root's netrc say of it.
binary-cache-open = binary cache
cmd-binary-cache-open = Your binary cache: your own Cachix cache, which this machine downloads from and pushes to after each update
cmd-binary-cache-name = Names your binary cache — its name, then its public key
cmd-binary-cache-key = Gives your binary cache its public key, exactly as cachix.org shows it
cmd-binary-cache-token = Gives your binary cache its push token, kept encrypted with your belongings
cmd-binary-cache-netrc = Lets this machine read your private binary cache: root's netrc, written through sudo
ask-binary-cache-name = Name of your Cachix cache
ask-binary-cache-key = Public key of { $name }
binary-cache-heading = Your binary cache
binary-cache-row-cache = cache
binary-cache-row-key = public key
binary-cache-row-visibility = who reads it
binary-cache-row-token = push token
binary-cache-row-access = this machine reads it
binary-cache-row-machine = on this machine
binary-cache-row-last = last push
binary-cache-row-forget-token = forget the push token on this machine
binary-cache-row-remove = stop using this cache
binary-cache-about-cache = Your own Cachix cache — never the distro's, which has none. This machine downloads from it before cache.nixos.org, and pushes to it after each update that switched it. The choice travels with your belongings to your other machines. Choosing it asks its name, then its key.
binary-cache-about-key = The cache's signing key, exactly as cachix.org shows it on the cache's page: every download from it is checked against this key. Choosing it asks it.
binary-cache-about-visibility = A public cache receives only the distro's own outputs — never a path of yours, never an unfree program; anybody may download them. A private cache receives this machine's whole system, and reading it needs your token. Choosing it switches between the two.
binary-cache-about-token = The token Cachix gives you to push. It is kept encrypted in your belongings, readable by you alone, and never shown, written on a command line or logged. Choosing it asks a new one.
binary-cache-about-access = A private cache is read by this machine's Nix through a netrc only root reads, holding your token. It is written through sudo when you set the token or make the cache private; choosing it writes it again.
binary-cache-about-machine = What this machine uses now. A cache you name here reaches this machine at its next update, when your belongings' signed copy is applied.
binary-cache-about-last = What the last push carried, or why it was refused. A push follows each update that switched this machine, once.
binary-cache-about-forget-token = The token goes from this machine and from your belongings at their next sync; nothing more is pushed until you give it a new one.
binary-cache-about-remove = This machine stops downloading from and pushing to the cache at its next update. The cache itself, on Cachix, stays as it is.
binary-cache-none = none
binary-cache-key-none = none
binary-cache-public = public: the distro's own outputs, for anybody
binary-cache-private = private: this machine's system, read with your token
binary-cache-token-none = none: nothing is pushed
binary-cache-token-kept = kept, encrypted with your belongings
binary-cache-token-refused = not used
binary-cache-copy-unreadable = cannot be read
binary-cache-copy-newer = of a newer format
binary-cache-down = next row
about-binary-cache-down = Puts the cursor on the next row of the binary cache's page.
binary-cache-up = previous row
about-binary-cache-up = Puts the cursor on the previous row of the binary cache's page.
binary-cache-enter = change
about-binary-cache-enter = Does what the row under the cursor says: asks a name, a key or the token, switches who reads the cache, writes root's netrc, or asks before a gesture that does not come back.
binary-cache-steps = the steps
about-binary-cache-steps = Back to the update's steps.
binary-cache-access-written = yes, through root's netrc
binary-cache-access-missing = no: root's netrc is not written
binary-cache-machine-in-use = in use, before cache.nixos.org
binary-cache-machine-next = from the next update
binary-cache-machine-leaving = still the one before, until the next update
binary-cache-machine-next-restart = from the next restart
binary-cache-machine-leaving-restart = still the one before, until the next restart
binary-cache-machine-none = no cache
binary-cache-last-never = never
binary-cache-last-pushed = { $when }: { $paths ->
        [one] one path
       *[other] { $paths } paths
    } ({ $size })
binary-cache-last-refused = { $when }: nothing pushed
binary-cache-why-no-cache = This machine names no binary cache.
binary-cache-why-no-token = There is no push token on this machine.
binary-cache-why-token-open = The token's file is readable by somebody else, so it is not used: give the token again.
binary-cache-why-not-a-token = That is not a Cachix token.
binary-cache-why-not-let = A host the push reaches is not allowed in your consent: naming the cache again allows them.
binary-cache-why-nothing-new = No update switched this machine since the last push: there was nothing new to push.
binary-cache-why-no-tools = This machine has no cachix or nix where the push looks for them.
binary-cache-why-store-failed = Nix did not answer about the store.
binary-cache-why-cachix-failed = Cachix refused the push, or it did not finish: the update's output says why.
binary-cache-why-unwritable = The file could not be written.
binary-cache-why-newer = Your options file is of a newer format than this machine reads: update this machine first.
binary-cache-why-unreadable = Your options file does not read, so it is not rewritten.
binary-cache-confirm-forget = Forget the push token on this machine? It goes from your belongings too, and nothing is pushed until you give a new one.
binary-cache-confirm-remove = Stop using this binary cache on your machines? Each stops downloading from it and pushing to it at its next update.
binary-cache-answer-no = no, keep it
binary-cache-answer-yes-forget = yes, forget the token
binary-cache-answer-yes-remove = yes, stop using it
binary-cache-said-not-a-name = A Cachix name is lower-case letters, digits and dashes, starting with a letter or a digit.
binary-cache-said-not-the-key = That key is not this cache's: it is the name, .cachix.org-1: and 44 characters, as cachix.org shows it.
binary-cache-said-not-let = The cache was named, and the consent for its hosts could not be kept: nothing will be pushed until it is.
binary-cache-said-named = The cache is { $name }; this machine uses it from its next update.
binary-cache-said-public = The cache is public: it receives only the distro's own outputs.
binary-cache-said-private = The cache is private: it receives this machine's system, read with your token.
binary-cache-said-token = The push token is kept, encrypted with your belongings.
binary-cache-said-forgotten = The push token is forgotten on this machine.
binary-cache-said-removed = No binary cache any more, from the next update.
binary-cache-said-netrc-written = This machine reads your private cache now: root's netrc is written.
binary-cache-said-netrc-removed = Root's netrc for the cache is gone: nothing holds your token there any more.
binary-cache-said-netrc-footer = Root's netrc is asked for in the sudo bar, above the modeline, if sudo wants you.
# THE DISTRO'S APPS THAT FOLLOW THEIR RELEASES — a page of the update's view
# (`f`, `LEADER s u f`), in both doors: the person's option to let the routine
# update move them past the versions the distro tested.
distro-follow-open = follow releases
cmd-distro-follow-open = The distro's apps that follow their releases: whether each update moves them past the versions the distro tested
distro-follow-heading = The distro's apps that follow their releases
distro-follow-row-follow = follow their releases
distro-follow-row-hosts = asks
distro-follow-row-machine = on this machine
distro-follow-about-follow = When on, every update asks GitHub for the newest release of each app below — a version the distro's tests never built — and, once the update's own check has built it, puts it in your programs, in place of the distro's. One that does not build is held and tried again at every update; meanwhile the last one that built, or the distro's, stays. When the distro moves its own version past yours, the distro's comes back. Off, the distro's versions come back at the next update. The choice travels signed with your belongings to your other machines; one whose distro is older than this one refuses your options file until its distro is updated. Choosing it turns it.
distro-follow-about-hosts = Following asks these hosts, as every download asks you first: turning it on allows them. Without them the update says so, and the apps keep the versions they have.
distro-follow-about-machine = What this machine was built with: a change here reaches it at its next update. Your choice and the versions you follow reach a machine only signed with your key — one that trusts no key of yours, or has not got your signature yet, keeps the distro's.
distro-follow-about-app = The version this machine runs, and whose it is: the distro's, as its tests built it, or the newer one you follow, in your own programs. A newer one is used only once the update's check has built it; one that did not build is tried again at every update.
distro-follow-on = on
distro-follow-off = off
distro-follow-copy-unreadable = cannot be read
distro-follow-copy-newer = of a newer format
distro-follow-hosts-let = { $hosts }: allowed
distro-follow-hosts-not-let = { $hosts }: not allowed
distro-follow-machine-in-use = follows them
distro-follow-machine-next = from the next update
distro-follow-machine-leaving = still follows them, until the next update
distro-follow-machine-next-restart = from the next restart
distro-follow-machine-leaving-restart = still follows them, until the next restart
distro-follow-machine-none = the distro's versions
distro-follow-app-distro = { $version }, the distro's
distro-follow-app-followed = { $version }, followed
distro-follow-app-elsewhere = not on this machine
distro-follow-app-followed-but = { $version }, followed: { $note }
distro-follow-app-distro-but = { $version }, the distro's: { $note }
distro-follow-note-arriving = { $followed } is on its way: the parts' page says how it stands
distro-follow-note-missing = { $followed } could not be downloaded: the parts' page brings it in another way
distro-follow-note-failed = { $followed } did not build: the parts' page says why
distro-follow-note-refused-id = the followed version names no app that follows its releases
distro-follow-note-refused-url = the followed version is not a release of this app's own repository
distro-follow-note-refused-asset = the followed version is another file of its release
distro-follow-note-refused-version = the followed version is not its tag's
distro-follow-note-refused-hash = the followed version has no SHA-256 hash
distro-follow-note-refused-duplicate = a second followed version of it was left out
distro-follow-machine-no-key = not taken here: this machine trusts no key of yours
distro-follow-machine-unsigned = waits for your signature
distro-follow-machine-unreadable = your followed versions do not read here
distro-follow-why-newer = Your options file is of a newer format than this machine reads: update this machine first.
distro-follow-why-unreadable = Your options file does not read, so it is not rewritten.
distro-follow-why-unwritable = The file could not be written.
distro-follow-none = None of the distro's apps on this machine follows its releases.
distro-follow-said-on = The distro's apps follow their releases from the next update.
distro-follow-said-off = The distro's versions come back at the next update.
distro-follow-said-not-let = Following is on, and allowing GitHub in your consent failed: the update will say it was not allowed.
distro-follow-down = next row
about-distro-follow-down = Puts the cursor on the next row of the page of the apps that follow their releases.
distro-follow-up = previous row
about-distro-follow-up = Puts the cursor on the previous row of the page of the apps that follow their releases.
distro-follow-enter = change
about-distro-follow-enter = Turns following on or off, on its row.
distro-follow-steps = the steps
about-distro-follow-steps = Back to the update's steps.
distro-parts-open = parts
cmd-distro-parts-open = The machine's parts: what arrives after the installation, each on its own, and the other ways to bring in a download that keeps failing
cmd-distro-parts-hand-in = Hands in a file you downloaded elsewhere, for the part or the system under the cursor: it is taken only if it is that very download
cmd-distro-parts-import = Brings in, from a stick, the downloads the parts and the system are missing
cmd-distro-parts-export = Writes the downloads this machine keeps for its parts and its system on a stick
distro-parts-heading = What arrives after the installation
distro-parts-none = This machine wants no part: everything it runs came with the installation.
distro-parts-row-import = bring in from a stick
distro-parts-row-export = write on a stick
distro-parts-row-system = the system
distro-parts-system-failed = { $count ->
        [one] the last update did not build: one download is missing
       *[other] the last update did not build: { $count } downloads are missing
    }
distro-parts-system-waits = { $count ->
        [one] the hardware that waits for a network is missing one download
       *[other] the hardware that waits for a network is missing { $count } downloads
    }
distro-parts-system-all-here = everything it was missing is here: update again
distro-parts-phase-here = here
distro-parts-phase-arriving = on its way
distro-parts-phase-retrying = { $tries ->
        [one] one try failed; the next at { $when }
       *[other] { $tries } tries failed; the next at { $when }
    }
distro-parts-phase-waiting = waiting for a network
distro-parts-phase-metered = waiting for an unmetered network
distro-parts-about-metered = On a metered network nothing is downloaded by itself: it waits for another one. Choosing it uses this one, for this try only.
distro-parts-phase-needs-you = needs you: { $tries } tries failed; the next at { $when }
distro-parts-about-part = A part arrives after the installation, on its own: one that does not arrive holds back no other, and the machine works without it. It is tried again by itself, less often each time, and choosing it tries it now.
distro-parts-about-missing-by-hand = It is missing { $name }, from { $hosts }: download it elsewhere and hand it in with { $keys }, bring it on a stick from a machine that has it, or name a mirror that holds it.
distro-parts-about-missing-stick = It is missing { $name }, which is not one file: it comes on a stick from a machine that has it.
distro-parts-about-missing-here = { $name } is here now.
distro-parts-about-system = What the system's last update could not fetch. The machine runs as it did before, and every update tries it again; choosing it updates now.
distro-parts-about-system-waits = What the hardware that waits for a network still needs: the machine was updated without it, and every update tries it again; choosing it updates now.
distro-parts-about-system-all-here = Everything it was missing is here: the next update builds with it.
distro-parts-about-next-restart = These are the parts of the system that runs. A new system waits for this machine's next restart: the parts it wants arrive after that restart, and are drawn here then.
distro-parts-about-why-build-failed = The last try failed: { $detail }
distro-parts-about-why-output-differs = The last try built something other than what this system wants; the next update brings the right one.
distro-parts-about-why-profile-failed = The last try built it, and it could not be put in place.
distro-parts-about-why-hand-in-refused = The file handed in was refused: { $detail }
distro-parts-about-why-eval-failed = What you declared did not evaluate: { $detail }
distro-parts-about-import = Brings in, from a stick another machine of yours filled, the downloads the parts and the system are missing. Only downloads are taken, each checked by its content — never a program built elsewhere. Choosing it asks for the stick's folder.
distro-parts-about-export = Writes the downloads this machine keeps for its parts and its system on a stick, for another machine to bring in. Choosing it asks for the stick's folder.
distro-parts-stick-last-import = { $count ->
        [one] Last time, one download was brought in, { $when }.
       *[other] Last time, { $count } downloads were brought in, { $when }.
    }
distro-parts-stick-last-export = { $count ->
        [one] Last time, one download was written, { $when }.
       *[other] Last time, { $count } downloads were written, { $when }.
    }
distro-parts-stick-refused = Last time, { $when }, nothing was moved:
distro-parts-refused-no-folder-named = no folder was named.
distro-parts-refused-folder-missing = the folder is not there.
distro-parts-refused-folder-through-a-link = the way to the folder goes through a link.
distro-parts-refused-not-a-removable-medium = the folder is not on a stick or another removable medium.
distro-parts-refused-no-downloads-there = the stick holds no downloads written by a machine of this distro.
distro-parts-refused-unwritable = the downloads could not be written there.
distro-parts-said-retrying = { $name } is being tried now.
distro-parts-said-not-started = The machine did not start it.
distro-parts-said-already-here = It is here already.
distro-parts-said-not-a-part = Put the cursor on a part, or on the system, to hand in a file for it.
distro-parts-said-nothing-missing = It is missing no download now: choose it to try it again.
distro-parts-said-not-one-file = What it is missing is not one file: bring it on a stick.
distro-parts-said-no-file = There is no file there.
distro-parts-said-not-copied = The file could not be handed over.
distro-parts-said-taken = { $name } was taken, and the part is being built.
distro-parts-said-taken-for-the-system = { $name } was taken: the next update builds with it.
distro-parts-said-refused = { $name } was refused: it is not that download.
distro-parts-said-not-named = The stick's folder could not be named for the machine.
distro-parts-said-imported = { $count ->
        [one] One download was brought in.
       *[other] { $count } downloads were brought in.
    }
distro-parts-said-exported = { $count ->
        [one] One download was written on the stick.
       *[other] { $count } downloads were written on the stick.
    }
cmd-distro-parts-mirrors = Names the mirrors the machine asks for a part's download when its origin fails
ask-distro-parts-mirrors = Your mirrors' addresses, separated by spaces (nothing names none)
distro-parts-row-mirrors = mirrors
distro-parts-mirrors-none = none
distro-parts-mirrors-unreadable = your options file cannot be read
distro-parts-mirrors-newer = your options file is of a newer format
distro-parts-about-mirrors = Servers of yours that hold downloads by their hash or by their name — a folder on a machine of your network, a copy of tarballs.nixos.org. When a part's download fails at its origin, the machine asks them, and takes a file only if it is the very one the distro pinned. They travel with your options and reach a machine at its next update. Choosing it asks for their addresses.
distro-parts-why-unreadable = Your options file does not read, so it is not rewritten.
distro-parts-why-newer = Your options file is of a newer format than this machine reads: update this machine first.
distro-parts-why-unwritable = The file could not be written.
distro-parts-said-not-a-mirror = A mirror is an http or https address, with no user name, query or fragment.
distro-parts-said-too-many-mirrors = That is more mirrors than your options may name.
distro-parts-said-no-mirror = No mirror now: the next update takes them away from this machine.
distro-parts-said-mirrors = The mirrors reach this machine at its next update.
distro-parts-said-mirrors-not-let = The mirrors reach this machine at its next update; allowing their hosts in your consent failed.
distro-parts-pick-hand-in = The file you downloaded elsewhere
distro-parts-pick-import = The stick to bring the downloads in from
distro-parts-pick-export = The stick to write the downloads on
distro-parts-about-pick-hand-in = Choose { $name }, as you saved it after downloading it elsewhere: it is taken only if it is that very download.
distro-parts-stick-never = not yet
distro-parts-stick-moved = { $count ->
        [one] one download, { $when }
       *[other] { $count } downloads, { $when }
    }
distro-parts-stick-refused-short = refused, { $when }
distro-parts-down = next row
about-distro-parts-down = Puts the cursor on the next row of the parts' page.
distro-parts-up = previous row
about-distro-parts-up = Puts the cursor on the previous row of the parts' page.
distro-parts-enter = try now
about-distro-parts-enter = Tries the part under the cursor now; on a stick's row, asks for the stick's folder.
distro-parts-hand = hand in a file
about-distro-parts-hand = Hands in a file you downloaded elsewhere for the part under the cursor.
distro-parts-steps = the steps
about-distro-parts-steps = Back to the update's steps.
