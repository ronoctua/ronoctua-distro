# The texts `ronoctua-installer` says, in en-GB (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

## Installer — the six questions (section 7.1)

inst-title = Install ronoctua
inst-disk = Target disk
inst-disk-help = Will be erased entirely, with no recovery.
inst-hostname = Machine name
inst-hostname-help = Starts with `noctua-`. Lowercase, digits and hyphen.
inst-password = User password
inst-password-help = To log in once the system is installed.
inst-passphrase = Disk passphrase
inst-passphrase-help = Unlocks the disk when the TPM refuses. Keep it off the computer.
inst-language = Language
inst-language-help = For the interface and the keyboard.
inst-keyboard = Keyboard layout
inst-keyboard-help = Pick the one your keyboard is. It is what you will type the passphrase on.
inst-keyboard-variant = Layout variant
inst-keyboard-variant-help = Tells apart keyboards of the same country. No variant is a valid answer.
inst-compose-key = Accent key
inst-compose-key-help = The Compose key: press and release it, then two keys, and out comes the accent. Without it, a keyboard with no accent keys cannot reach them. The default is { $compose }; { $toggle } turns { $capslock } on and off.
inst-system-locale = System formats
inst-system-locale-help = Date, number and units. Not the interface language, which is already chosen.
inst-timezone = Time zone
inst-timezone-help = Where the machine is. It decides the clock and the log's timestamps.
inst-profile = Installation
inst-profile-help = `full` brings everything and is the recommended one. `minimal` leaves out Android, the toolchains other than Rust and Common Lisp, the browser, the editors, media, the office suites, the Crafting Apps and the font variants — it installs in minutes, and each group comes back with one line in host.nix.
inst-resume-offset = Swap offset
inst-resume-offset-help = Computed once the disk exists; not typed.

## Installer — the steps

inst-step-key = Hand the passphrase to disko
inst-step-partition = Partition and encrypt the disk
inst-step-flake = Copy the flake to the target system
inst-step-host-nix = Write this machine's description
inst-step-facter = Detect the hardware
inst-step-password = Write the password hash to /persist
inst-step-install = Build and install the system
inst-step-zone = Set this machine's time zone
inst-step-target = Make the target system's configuration folder

## Installer — the interface

inst-review = Review before running
inst-destructive = The marked steps erase the disk
inst-confirm = To confirm, type the disk path
inst-confirm-error = Does not match the chosen disk
inst-to-apply = For real, with the disk erased: --execute
inst-dryrun = Dry run: nothing was touched
inst-running = Installing
inst-done = Install finished. Remove the drive and reboot.
inst-failed = The install stopped at this step
inst-no-disk = No disk found on this machine
inst-keys-edit = { $enter } continues · { $esc } goes back · { $quit } quits
inst-keys-choose = { $arrows } choose · { $enter } continues · { $esc } goes back
inst-keys-review = { $enter } runs · { $esc } goes back · { $quit } quits
inst-keys-quit = { $quit } quits

## Installer — the seventh question (network)

inst-wifi = Wi-Fi network
inst-wifi-help = The password stays on the installed machine, never in the repository.
inst-wifi-none = no Wi-Fi — I will use a cable
inst-wifi-scanning = Looking for networks…
inst-wifi-empty = No networks in range
inst-wifi-password = Network password
inst-wifi-password-short = a WPA network password is at least 8 characters
inst-step-seal-wifi = Seal the Wi-Fi password in the TPM
inst-step-wifi = Write the Wi-Fi network profile

## file info editing — everything the info panel can change

# The installer's fifteenth question.
inst-belongings = Belongings repository
inst-belongings-help = The private repository your configuration is kept in, encrypted: git@host:path, ssh://… or https://…, never with a password in it. Your declarations are installed with the machine, and your settings restored into its home.
inst-step-belongings = Write the belongings' repository into your home
inst-step-plain-passphrase = Hand the passphrase to the copy of your declarations
inst-step-pin = Pin your signing key on this machine
inst-step-cache-token = Read your private binary cache's token for the installation
inst-step-cache-netrc = Let this machine read your private binary cache
inst-belongings-access = Access to the repository
inst-belongings-access-help = This machine reaches the repository with its own key, made now and kept only in your home. It is asked again until it reads and writes.
inst-belongings-token-help = Type a token that reads and writes the repository. It stays in your home, readable by you alone.
inst-belongings-passphrase = Belongings passphrase
inst-belongings-passphrase-help = The passphrase that opens your belongings. It is checked against the repository before anything is touched.
inst-belongings-passphrase-new-help = The repository is empty: choose the passphrase that will seal your belongings, at least 12 characters. It is typed twice.
inst-belongings-passphrase-again = Type the passphrase again
inst-belongings-passphrase-differs = The two passphrases differ. Type it again from the start.
inst-belongings-key-intro = Add this key to the repository, with write access:
inst-belongings-key-where = Where: { $address }
inst-belongings-key-where-unknown = Where: in the repository's settings, as a deploy key that may write.
inst-belongings-token-where = A token is made here: { $address }
inst-access-waiting = The repository does not answer to this key yet.
inst-access-reads = The key reads the repository but cannot write to it yet: give it write access.
inst-access-ready = The key reads and writes the repository.
inst-checking = Checking…
inst-network-help = The installation needs the network. It reaches { $hosts } and your belongings' repository; choosing a network, or the cable, lets it.
inst-network-help-unnamed = The installation needs the network: it reaches the distro's release, its binary cache and your belongings' repository.
inst-gate-checking = Connecting and looking for a way out…
inst-gate-open = The network reaches out.
inst-gate-no-daemon = No network service answers on this installer.
inst-gate-off = Networking is switched off.
inst-gate-no-link = No network: plug in a cable, or choose a Wi-Fi network.
inst-gate-unconnected = That network did not take the connection: check its password, or choose another.
inst-gate-portal = A login page stands between this network and the internet: choose another network.
inst-release-looking = Asking the release which installer to use…
inst-release-newer = A newer installer is out ({ $rev }). { $enter } starts it, with the answers you gave.
inst-release-ahead = This installer is newer than the release: the machine is installed from the distro it carries, and stays on it.
inst-release-unreachable = The release did not answer: this machine is installed from this installer's own distro, and stays on it.
inst-release-development = This installer is a development build: the machine is installed from the distro it carries, and stays on it.
inst-invalid-blank = It cannot be left empty.
inst-invalid-hostname = Lowercase letters, digits and hyphens only; not starting or ending with a hyphen.
inst-invalid-hostname-prefix = The machine's name starts with `{ $prefix }`.
inst-invalid-passphrase-short = At least 12 characters.
inst-invalid-disk = That path is not a disk.
inst-invalid-language = That language is not available.
inst-invalid-layout = That keyboard layout does not exist.
inst-invalid-profile = The installation is `full` or `minimal`.
inst-invalid-access = Name the file that holds the key or the token, by its full path.
inst-step-template = Write this machine's flake from the template
inst-step-aim = Point the flake at this installer's distro
inst-step-lock = Lock the distro this machine is built from
inst-step-attribute = Ask the flake which system it builds
inst-step-plain = Fetch your declarations from the repository
inst-step-person = Apply your declarations to this machine
inst-step-verify = Try each program you declared
inst-step-access = Give your home its access to the repository
inst-step-consent = Let the repository's host in your consent
inst-step-passphrase = Hand the passphrase to the restore
inst-step-restore = Restore your belongings into your home
inst-step-init = Make the repository, keeping this machine's settings in it
inst-step-skipped = Nothing to do here.

## Installer — the eighteenth question (the programs a full installation leaves out)

inst-left-out = Optional programs
inst-left-out-help = Every program marked is installed, unless your belongings decline it on every machine. One unmarked is not downloaded now, and no update brings it, on this machine alone — the software centre brings it back.
inst-left-out-continue = Continue with these
inst-left-out-kept = install
inst-left-out-out = leave out
inst-left-out-review = Left out of this machine: { $names }
inst-keys-check = { $arrows } choose · { $enter } marks or continues · { $esc } goes back
inst-keys-check-space = { $arrows } choose · { $space } marks · { $enter } marks or continues · { $esc } goes back
inst-invalid-left-out = Only programs of the list are left out, each once; or none.
inst-step-left-out = Write the programs this machine leaves out
inst-step-plain-declining = Sign this machine's choice of programs into your repository, and fetch your declarations
inst-step-left-out-home = Write the programs left out into your home
inst-left-out-review-kept = A program marked that this machine left out before comes back; one your belongings decline on every machine stays out — the software centre brings it back.
inst-left-out-dropped = This release no longer lists { $ids }: none of that is left out. Unmark again what to leave out.
inst-left-out-not-applied = The declarations applied to this machine do not say its choice of programs; the disk was not touched.
inst-step-left-out-applied = Check that this machine's choice of programs was applied
inst-more-above = ↑ { $count } more
inst-more-below = ↓ { $count } more
inst-review-below = ↓ { $count ->
        [one] one more step
       *[other] { $count } more steps
    } below — { $marked ->
        [0] none of them erases the disk
        [one] one of them erases the disk
       *[other] { $marked } of them erase the disk
    }
inst-step-choice = Write which programs this machine keeps and which it leaves out
inst-kept-everywhere = Marked, and still declined on every machine by your belongings: { $names }. The software centre brings them back.
