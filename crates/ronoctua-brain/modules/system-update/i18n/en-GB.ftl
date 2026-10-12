# The system-update module's own texts: its name and the line under it, its
# commands, the name and the sentence of every step of the sequence, the
# closed words the sequence's table carries, and the view that watches a run.
brain-task-system-update = Update the system
brain-task-system-update-does = Gathers what you declared on your other machines, moves the distro forward, builds the new system and switches to it — the next boot tries it once and comes back to the last good one if it does not start —, updates your own programs and caches, reclaims store space, and publishes what this machine did. The switch restarts the services whose definition changed; programs already open keep the version they started with, and a new kernel waits for the next reboot.

# ITS COMMANDS. The whole sequence now, its rehearsal, and the two doors of
# the view that watches it — the footer's region and a window.
cmd-system-update = Update the system
cmd-system-update-rehearsal = Rehearse the update
cmd-system-update-open = The update, in the footer
cmd-system-update-watch = The update, in a window

# EACH STEP'S NAME, and under it the line that reaches the message line when
# the cursor is on it. The name is what it is called; the line under it is
# what it actually runs, and what it costs to be wrong about.
#
# The generator (`gen-updates.py`, beside this folder's `steps.nix`) derives
# both keys from every step's id and hands them to the brain on the wire, so
# a step with no sentence in both languages cannot be written. The order is
# the SEQUENCE'S order, which is the declaration's: sorting these would put
# back the second source the table exists to remove.
system-update-step-flake-update = fetch the new versions
system-update-step-flake-update-does = nix flake update of the distro's input, in this machine's own flake: it moves the distro to its newest release and touches nothing but flake.lock — the other inputs are the release's own, as it was tested. It needs the network; it is skipped and says so while the machine is held at a revision, on the ISO, whose machine folder is read-only, and whenever the rebuild would be — on battery below the floor, or with the disk nearly full — so the folder never names a distro the machine did not build.
system-update-step-system-rebuild = rebuild the system
system-update-step-system-rebuild-does = ronoctua-machine apply: it builds the new system first and switches only to what built — a build that fails leaves the machine and its folder as they were. After the switch the next boot tries the new system once, and comes back to the last good one by itself if it does not start. It is skipped on battery below the floor unless you said yes, and when the disk is nearly full even after collecting the store's garbage. If it fails, the sequence stops here.
system-update-step-flatpak-system = update the system's Flatpaks
system-update-step-flatpak-system-does = flatpak update --system: it moves what the machine's own Flatpak installation holds — the Flathub remote, and whatever the configuration declares — and installs nothing. Your own apps are in your installation and move in your half, further down. Without that installation the step is skipped and says so.
system-update-step-android-images = update Android's system image
system-update-step-android-images-does = ronoctua-software upgrade android-system: Waydroid's own upgrade of the image your Android apps run on — about a gigabyte, checked against the digest its channel publishes. It waits while Android is running rather than stopping it. It ships off: turning it on is one line in this machine's configuration.
system-update-step-old-generations = trim the system's old generations
system-update-step-old-generations-does = ronoctua-system-generations: the system's generations beyond the newest ones go (how many is this machine's configuration) — never the one running, the one it booted, the one it last switched away from, the profile's current one or one the boot menu lists, so the way back stays. Only the system's: your own Nix profile is never touched. The boot menu lists no more than it keeps, so every entry still boots. A rehearsal of the update runs this too, and says which generations would go without removing any.
system-update-step-store-collect-garbage = collect the store's garbage
system-update-step-store-collect-garbage-does = nix-collect-garbage with no --delete-older-than and no -d: it deletes only what nothing references — never a generation. Which generations go was decided by the trim before it, by count.
system-update-step-store-optimise = deduplicate the store
system-update-step-store-optimise-does = nix store optimise: identical files become hard links. It runs after the collection, or it would optimise precisely what was about to be deleted.
system-update-step-journal-vacuum = trim the journal
system-update-step-journal-vacuum-does = journalctl --vacuum-size=256M: it throws away the oldest record, this update's own included — which is the answer to "why did the rebuild fail?". It ships off.
system-update-step-font-cache = rebuild the font cache
system-update-step-font-cache-does = fc-cache -rf: from scratch, ignoring the timestamps, because a rebuild can have changed the font set and a stale cache with a fresh date is exactly what brings somebody here.
system-update-step-tldr-cache = update the tldr pages
system-update-step-tldr-cache-does = tldr --update, as you: the weekly unit downloads them as `nobody`, into a home nobody reads. It needs the network; without it the step is skipped, rather than waiting for a timeout and calling that a failure.
system-update-step-nix-profile = update your Nix programs
system-update-step-nix-profile-does = ronoctua-software upgrade nix: the programs in your own Nix profile, moved to the nixpkgs the system was just rebuilt from. It installs none, and the ones you agreed to although they are not free go through with that same agreement. Without a profile the step is skipped and says so.
system-update-step-flatpak-user = update your Flatpaks
system-update-step-flatpak-user-does = ronoctua-software upgrade flatpak: your own Flatpak installation — the apps and the runtimes they use — and Flathub's catalogue with it. It installs none. Without that installation the step is skipped and says so.
system-update-step-appimages = update your AppImages
system-update-step-appimages-does = ronoctua-software upgrade appimage: each AppImage in ~/Applications against the releases of the place it came from. The new file replaces the old one in a single step, so a download that fails leaves the old one working. Without that folder the step is skipped and says so.
system-update-step-android-apps = update your Android apps
system-update-step-android-apps-does = ronoctua-software upgrade android: the apps inside Android that came from F-Droid, each checked against who signed it and the digest the index records. Apps you installed from inside Android are Android's own business.
system-update-step-rust-toolchains = update the Rust toolchains
system-update-step-rust-toolchains-does = rustup update: it refreshes the channels already under ~/.rustup, and installs none. Here Rust comes from the flake, so that directory does not exist and the step is skipped saying so — it is for the machine where you put rustup there by hand.
system-update-step-node-global = update npm's global packages
system-update-step-node-global-does = npm update --global: it moves what is already in the global prefix and installs nothing. Node comes from the flake; the global prefix does not — without that directory the step is skipped and says so.
system-update-step-pnpm-global = update pnpm's global packages
system-update-step-pnpm-global-does = pnpm update --global: pnpm keeps a global store of its own, which is not npm's — which is why these are two steps and not one. Without its directory the step is skipped and says so.
system-update-step-python-tools = update the uv tools
system-update-step-python-tools-does = uv tool upgrade --all: it upgrades the tools under $XDG_DATA_HOME/uv/tools, one venv each, and adds none. uv comes from the flake; they do not — without that directory the step is skipped and says so.
system-update-step-doom-sync = Bring Doom Emacs's packages in line
system-update-step-doom-sync-does = Resolves Doom's package list against the configuration the rebuild just wrote, and byte-compiles what changed. Until it runs, the editor is still loading the previous one. It skips on a machine where Doom was never installed.
system-update-step-flatpak-unused = remove the Flatpak runtimes nothing uses
system-update-step-flatpak-unused-does = ronoctua-software tidy flatpak: the runtimes and extensions in your installation that no app of yours needs any more — what an update leaves behind. No app is ever touched.
system-update-step-nix-profile-history = forget the old Nix profile generations
system-update-step-nix-profile-history-does = ronoctua-software tidy nix: the old generations of your Nix profile, each of which keeps the programs it had alive in the store. It ships off: nothing automatic removes your way back to a program you had. The software view offers the same tidy when you ask for it, and turning it on here is one line in this machine's configuration.
system-update-step-appimage-cache = clear what AppImages left behind
system-update-step-appimage-cache-does = ronoctua-software tidy appimage: what appimage-run unpacked for AppImages that are gone or were replaced — it unpacks each one the first time it starts and never cleans up after it. The AppImages themselves are never touched.

# WHY A STEP WAS SKIPPED. The path or the name it is about travels beside the
# sentence as data (`Needs::about()`), which is why none of these names one.
system-update-needs-always = nothing can make this step inapplicable
system-update-needs-binary = this program is not on your PATH
system-update-needs-directory = this directory does not exist
system-update-needs-writable-file = this file does not exist, or cannot be written to
system-update-needs-network = NetworkManager says this machine is not connected

# WHAT A FAILURE DOES TO THE REST OF THE SEQUENCE. Two answers and no third:
# this runs from a unit, and there is nobody to ask.
system-update-on-failure-stop = if it fails, the sequence stops here
system-update-on-failure-carry-on = if it fails, the sequence carries on to the next step

# THE FIVE CLASSES A STEP CAN BE. They decide the order laws.
system-update-kind-move-inputs = moves the pinned inputs
system-update-kind-apply-configuration = applies the configuration
system-update-kind-upgrade-software = updates programs you installed yourself
system-update-kind-refresh-cache = refreshes a cache
system-update-kind-reclaim-space = reclaims disk space

# THE VIEW THAT WATCHES A RUN, in both doors: its keys and what each says under
# the pointer, `update now` (the key the view teaches for the whole sequence),
# what starting said, and the word beside each step.
system-update-next-step = next step
about-system-update-next-step = Moves to the next step, and says what it is.
system-update-previous-step = previous step
about-system-update-previous-step = Moves to the previous step, and says what it is.
system-update-output-down = output down
about-system-update-output-down = Moves the output a page towards its end.
system-update-output-up = output up
about-system-update-output-up = Moves the output a page towards its start.
system-update-output-end = the end
about-system-update-output-end = Goes to the end of the output, where a run in progress is.
system-update = update now
echo-system-update-queued = In the queue — it starts when the machine is free
echo-system-update-started = The update has started
echo-system-update-nowhere = There is nothing here that can start it
system-update-no-output = nothing has been said yet
system-update-verdict-ran = it worked
system-update-verdict-failed = it failed
system-update-verdict-off = switched off
system-update-verdict-skipped = did not apply
system-update-verdict-rehearsed = only rehearsed
system-update-verdict-stopped = it stopped here
system-update-verdict-pending = still to come

## ── anchor: L03 ──

## ── anchor: L05 ──

## ── anchor: L19 ──

# THE 2026-10-05 WAVE: the steps that gather before the root half, migrate,
# verify, reconcile and publish — in the sequence's order — and what the
# view says of the machine's records and of a run on battery.
system-update-step-person-fetch = bring in what you declared elsewhere
system-update-step-person-fetch-does = ronoctua-belongings plain pull: the plain part of your belongings — the programs you declared and declined on your other machines, their records — fetched and merged into this machine's copy, with no passphrase and the encrypted part untouched — and only what your key signed: anything else is held, and the view says why. A file changed here and elsewhere is left as this machine has it, and the view says so. Without a repository, or a network, the step is skipped and says so.
system-update-step-person-pins = move the pins forward
system-update-step-person-pins-does = ronoctua-software pins refresh: every declared program that follows its releases — an AppImage, an Android app from F-Droid — pinned to the version and the digest it has now. Without a declaration there is nothing to move.
system-update-step-person-sign = sign what you declared
system-update-step-person-sign-does = ronoctua-belongings sign: your declarations as they are now — what came from your other machines, the pins just moved — signed on this machine with your key, with no network and no question, so the machine takes them. Without your identity open (no passphrase typed this session), the step declines and the machine keeps the declarations it had until your next sync.
system-update-step-person-stage = hand your declarations to the machine
system-update-step-person-stage-does = ronoctua-software person stage: your declarations put where the machine's narrow applier takes them, validated and swapped into the machine's folder whole — as data, never as code. A rehearsal only checks that they would be taken.
system-update-step-machine-migrate = give the machine its own flake
system-update-step-machine-migrate-does = ronoctua-machine migrate: a machine that still carries a copy of the distro gets its own flake, with the distro as an input — built beside the old folder and swapped in only when it builds; a failure leaves the machine exactly as it was. Every other machine skips it. If it fails, the sequence stops here.
system-update-step-person-verify = try what you declared
system-update-step-person-verify-does = ronoctua-person verify: every program you declared, built on its own against the distro just fetched, so one that no longer builds is held back with its reason instead of taking the whole rebuild down. Its rehearsal tries, and writes nothing.
system-update-step-person-reconcile = bring in what is declared
system-update-step-person-reconcile-does = ronoctua-software reconcile: what the system cannot do for you — the Android apps your declarations list, installed from the machine's copy — and the copy you installed by hand of a program now declared, taken out so the two do not shadow each other. It touches only what it put there itself.
system-update-step-person-record = record what this machine did
system-update-step-person-record-does = ronoctua-person record: this machine's record — its last good system, its boot trial, a hold, how its last apply ended — copied into your belongings' plain part, so your other machines know. Left alone when nothing changed.
system-update-step-person-publish = publish what this machine did
system-update-step-person-publish-does = ronoctua-belongings plain push: the plain part sent to your repository — your declarations and this machine's record — with no passphrase and the encrypted part untouched. Last, so what is sent is what this run ended with.

system-update-needs-file = this file does not exist
system-update-needs-power = the machine is on battery below the floor, and nobody said yes to this run
system-update-needs-free-space = the disk is nearly full, even after collecting the store's garbage
system-update-needs-not-held = the machine is held at a revision: the distro stays where it is

system-update-kind-gather = gathers what you declared
system-update-kind-migrate = migrates the machine's folder
system-update-kind-verify = tries what you declared
system-update-kind-reconcile-declared = brings in what is declared
system-update-kind-publish = publishes what this machine did

system-update-declined-empty = This machine has not synced its belongings yet: there is no key to sign with, and the update goes on.
system-update-declined-no-passphrase = Your identity is not open in this session: nothing was signed, and the machine keeps the declarations it had until your next sync signs them.
system-update-declined-did-not-boot = This is the system that did not start when it was tried: there is nothing new to apply until the distro moves on, and the rest of the update goes on.
system-update-declined-nothing-to-migrate = This machine already has its own flake: there was nothing to migrate, and the update goes on.
system-update-verdict-declined = nothing new

# WHAT THE MACHINE'S RECORDS SAY, above the steps, in both doors — only what
# says something. The boot trial and why there is none are the machine's own
# words (`machine-trial-…`, `machine-no-trial-…`).
system-update-records-conflicts = { $count ->
        [one] One file of your belongings changed here and on another machine, and was left as this machine has it. Open them with { $keys }.
       *[other] { $count } files of your belongings changed here and on another machine, and were left as this machine has them. Open them with { $keys }.
    }
system-update-records-held = { $id } is held back: { $why }. The rebuild leaves it out until it changes.
system-update-held-eval-failed = it does not evaluate
system-update-held-fetch-failed = its download could not be fetched
system-update-held-upstream-changed = its download is there, and it is another file
system-update-held-build-failed = it did not build
system-update-records-hold = This machine is held at { $rev } since { $since }: the update leaves the distro where it is.

# A RUN BY HAND ON A LOW BATTERY asks first, with the no first; a scheduled
# one waits for the charger and asks nobody.
system-update-ask-on-battery = The battery is at { $percent }%, below the { $floor }% an update needs to build the system. Update on battery anyway?
system-update-ask-on-battery-unknown = The machine is running off a battery whose charge cannot be read, and an update builds the system. Update on battery anyway?
system-update-answer-no = no, wait for the charger
system-update-answer-yes = yes, update on battery
echo-system-update-on-battery = The update has started on battery, as you said
echo-system-update-not-started = Not started: plug the machine in, or answer yes

# A RUN BY HAND THAT THE BRAIN HOLDS FOR A METERED WAY OUT asks whether to
# spend this one, with the no first; the no leaves it waiting in the queue.
system-update-ask-metered = This connection is metered — paid by the byte — and the update waits for another before it downloads the new system, which can be gigabytes. Update on this connection anyway?
system-update-answer-no-metered = no, wait for another network
system-update-answer-yes-metered = yes, update on this connection
echo-system-update-on-metered = The update has started on this connection, as you said
echo-system-update-kept-waiting = It stays in the queue, and starts by itself once nothing it waits for is in the way — a metered network, a low battery

# THE 2026-10-11 WAVE: the push to the person's binary cache, last.
system-update-step-binary-cache-push = push to your binary cache
system-update-step-binary-cache-push-does = ronoctua-cachix push: what this machine built, sent to your own Cachix cache — only after an update that switched the machine, and only with your token. A public cache receives only the distro's own outputs, never a path of yours nor an unfree program; a private one receives the system; neither receives what cache.nixos.org already serves. Without a cache named, the step is skipped. Its rehearsal says what would go, and pushes nothing.
system-update-declined-no-token = Your binary cache has no push token on this machine: nothing was pushed. Give it one in the update's binary cache section.
system-update-declined-not-let = A host the push reaches is not allowed in your consent: nothing was pushed. Naming the cache again allows them.
system-update-declined-nothing-new = No update switched this machine since the last push: there is nothing new to push.
system-update-declined-no-cache = This machine names no binary cache of yours: there is nothing to push to.
