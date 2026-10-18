# The texts `ronoctua-shell-incus` says, in en-GB (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

incus-refused-no-incus = Incus is not part of this installation, or its daemon is not running.
incus-refused-not-in-group = Incus answers only the incus-admin group; log out and in again for the group to count.
incus-refused-gone = It is not there any more.
incus-refused-busy = Incus is still busy with it; try again when it is done.
incus-refused-not-let = That server, or a host it fetches from, has not been allowed yet; allow them first.
incus-refused-invalid = Incus does not take that name, path or limit — or the name is taken, or the file is already there.
incus-refused-failed = Incus could not do it; the system log says why.
# INCUS (LEADER s x) — `incus.rs`, its words (`incus_words.rs`), its threads
# and its window. A name, an alias, an address, an image's description and a
# snapshot's name are the daemon's own words and are drawn as they came; a key
# is never text here: a sequence arrives as `{ $keys }`.
group-incus = Incus
incus-title = Incus
cmd-incus-open = Incus: your containers and virtual machines
cmd-incus-window = Incus, in a window
cmd-incus-new = A new Incus container or virtual machine
cmd-incus-images = Incus's images, here and in the catalogues
incus-down = down
incus-up = up
incus-enter = open
incus-left = left
incus-right = right
incus-next-kind = next kind
incus-previous-kind = previous kind
incus-next-tab = next tab
incus-previous-tab = previous tab
incus-launch = new
incus-start-stop = start/stop
incus-restart = restart
incus-freeze = freeze/thaw
incus-delete = delete
incus-console = console
incus-screen = screen
incus-shell = shell
incus-push = send in
incus-pull = take out
incus-export = export
incus-import = import
incus-copy-image = keep a copy
cmd-incus-down = The next row
cmd-incus-up = The row above
cmd-incus-enter = Open the row; on a fact, copy it
cmd-incus-left = The column to the left, or back from the detail
cmd-incus-right = The column to the right, or the detail of the row
cmd-incus-next-kind = The next kind: instances, images, remotes, profiles, networks, storage, projects
cmd-incus-previous-kind = The previous kind
cmd-incus-next-tab = The instance's next tab
cmd-incus-previous-tab = The instance's previous tab
cmd-incus-launch = A new instance: its kind, name, image and limits, then launched on the queue
cmd-incus-start-stop = Start it, or stop it
cmd-incus-restart = Restart it
cmd-incus-freeze = Freeze it, or thaw it
cmd-incus-delete = Delete it — an instance, an image or a snapshot — after a question
cmd-incus-console = Its text console, in the terminal
cmd-incus-screen = A virtual machine's screen, in remote-viewer
cmd-incus-shell = A shell inside it, in the terminal
cmd-incus-push = Send a file into it, on the queue
ask-incus-push = File to send in
cmd-incus-push-to = Where inside the file sent goes
ask-incus-push-to = Where inside
cmd-incus-pull = Take a file out of it, on the queue
ask-incus-pull = Path inside to take out
cmd-incus-pull-to = Where here the file taken out goes, never over another
ask-incus-pull-to = Save it as
cmd-incus-export = Export it to a file, on the queue
ask-incus-export = Export to a file that is not there yet
cmd-incus-import = Import an instance from a file, on the queue
ask-incus-import = File to import
cmd-incus-copy-image = Keep a copy of the catalogue's image here, on the queue
cmd-incus-name = The new instance's name
ask-incus-name = Name of the new instance
cmd-incus-image = The new instance's image
ask-incus-image = Image (remote:alias, or a local alias)
cmd-incus-cpu = Its processors
ask-incus-cpu = Processors (nothing typed: no limit)
cmd-incus-memory = Its memory
ask-incus-memory = Memory, with its unit (nothing typed: no limit)
cmd-incus-processes = Its processes
ask-incus-processes = Processes (nothing typed: no limit)
cmd-incus-snapshot = A snapshot of it, by a name
ask-incus-snapshot = Name of the snapshot
cmd-incus-publish = Publish it as an image here, on the queue
ask-incus-publish = Alias of the new image
incus-kind-instances = Instances
incus-kind-images = Images
incus-kind-remotes = Remotes
incus-kind-profiles = Profiles
incus-kind-networks = Networks
incus-kind-storage = Storage
incus-kind-projects = Projects
incus-empty-instances = No instances yet: a new one is made from an image, here or in a remote's catalogue.
incus-empty-images = No images here. A remote's catalogue keeps a copy of one, and a launch keeps the one it used.
incus-empty-remotes = No remotes: the client knows no server to take images from.
incus-empty-profiles = No profiles.
incus-empty-networks = No networks.
incus-empty-storage = No storage pools.
incus-empty-projects = No projects.
incus-projects-note = This view works in the default project; the incus command reaches the others.
incus-tab-overview = Overview
incus-tab-snapshots = Snapshots
incus-tab-limits = Limits
incus-tab-actions = Actions
incus-status-running = running
incus-status-stopped = stopped
incus-status-frozen = frozen
incus-status-error = in error
incus-status-other = changing
incus-machine-container = container
incus-machine-vm = virtual machine
incus-list-address = { $status } · { $address }
incus-reading = Asking Incus…
incus-events-lost = Incus's events stopped (the daemon restarted?), so a change made elsewhere may not show yet; they are opened again on their own.
incus-working = { $what } { $name }: { $stage }
incus-row-name = Name
incus-row-kind = Kind
incus-row-status = State
incus-row-ipv4 = IPv4 address
incus-row-ipv6 = IPv6 address
incus-row-cpu = Processor time
incus-row-memory = Memory in use
incus-row-processes = Processes running
incus-row-disk = Disk in use
incus-row-image = Made from
incus-row-profiles = Profiles
incus-row-made = Made
incus-row-project = Project
incus-row-description = Description
incus-row-alias = Alias
incus-row-size = Size
incus-row-fingerprint = Fingerprint
incus-row-address = Address
incus-row-protocol = Speaks
incus-row-type = Type
incus-row-managed = Managed by Incus
incus-row-driver = Driver
incus-row-nth = { $label } { $n }
incus-yes = yes
incus-no = no
incus-seconds = { $count ->
        [one] 1 second
       *[other] { $count } seconds
    }
incus-minutes = { $count ->
        [one] 1 minute
       *[other] { $count } minutes
    }
incus-hours = { $hours } h { $minutes } min
incus-snapshot-take = Take a snapshot
incus-snapshot-take-value = by a name
incus-snapshots-none = No snapshots yet.
incus-limit-cpu = Processors
incus-limit-memory = Memory
incus-limit-processes = Processes
incus-limit-none = no limit
incus-limit-from-profile = { $value }, from a profile
incus-limit-cpus = { $count ->
        [one] 1 processor
       *[other] { $count } processors
    }
incus-limit-count = { $count ->
        [one] 1 process
       *[other] { $count } processes
    }
incus-limits-note = A limit set here is the instance's own and wins over its profiles'; answering with nothing typed takes the instance's own away. A limit that comes from a profile is changed in the profile.
incus-limits-vm-processes = A virtual machine has no limit of processes: its own kernel counts them.
incus-use-console = Text console
incus-use-screen = Screen
incus-use-shell = Shell
incus-use-push = Send a file in
incus-use-pull = Take a file out
incus-use-export = Export to a file
incus-use-publish = Publish as an image
incus-use-in-terminal = in the terminal
incus-use-in-viewer = in remote-viewer
incus-use-on-queue = on the queue
incus-use-not-running = only while it runs
incus-image-launch = Launch an instance from it
incus-image-launch-value = a new instance
incus-heading-catalogue = Its images
incus-catalogue-reading = Asking { $remote } for its images…
incus-catalogue-empty = It offers nothing for this machine.
incus-offered-value = { $kind } · { $size }
incus-consent-catalogue = The remote's list of images comes from there, and so do the images an instance is made from.
incus-consent-launch = The image the new instance is made from comes from there.
incus-protocol-simplestreams = simplestreams
incus-protocol-oci = an OCI registry
incus-heading-volumes = Its volumes
incus-volumes-none = No volumes.
incus-new-title = A new instance
incus-field-machine = Kind
incus-field-name = Name
incus-field-image = Image
incus-unchosen = not chosen yet
incus-image-hint = The image is remote:alias — the remotes here are { $remotes } — or the alias of an image here.
incus-create = Launch it
incus-create-value = made and started on the queue
incus-ask-delete = Delete { $name }?
incus-ask-delete-running = It is running: it is stopped first.
incus-ask-delete-snapshots = Its snapshots go with it.
incus-ask-delete-image = Delete the image { $name }?
incus-ask-delete-snapshot = Delete the snapshot { $snapshot } of { $name }?
incus-ask-restore = Restore { $name } to { $snapshot }? Whatever changed in it since is lost.
incus-answer-no = No
incus-answer-yes = Yes
incus-did-start = Starting
incus-did-stop = Stopping
incus-did-restart = Restarting
incus-did-freeze = Freezing
incus-did-unfreeze = Thawing
incus-did-delete = Deleting
incus-did-limits = Setting the limits of
incus-did-snapshot = Taking the snapshot
incus-did-restore = Restoring
incus-did-delete-snapshot = Deleting the snapshot
incus-did-delete-image = Deleting the image
incus-echo-asked = { $what } { $name }…
incus-echo-done = { $what } { $name }: done.
incus-echo-refused = { $what } { $name }: { $why }
incus-echo-queued = { $what } { $name } went to the queue — { $keys } shows it.
incus-no-queue = The transfer queue is not answering, so nothing was queued.
incus-refused-not-here = That is not something one does to { $kind }.
incus-refused-not-running = It is not running.
incus-refused-not-vm = Only a virtual machine has a screen.
incus-refused-unchosen = Choose a name and an image first.
incus-refused-name = { $name } is not a name Incus takes: letters, digits and hyphens, a letter first.
incus-refused-name-taken = An instance is already called { $name }.
incus-refused-no-image = No remote image and no image here is called { $image }.
incus-refused-number = { $said } is not a whole number above zero.
incus-refused-size = { $said } is not a size: say its unit, as in 512MiB or 2GiB.
incus-refused-snapshot-name = { $name } is not a snapshot name Incus takes: no spaces and no slashes.
incus-refused-no-file = { $path } is not a file here.
incus-refused-inside = { $path } is not a path inside it: one starts with a slash.
incus-refused-exists = { $path } is already there; nothing is written over it.
incus-refused-no-folder = The folder of { $path } is not there.
incus-refused-alias = { $alias } is not an alias Incus takes: no spaces.
incus-refused-limit-profile = { $limit }: this limit of { $name } comes from a profile, and { $name } has none of its own to take away.
