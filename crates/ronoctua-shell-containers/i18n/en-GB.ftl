# The texts `ronoctua-shell-containers` says, in en-GB (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

## file info editing — everything the info panel can change

# THE PERSON'S CONTAINERS (LEADER s o) — `containers.rs`, its words
# (`containers_words.rs`), its threads and its window. A status line, a name
# and a log line are podman's own words and are drawn as they came; a key is
# never text here: every sequence arrives as `{ $keys }`.
containers-title = Containers
group-containers = containers
cmd-containers-open = Your containers: pods, compose, images, volumes and networks
cmd-containers-window = Your containers, in a window
containers-down = down
containers-up = up
containers-enter = open
containers-left = left
containers-right = right
containers-next-kind = next kind
containers-previous-kind = previous kind
containers-next-tab = next tab
containers-previous-tab = previous tab
containers-start-stop = start/stop
containers-restart = restart
containers-remove = remove
containers-logs = log
containers-shell = shell
containers-inspect = inspect
containers-pull = pull
containers-build = build
containers-login = log in
containers-prune = prune
containers-reveal = show values
cmd-containers-down = The next row
cmd-containers-up = The row above
cmd-containers-enter = Open the row; on a fact, copy it
cmd-containers-left = The column to the left, or back from the detail
cmd-containers-right = The column to the right, or the detail of the row
cmd-containers-next-kind = The next kind: containers, pods, compose, images, volumes, networks
cmd-containers-previous-kind = The previous kind
cmd-containers-next-tab = The detail's next tab
cmd-containers-previous-tab = The detail's previous tab
cmd-containers-start-stop = Start it, or stop it; a compose project goes up, or down after a question
cmd-containers-restart = Restart it
cmd-containers-remove = Remove it, after a question
cmd-containers-logs = Its log, as it grows
cmd-containers-shell = A shell inside it, in the terminal
cmd-containers-inspect = Everything podman knows of it, highlighted
cmd-containers-pull = Pull an image, on the queue
ask-containers-pull = Image to pull
cmd-containers-build = Build an image from a folder, on the queue
ask-containers-build = Folder to build
cmd-containers-login = Log in to a registry: its name and your user, then the password on a hidden line
ask-containers-login = Registry and user (quay.io alice)
cmd-containers-login-password = The password of the registry login being typed, on a hidden line
ask-containers-login-password = The registry's password
cmd-containers-prune = Remove what nothing uses, after a question
cmd-containers-reveal = Show the environment's values, or hide them again
containers-kind-containers = Containers
containers-kind-pods = Pods
containers-kind-compose = Compose
containers-kind-images = Images
containers-kind-volumes = Volumes
containers-kind-networks = Networks
containers-empty-containers = No containers. A pull puts an image here, and `podman run` or compose makes one.
containers-empty-pods = No pods.
containers-empty-compose = No compose projects. One appears when compose makes its containers.
containers-empty-images = No images.
containers-empty-volumes = No volumes.
containers-empty-networks = No networks.
containers-tab-overview = Overview
containers-tab-logs = Log
containers-tab-stats = Use
containers-tab-inspect = Inspect
containers-tab-ports = Ports
containers-tab-mounts = Mounts
containers-tab-env = Environment
containers-state-created = created
containers-state-running = running
containers-state-paused = paused
containers-state-exited = stopped
containers-state-stopping = stopping
containers-state-unknown = in a state podman did not name
containers-why-no-podman = Containers are not part of this installation: podman is not here.
containers-why-unstartable = podman could not be started.
containers-why-failed = podman could not do it; the system log says why.
containers-why-garbled = podman answered something that is not what was asked.
containers-why-not-found = It is not there any more.
containers-why-refused = The registry refused the login.
containers-why-too-large = podman's answer was too large to show.
containers-reading = Asking podman…
containers-working = { $what } { $name }: { $stage }
containers-pod-value = { $status } · { $count }
containers-project-value = { $count ->
        [one] { $up } of 1 running
       *[other] { $up } of { $count } running
    }
containers-image-unnamed = unnamed ({ $id })
containers-heading-members = Its containers
containers-row-name = Name
containers-row-image = Image
containers-row-state = State
containers-row-status = podman says
containers-row-created = Made
containers-row-started = Started
containers-row-finished = Ended
containers-row-exit = Ended with
containers-row-command = Runs
containers-row-restart = Restarts
containers-row-restarts = Restarted
containers-row-network = Network
containers-row-pod = Pod
containers-row-project = Compose project
containers-row-service = Service
containers-row-id = Id
containers-row-folder = Folder
containers-row-file = Compose file
containers-row-size = Size
containers-row-used-by = Used by
containers-row-platform = Made for
containers-row-driver = Driver
containers-row-mountpoint = Kept in
containers-row-subnet = Addresses
containers-row-dns = Names inside it
containers-used-by = { $count ->
        [0] no container
        [one] one container
       *[other] { $count } containers
    }
containers-network-value = { $network }: { $address }
containers-on = on
containers-off = off
containers-open = open
containers-logs-waiting = Waiting for the log…
containers-logs-empty = It has written nothing.
containers-logs-ended = The log ends here: podman stopped following it.
containers-stats-waiting = Waiting for podman's first count…
containers-stats-not-running = Only a running container uses the machine.
containers-stats-every = Counted again every { $seconds } seconds, while this is shown.
containers-stat-cpu = Processor
containers-stat-memory = Memory
containers-stat-memory-share = Of its memory limit
containers-stat-network = Network in / out
containers-stat-disk = Disk in / out
containers-stat-pids = Processes
containers-ports-none = It publishes no port: nothing outside it reaches it.
containers-port-label = { $published } to { $inside }
containers-port-local = this machine only
containers-port-open = open: the firewall is off
containers-port-allowed = let in
containers-port-home-only = let in at home only
containers-port-shields = refused: shields are up
containers-port-refused = refused by the firewall
containers-port-unknown = rules unreadable
containers-port-firewall = The firewall's rules
containers-ports-blocked = Someone on another machine cannot reach a port the firewall refuses; only this machine can. A rule of yours lets it in.
containers-ports-unknown = The firewall's rules could not be read, so what reaches these ports is not known.
containers-port-narrowed = let in from some addresses only
containers-ports-shields = Shields are up: the firewall drops every new connection from another machine, whatever the rules say, until they are lowered.
containers-ports-narrowed = A rule of yours lets in only the addresses or the connection it names; anyone else cannot reach these ports.
containers-ports-open = The firewall is off: anyone on a network this machine is on reaches these ports.
containers-inspect-env = Environment values
containers-row-nth = { $label } { $n }
containers-mounts-none = It mounts nothing of this machine's.
containers-mount-value = { $source } · { $how } · { $kind }
containers-mount-writable = it can write
containers-mount-read-only = read only
containers-env-none = Its environment is empty.
containers-env-values = Values
containers-env-hidden = hidden
containers-env-shown = shown
containers-env-mask = ••••••
containers-ask-remove = Remove { $name }?
containers-ask-remove-running = It is running: it is stopped first.
containers-ask-remove-pod = Its containers are removed with it.
containers-ask-remove-used = Containers made from it are removed with it.
containers-ask-prune-system = Remove every stopped container and pod, every network no container uses, every image nothing names and the build cache?
containers-ask-prune-images = Remove every image nothing names?
containers-ask-prune-volumes = Remove every volume no container uses — and what is in them?
containers-ask-down = Take { $name } down? Its containers are stopped and removed; its volumes stay.
containers-answer-no = No
containers-answer-yes = Yes
containers-did-start = Starting
containers-did-stop = Stopping
containers-did-restart = Restarting
containers-did-pause = Pausing
containers-did-unpause = Resuming
containers-did-remove = Removing
containers-did-prune = Pruning
containers-echo-asked = { $what } { $name }…
containers-echo-done = { $what } { $name }: done.
containers-echo-refused = { $what } { $name }: { $why }
containers-echo-queued = { $what } { $name } went to the queue — { $keys } shows it.
containers-no-queue = The transfer queue is not answering, so nothing was queued.
containers-refused-not-here = That is not something one does to { $kind }.
containers-refused-not-running = Only a running container has a shell to open.
containers-refused-no-file = compose did not say where this project's file is.
containers-refused-no-folder = { $path } is not a folder.
containers-refused-no-containerfile = { $path } has no Containerfile or Dockerfile to build.
containers-refused-login-words = Type the registry and your user on it, a space between them: quay.io alice.
containers-refused-no-login = No login is waiting for a password: start with the registry and the user.
containers-echo-logging-in = Logging in to { $registry } as { $user }…
containers-echo-logged-in = Logged in to { $registry } as { $user }.
containers-echo-login-refused = The login to { $registry } did not go through: { $why }
transfers-work-container-pull = pull the image
transfers-work-container-build = build the image
transfers-work-container-compose-up = start the project
transfers-work-container-compose-down = take down the project
transfers-work-container-compose-pull = pull the project's images
