# The system-update module's own texts: its name and the line under it, its
# commands, the name and the sentence of every step of the sequence, the
# closed words the sequence's table carries, and the view that watches a run.
brain-task-system-update = Update the system
brain-task-system-update-does = Moves the pinned inputs forward, rebuilds the system and switches to it, and reclaims store space. The switch restarts the services whose definition changed; programs already open keep the version they started with, and a new kernel waits for the next reboot.

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
system-update-step-flake-update-does = nix flake update on /etc/nixos: moves the pinned inputs forward and touches nothing but flake.lock. It needs the network, and on the ISO /etc/nixos is read-only — there the step is skipped and says so.
system-update-step-system-rebuild = rebuild the system
system-update-step-system-rebuild-does = nixos-rebuild switch, through the unit that already exists: it compiles, and it can bring a kernel that has never booted. The way back is the previous generation, in the boot menu. If it fails, the sequence stops here.
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
