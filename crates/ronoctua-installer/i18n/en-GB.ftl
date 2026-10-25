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
inst-passphrase-help = The disk's key: typed at every boot, and the only thing that opens it. Keep it off the computer.
inst-password-again = Type the user password again
inst-password-differs = The two passwords differ. Type it again from the start.
inst-passphrase-again = Type the disk passphrase again
inst-passphrase-differs = The two disk passphrases differ. Type it again from the start.
inst-language = Language
inst-language-help = For the interface and the keyboard.
inst-tpm = No security chip (TPM 2.0) was found
inst-tpm-what = The TPM is a security chip inside the computer. It is where ronoctua keeps the passwords the computer has to remember on its own — the Wi-Fi's, for one — in a way nobody can read by copying the disk.
inst-tpm-why = Without it, those passwords would be exposed. So the installation does not go on: it is for your safety.
inst-tpm-how = How to switch the TPM on
inst-tpm-find = Look for an option called TPM, Security Device, PTT (Intel processors) or fTPM (AMD processors) and set it to Enabled.
inst-tpm-none = If the setup has none of these options, the computer has no TPM 2.0, and ronoctua cannot be installed on it.
inst-tpm-csm = This computer started in BIOS mode, where many hide the TPM: if the setup has none of these options, switch UEFI on (Boot Mode, UEFI/Legacy or CSM) and look again.
inst-tpm-leave = Leave the installer
inst-tpm-preview = See the rest of the preview (nothing is installed)
inst-firmware = This computer started in BIOS mode; UEFI is better
inst-firmware-what = BIOS and UEFI are two ways a computer starts its system. UEFI is the modern one, and ronoctua is made for it; BIOS mode (Legacy or CSM) is for old systems. Almost every computer made after 2012 has UEFI.
inst-firmware-why = On UEFI the boot is systemd-boot, which is simpler and ronoctua's default. And in BIOS mode many computers hide the security chip (TPM), without which the installation does not go on.
inst-firmware-how = How to switch UEFI on
inst-firmware-find = Look for Boot Mode, UEFI/Legacy or CSM, and choose UEFI (or switch CSM off).
inst-firmware-still = You can still install in BIOS mode: ronoctua works, with GRUB in place of systemd-boot.
inst-firmware-leave = Leave to switch UEFI on (recommended)
inst-firmware-worse = Go on with the worse alternative (BIOS)
inst-step = { $n }.
inst-setup-open = Restart the computer and, as soon as it starts, press the setup key — usually { $f2 }, { $f10 }, { $f12 } or { $del }.
inst-setup-save = Save and exit (Save & Exit).
inst-setup-boot-again = Boot from the stick again.
inst-firmware-boot-again = Boot from the stick again; if the menu lists it twice, choose the "UEFI:" entry.
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
inst-step-install = Install the system onto the disk
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
inst-keys-secret = { $enter } continues · { $tab } shows it · { $esc } goes back · { $quit } quits
inst-keys-secret-shown = { $enter } continues · { $tab } hides it · { $esc } goes back · { $quit } quits

## Installer — the seventh question (network)

inst-wifi = Wi-Fi network
inst-wifi-help = The password stays on the installed machine, never in the repository.
inst-wifi-none = no Wi-Fi — a cable, or none: it installs offline
inst-wifi-connected = { $network } — connected now
inst-wifi-scanning = Looking for networks…
inst-wifi-empty = No networks in range
inst-wifi-password = Network password
inst-wifi-password-short = a WPA network password is at least 8 characters
inst-step-seal-wifi = Seal the Wi-Fi password in the TPM
inst-step-wifi = Write the Wi-Fi network profile

## file info editing — everything the info panel can change

# The installer's fifteenth question.
inst-belongings = Belongings repository
inst-belongings-help = The private repository your configuration is kept in, encrypted: git@host:path, ssh://… or https://…, never with a password in it. The machine's first boot restores your belongings from it into your home.
inst-step-belongings = Write the belongings' repository into your home
inst-belongings-access = Access to the repository
inst-belongings-access-help = This machine reaches the repository with its own key, made now and kept only in your home. It is asked again until it reads and writes.
inst-belongings-access-offline-help = This machine reaches the repository with its own key, made now and kept only in your home. Add it to the repository now: with no network it is not tried here, and the first boot uses it.
inst-belongings-token-help = Type a token that reads and writes the repository. It stays in your home, readable by you alone.
inst-belongings-passphrase = Belongings passphrase
inst-belongings-passphrase-help = The passphrase that opens your belongings. It is checked against the repository before anything is touched.
inst-belongings-passphrase-new-help = The repository is empty: choose the passphrase that will seal your belongings, at least 12 characters. It is typed twice.
inst-belongings-passphrase-offline-help = The passphrase that opens your belongings — or, if the repository is empty, the one that will seal them. With no network it is not checked now: at least 12 characters, typed twice.
inst-belongings-passphrase-again = Type the passphrase again
inst-belongings-passphrase-differs = The two passphrases differ. Type it again from the start.
inst-belongings-key-intro = Add this key to the repository, with write access:
inst-belongings-key-where = Where: { $address }
inst-belongings-key-where-unknown = Where: in the repository's settings, as a deploy key that may write.
inst-belongings-token-where = A token is made here: { $address }
inst-belongings-token-rights-github = What it needs: a fine-grained token; under Repository access, Only select repositories, with this one; under Repository permissions, Contents: Read and write. GitHub adds Metadata: Read-only by itself. Nothing else.
inst-belongings-token-rights-gitlab = What it needs: the write_repository scope, which reads and writes through git. Nothing else.
inst-belongings-token-rights-codeberg = What it needs: under Repository and Organization Access, Specific repositories, with this one; the repository permission, Read and write (write:repository). Nothing else.
inst-belongings-https-not-post-quantum = This server's HTTPS offers no post-quantum key exchange, and the belongings ask for one: no token would get through. The field now holds the same repository's SSH address; going on with it makes a deploy key for this machine.
inst-access-waiting = The repository does not answer to this key yet.
inst-access-reads = The key reads the repository but cannot write to it yet: give it write access.
inst-access-ready = The key reads and writes the repository.
inst-checking = Checking…
inst-network-help = Optional: the installation itself needs no network. With one, your belongings' repository is checked now; a Wi-Fi network chosen also stays on the installed machine. With none, the machine's first boot checks it.
inst-gate-checking = Connecting and looking for a way out…
inst-gate-open = The network reaches out.
inst-gate-no-daemon = No network service answers on this installer.
inst-gate-off = Networking is switched off.
inst-gate-no-link = No network: plug in a cable, or choose a Wi-Fi network.
inst-gate-unconnected = That network refused the password: check it, or choose another.
inst-gate-unjoined = That network was not joined, and its password was not refused: try again, or choose another.
inst-gate-portal = A login page stands between this network and the internet: choose another network.
inst-offline = No network now: the repository is not checked here; the machine's first boot checks it.
inst-first-boot-restore = On its first boot, once there is a network, this machine restores your belongings into your home.
inst-first-boot-start = The repository is empty: on its first boot, once there is a network, this machine starts your belongings in it, sealed with the passphrase you chose.
inst-first-boot-unchecked = Your belongings were not checked, with no network now: on its first boot, once there is a network, this machine checks the repository and restores them — or starts them in it, if it is empty.
inst-first-boot-later = The programs you declared, and the parts that need a download, arrive after the first boot, over the network.
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
inst-step-person = Apply your declarations to this machine
inst-step-access = Give your home its access to the repository
inst-step-consent = Let the repository's host in your consent
inst-step-defer = Leave for the first network what nobody may redistribute
inst-step-build-first = Build the system before the disk is touched
inst-step-build = Build the system the disk gets, with its swap measured
inst-step-seed-parts = Copy the parts this medium carries to the machine
inst-step-first-boot = Write what the first boot finishes: your belongings
inst-step-seal-passphrase = Seal the belongings' passphrase in the TPM for the first boot
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
