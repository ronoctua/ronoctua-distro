# The system-update module's own texts: its name and the line under it, its
# commands, the name and the sentence of every step of the sequence, the
# closed words the sequence's table carries, and the view that watches a run.
brain-task-system-update = Update the system
brain-task-system-update-does = Updates the whole machine: it gathers what you declared on your other machines, moves the distro and nixpkgs forward, and builds the new system for the next restart — nothing running now is stopped or restarted. It also updates your programs and caches, frees space in the store, and publishes what this machine did. On the next restart the new system is tried once; if it does not start, the machine goes back to the last good one by itself. When a new system is waiting, the run ends by asking you to restart.
brain-task-system-update-brief = Updates the distro, nixpkgs and your programs, and sets the new system for the next restart.

# ITS COMMANDS. The whole sequence now, and the two doors of the view that
# watches it — the footer's region and a window.
cmd-system-update = Update the system
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
system-update-step-flake-update-does = Moves the distro and nixpkgs to their newest versions; everything else stays as tested. On a machine that follows a local checkout of the distro, only nixpkgs moves. Skipped while the machine is held, or when the battery or the disk would stop the rebuild.
system-update-step-system-rebuild = rebuild the system
system-update-step-system-rebuild-does = Builds the new system and sets it for the next restart, so nothing running now stops. If the build fails, the machine stays as it was and the update stops here.
system-update-step-parts-plan = plan the parts
system-update-step-parts-plan-does = Records which parts this system and the new one need, like programs that are not free, so the clean-up later keeps them. It never waits for a download.
system-update-step-flatpak-system = update the system's Flatpaks
system-update-step-flatpak-system-does = Updates the Flatpaks installed for the whole machine, without installing new ones. Your own Flatpaks are updated in a later step.
system-update-step-android-images = update Android's system image
system-update-step-android-images-does = Updates the system your Android apps run on, about a gigabyte, and checks the download. It waits while Android is open, and is off unless this machine's configuration turns it on.
system-update-step-old-generations = trim the system's old generations
system-update-step-old-generations-does = Removes the system's oldest generations but keeps the newest, the running one and all the boot menu lists, so you can always go back. Your own Nix programs are untouched.
system-update-step-store-collect-garbage = free space in the store
system-update-step-store-collect-garbage-does = Deletes from the store whatever nothing uses any more. It never removes a generation; the step before decides which ones go.
system-update-step-store-optimise = deduplicate the store
system-update-step-store-optimise-does = Keeps a single copy of identical files in the store to save space. It runs after the clean-up, so it does not work on what is about to be deleted.
system-update-step-journal-vacuum = trim the system log
system-update-step-journal-vacuum-does = Deletes the oldest entries of the system log, down to a fixed size. It is off, because it would also delete the record of why an update failed.
system-update-step-font-cache = rebuild the font cache
system-update-step-font-cache-does = Rebuilds the font cache from scratch for the running system, so programs find the right fonts. The new system's fonts are cached at the first update after the restart.
system-update-step-tldr-cache = update the tldr pages
system-update-step-tldr-cache-does = Downloads the latest tldr pages, short examples of how to use a command, into your home folder. Without a network it is skipped instead of failing.
system-update-step-nix-profile = update your Nix programs
system-update-step-nix-profile-does = Updates the programs in your Nix profile to the running system's nixpkgs, without installing new ones. The new system's nixpkgs reaches them at the first update after the restart.
system-update-step-flatpak-user = update your Flatpaks
system-update-step-flatpak-user-does = Updates your own Flatpak apps, the runtimes they use and the Flathub catalogue, without installing new ones.
system-update-step-appimages = update your AppImages
system-update-step-appimages-does = Looks for a newer release of each AppImage in your Applications folder and swaps the file in one go, so a failed download leaves the old one working.
system-update-step-android-apps = update your Android apps
system-update-step-android-apps-does = Updates the Android apps that came from F-Droid through this system, checking who signed each one. If Android is open, nothing is replaced and the apps wait for the next update.
system-update-step-rust-toolchains = update the Rust toolchains
system-update-step-rust-toolchains-does = Updates the Rust toolchains you installed yourself in your home folder, without adding any. Rust already comes with the system, so this is for a machine where you set it up by hand.
system-update-step-node-global = update npm's global packages
system-update-step-node-global-does = Updates the npm packages you installed globally, without adding any. Node comes with the system but those packages do not, so without them the step is skipped.
system-update-step-pnpm-global = update pnpm's global packages
system-update-step-pnpm-global-does = Updates the pnpm packages you installed globally. pnpm keeps them apart from npm's, which is why this is a separate step.
system-update-step-python-tools = update the uv tools
system-update-step-python-tools-does = Updates the Python tools you installed with uv, without adding any. uv comes with the system but those tools do not, so without them the step is skipped.
system-update-step-flatpak-unused = remove the Flatpak runtimes nothing uses
system-update-step-flatpak-unused-does = Removes the Flatpak runtimes and extensions that none of your apps needs any more, left behind by updates. Your apps are never touched.
system-update-step-nix-profile-history = forget the old Nix profile generations
system-update-step-nix-profile-history-does = Forgets the old generations of your Nix profile, which keep old programs in the store. It is off, so nothing automatic takes away your way back; the software view offers it when you ask.
system-update-step-appimage-cache = clear what AppImages left behind
system-update-step-appimage-cache-does = Deletes the unpacked copies of AppImages that are gone or were replaced, which nothing else cleans up. The AppImages themselves are never touched.

# WHY A STEP WAS SKIPPED. The path or the name it is about travels beside the
# sentence as data (`Needs::about()`), which is why none of these names one.
system-update-needs-always = Nothing can keep this step from applying.
system-update-needs-binary = The program it runs is not installed for you.
system-update-needs-directory = A folder it needs is not on this machine.
system-update-needs-writable-file = A file it has to change is missing, or cannot be changed.
system-update-needs-network = This machine is not connected to a network.

# THE VIEW THAT WATCHES A RUN, in both doors: its keys and what each says under
# the pointer, `update now` (the key the view teaches for the whole sequence),
# what starting said, and the word beside each step.
system-update-next-step = next
about-system-update-next-step = Moves down one line: the status, the actions and the records' choices, then the steps. Says what the line is.
system-update-previous-step = previous
about-system-update-previous-step = Moves up one line, back from the steps to the actions and the status. Says what the line is.
system-update-section = section
about-system-update-section = Jumps between the lines at the top and the steps, each keeping its place.
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

# THE TOP OF THE VIEW, in both doors: one line that says where the update
# stands — the brain's run first, then what waits for you, then what GitHub
# said of the distro's and nixpkgs' branches —, the actions under it, and what
# the focused line is. A countdown's answers and the stop are taught only
# while they mean something.
system-update-proceed-does = Starts the update that is counting down now, without waiting for the countdown.
system-update-abort-does = Puts off the update that is counting down until its next scheduled time.
system-update-stop = stop the update
about-system-update-stop = Stops the update that is running. The steps it already did stay done.
system-update-start-does = Starts a full update now, after a short countdown that you can skip or cancel.
system-update-start-queued = The update is already in the queue, so there is nothing to start. It begins by itself as soon as nothing holds it.
system-update-status-running = An update is running now.
system-update-status-counting-down = An update is about to start: the countdown is on.
system-update-status-queued = An update is in the queue, and starts when the machine is free.
system-update-status-waiting-ram = An update is waiting: the memory is busy.
system-update-status-waiting-cpu = An update is waiting: the processor is busy.
system-update-status-waiting-battery = An update is waiting for the charger.
system-update-status-waiting-busy = An update is waiting for another task of the brain to finish.
system-update-status-waiting-network = An update is waiting for a network that does not charge by the amount of data.
system-update-status-waiting-asleep = An update is waiting for the machine to wake up.
system-update-status-restart = A new system is ready: restart to use it.
system-update-status-restart-unguarded = A new system is ready: restart to use it. If it does not start, only the boot menu takes you back.
system-update-status-failed = The last update failed. The steps below show where.
system-update-status-held = This machine is held at one version, so updates leave the distro and nixpkgs as they are.
system-update-status-checking = Checking whether an update is available…
system-update-status-available-distro = An update is available: the distro has a new version.
system-update-status-available-nixpkgs = An update is available: nixpkgs has new versions.
system-update-status-available-both = An update is available: the distro and nixpkgs have new versions.
system-update-status-up-to-date = The system is up to date.
system-update-status-unchecked = Could not check for updates: { $why }.
system-update-status-unasked = Not checked for updates yet.
system-update-status-last = Last updated { $when }.
system-update-unchecked-offline = this machine has no network
system-update-unchecked-rate-limited = GitHub asked to wait a while
system-update-unchecked-other = GitHub did not answer as expected
system-update-unchecked-no-lock = this machine has no flake of its own
system-update-unchecked-unreadable = this machine's flake.lock cannot be read
system-update-unchecked-nothing = this machine's flake follows no branch on GitHub
system-update-unchecked-unasked-distro = the distro does not follow a named branch on GitHub, so only an update can tell
system-update-unchecked-unasked-nixpkgs = nixpkgs does not follow a named branch on GitHub, so only an update can tell
system-update-status-does = Whether the distro or nixpkgs has moved past what this machine runs, as GitHub says. Choosing it asks again.
system-update-status-next = The next scheduled update is { $when }.
system-update-register-belongings = Choosing it opens your belongings, where the files changed on two machines are.
system-update-register-asks = Choosing it asks you first; nothing changes until you answer yes.

## ── anchor: L03 ──

## ── anchor: L05 ──

## ── anchor: L19 ──

# THE 2026-10-05 WAVE: the steps that gather before the root half, migrate,
# verify, reconcile and publish — in the sequence's order — and what the
# view says of the machine's records and of a run on battery.
system-update-step-person-fetch = bring in what you declared elsewhere
system-update-step-person-fetch-does = Brings in the programs you declared or turned down on your other machines, from your belongings. Only what your key signed comes in; without a belongings repository, it is skipped.
system-update-step-person-pins = look for new versions of your programs
system-update-step-person-pins-does = Notes the newest version of each declared program that follows its releases, like an AppImage or an F-Droid app. Unless you turned it off, the distro's own such apps go past the tested versions too.
system-update-step-person-sign = sign what you declared
system-update-step-person-sign-does = Signs your declarations with your key, so this machine accepts them. If you have not typed your passphrase this session, what changed here waits for your next sync.
system-update-step-person-stage = hand your declarations to the machine
system-update-step-person-stage-does = Hands your declarations to the machine, which checks them and accepts them as data, never as code. If it refuses them, the update builds with the ones it already had.
system-update-step-machine-migrate = give the machine its own flake
system-update-step-machine-migrate-does = Gives a machine that still carries a copy of the distro its own flake, switching only once it builds. Other machines skip this; if it fails, nothing changes and the update stops here.
system-update-step-person-reconcile = bring in what is declared
system-update-step-person-reconcile-does = Installs the Android apps your declarations list, and takes out a copy you installed by hand of a program that is now declared, so the two do not clash.
system-update-step-person-record = record what this machine did
system-update-step-person-record-does = Saves this machine's status, like its last good system and how its last system change went, in your belongings for your other machines to see. Nothing is written if nothing changed.
system-update-step-person-publish = publish what this machine did
system-update-step-person-publish-does = Sends your declarations and this machine's status to your belongings repository, without asking for your passphrase. It runs after every step that changes them, so it sends what this update ended with.

system-update-needs-file = A file it needs is not on this machine.
system-update-needs-power = The battery is too low for this step, and nobody agreed to run this update on battery.
system-update-needs-free-space = The disk is nearly full, even after freeing space in the store.
system-update-needs-not-held = This machine is held at one version, so the distro and nixpkgs stay where they are.

system-update-declined-empty = This machine has not synced your belongings yet, so there is no key to sign with. The update goes on.
system-update-declined-no-passphrase = You have not typed your passphrase this session, so nothing was signed here. What your other machines signed still arrives; a change made here waits for your next sync.
system-update-declined-busy = Another change to this machine was running, so this step changed nothing. The update goes on with what the machine already had.
system-update-declined-did-not-boot = This is the system that did not start when it was tried. There is nothing new to build until the distro or nixpkgs moves on, and the rest of the update goes on.
system-update-declined-nothing-to-migrate = This machine already has its own flake, so there was nothing to do. The update goes on.
system-update-verdict-declined = nothing new

# WHAT THE MACHINE'S RECORDS SAY, above the steps, in both doors — only what
# says something. The boot trial and why there is none are the machine's own
# words (`machine-trial-…`, `machine-no-trial-…`).
system-update-records-conflicts = { $count ->
        [one] One file of your belongings changed both here and on another machine, and this machine's version was kept. Open it with { $keys }.
       *[other] { $count } files of your belongings changed both here and on another machine, and this machine's versions were kept. Open them with { $keys }.
    }
system-update-records-hold = Held at { $rev } since { $since }.
system-update-records-staged = A new system is ready for the next restart; until then the machine keeps the current one. That restart tries the new one once.
system-update-records-staged-unguarded = A new system is ready for the next restart; until then the machine keeps the current one. If it does not start, only the boot menu takes you back.
system-update-records-checkout = This machine follows a local checkout of the distro, so updates move only nixpkgs. Commits made there reach the system only through an action you approve with sudo.

# A RUN BY HAND ON A LOW BATTERY asks first, with the no first; a scheduled
# one waits for the charger and asks nobody.
system-update-ask-on-battery = The battery is at { $percent }%, below the { $floor }% needed to build the system. Update on battery anyway?
system-update-ask-on-battery-unknown = The machine is on battery and its charge cannot be read, and an update builds the whole system. Update on battery anyway?
system-update-answer-no = no, wait for the charger
system-update-answer-yes = yes, update on battery
echo-system-update-on-battery = The update has started on battery, as you said
echo-system-update-not-started = Not started: plug the machine in, or answer yes

# A RUN BY HAND THAT THE BRAIN HOLDS FOR A METERED WAY OUT asks whether to
# spend this one, with the no first; the no leaves it waiting in the queue.
system-update-ask-metered = This connection charges by the amount of data, and the new system can be gigabytes, so the update waits for another network. Update on this connection anyway?
system-update-answer-no-metered = no, wait for another network
system-update-answer-yes-metered = yes, update on this connection
echo-system-update-on-metered = The update has started on this connection, as you said
echo-system-update-kept-waiting = It stays in the queue and starts by itself once no metered network or low battery is in the way

# THE 2026-10-11 WAVE: the push to the person's binary cache, last.
system-update-step-binary-cache-push = push to your binary cache
system-update-step-binary-cache-push-does = Pushes a new system this machine built to your own Cachix cache, using your token. A public cache never gets anything of yours or a program that is not free.
system-update-declined-no-token = Your binary cache has no push token on this machine, so nothing was pushed. Add one on the update's binary cache page.
system-update-declined-not-let = A site the push needs is not allowed, so nothing was pushed. Naming the cache again allows its sites.
system-update-declined-nothing-new = No update has built a new system since the last push, so there is nothing new to push.
system-update-declined-no-cache = This machine has no binary cache of yours set up, so there is nowhere to push.

# THE RUN THAT SET A NEW SYSTEM FOR THE NEXT BOOT ends saying so, once —
# promising the boot trial's way back only when one is armed.
notif-system-update-restart-summary = Restart to use the new system
notif-system-update-restart-body = The update built a new system and set it for the next restart: nothing that runs now was stopped. Until you restart, this machine runs the one it ran; that restart tries the new one once, and comes back to the last good one by itself if it does not start.
notif-system-update-restart-unguarded-summary = Restart to use the new system
notif-system-update-restart-unguarded-body = The update built a new system and set it for the next restart: nothing that runs now was stopped. Until you restart, this machine runs the one it ran; that restart has no way back but the boot menu: if the new one does not start, choose the previous one there.

# WHY THE MACHINE'S OWN PROGRAM (`ronoctua-machine`) DID NOT DO WHAT IT WAS
# ASKED — an apply, a migration, a bless or a hold: on the step under the
# cursor in both doors, in the gestures of the view, and as the reason of a
# run of this task that stopped at one of its steps.
system-update-machine-refused-not-root = Only root can change the machine, so nothing was done.
system-update-machine-refused-busy = Another change to this machine is running. Try again when it ends.
system-update-machine-refused-nothing-to-migrate = This machine already has its own flake, so there is nothing to do.
system-update-machine-refused-build-failed = The new system could not be built, so nothing changed. If a download failed, the parts page names it and shows how to bring it in.
system-update-machine-refused-switch-failed = Switching to the new system failed. The next restart tries it once, and goes back to the last good system if it does not start.
system-update-machine-refused-next-boot-failed = The new system could not be set for the next restart. Nothing running changed, but the boot menu may not list it, and the next restart may start the current system.
system-update-machine-refused-did-not-boot = This is the system that did not start when it was tried, or that was undone. The machine stays on its current system until a newer distro arrives, or until you ask on this page to try it again.
system-update-machine-refused-last-good-would-fall-out = One more system would push the last good one off the boot menu. Restart first, so the system on trial can be confirmed.
system-update-machine-refused-held = This machine is held at a different version than the one it is set up with, so nothing was built.
system-update-machine-refused-no-failed-trial = No system has failed its boot trial on this machine, so there is nothing to try again.
system-update-machine-refused-checkout-moved = The distro was left where it was: on a machine that follows a local checkout, only an action you approve with sudo moves it.
system-update-machine-refused-unreadable = A file of this machine's configuration cannot be read.
system-update-machine-refused-failed = A program the machine needed failed. The system log says which.

# ROOT'S APPLIER REFUSED YOUR DECLARATIONS: its reason, as a sentence — on the
# step under the cursor, in both doors, and in the run's notification.
notif-system-update-declarations-refused-summary = Your declarations did not reach this machine
notif-system-update-declarations-refused-body = The machine's applier refused them: { $why } This update built the ones the machine already had.
system-update-applier-refused-unknown = This version cannot name the reason; the system log has it.
system-update-applier-refused-nothing-staged = There were no declarations to hand over.
system-update-applier-refused-link = Your declarations contain a symbolic link, so the machine accepts none of them.
system-update-applier-refused-hard-link = A file in your declarations has a second name, a hard link, so the machine accepts none of them.
system-update-applier-refused-special = Your declarations contain a pipe, a socket or a device, so the machine accepts none of them.
system-update-applier-refused-misplaced = In your declarations, a file stands where a folder belongs, or a folder where a file does.
system-update-applier-refused-not-owned = Something in your declarations does not belong to you, so the machine accepts none of them.
system-update-applier-refused-writable = Someone else can change something in your declarations, so the machine accepts none of them.
system-update-applier-refused-bad-name = A name in your declarations is not plain text.
system-update-applier-refused-not-a-module = A folder among your brain modules is not named my- followed by one word.
system-update-applier-refused-too-large = A file in your declarations, or all of them together, is bigger than the machine accepts.
system-update-applier-refused-too-many = Your declarations have more entries, icons, modules or files than the machine accepts.
system-update-applier-refused-too-deep = Your declarations have more folders inside folders than the machine accepts.
system-update-applier-refused-not-png = An icon in your declarations is not a PNG.
system-update-applier-refused-missing-icon = An entry in your declarations names an icon that did not come with them.
system-update-applier-refused-unreadable = Something in your declarations could not be read.
system-update-applier-refused-unwritable = The machine could not write to its own folder, or someone besides root can change it.
system-update-applier-refused-restaged = Your declarations changed again while the machine was reading them. The next update takes them.
system-update-applier-refused-not-a-nonce = The mark that tells one hand-over from the next could not be read.
system-update-applier-refused-not-signed = Your declarations are not signed, and this machine accepts only what your key signed.
system-update-applier-refused-bad-signature = Your declarations were not signed by the key this machine trusts for you.
system-update-applier-refused-rolled-back = These declarations are older than the last ones this machine accepted, and an older copy is never accepted again.
system-update-applier-refused-bad-key = The key this machine trusts for you is not a valid key.
# THE 2026-10-30 WAVE: the Android apps that wait, and why.
system-update-declined-android-in-use = Android was open, so none of its apps was replaced while you used it. They wait for the next update.
system-update-declined-fdroid-not-let = F-Droid is not allowed, so your Android apps were not checked and keep their versions. Allow f-droid.org and the next update moves them.
