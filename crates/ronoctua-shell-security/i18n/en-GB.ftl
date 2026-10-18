# The texts `ronoctua-shell-security` says, in en-GB (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

## file info editing — everything the info panel can change

# The view's keys, as the hint line names them (`security::OF_THE_VIEW`).
security-down = down
security-up = up
security-enter = do it
security-left = finger back
security-right = finger on
security-relax = no password
security-shields = shields
security-rule-add = add rule
security-rule-delete = remove
security-enrol = enrol
# THE SECURITY CENTRE (`LEADER s e`, `crate::security`): its leader group,
# its commands, its view's keys and questions, and its page.
group-security = security
cmd-security-open = Open the security centre
cmd-security-window = Open the security centre in a window
cmd-security-relax = Let the administrator act without a password for a while, or end that now
cmd-security-shields = Refuse every incoming connection, or stop refusing them
cmd-security-firewall = Switch your firewall on or off
cmd-security-check-password = Check whether a password is in known breaches
cmd-security-check-email = Check whether an e-mail address is in known breaches
cmd-security-check-file = Check a file with VirusTotal
cmd-security-down = Next row of the security centre
cmd-security-up = Previous row of the security centre
cmd-security-enter = Do what the row under the cursor offers
cmd-security-left = The previous finger to enrol
cmd-security-right = The next finger to enrol
cmd-security-rule-add = Add a firewall rule
cmd-security-rule-delete = Remove the rule or the finger under the cursor
cmd-security-enrol = Enrol the chosen finger
cmd-security-rule = A firewall rule, in words
cmd-security-vt-key = Keep your VirusTotal key
cmd-security-vt-forget = Forget your VirusTotal key
cmd-security-home = Treat the network that just came up as home
ask-security-rule = Rule (allow tcp 8080 from 192.168.1.0/24 via wlan0 home)
# The file manager's changes only the administrator may make (an owner, a
# group, permissions), through the same sudo door.
ask-security-check-password = Password to check (only five characters of its fingerprint leave this machine)
ask-security-check-email = E-mail address to check (it is sent to XposedOrNot)
ask-security-check-file = File to check (its fingerprint is sent to VirusTotal)
echo-security-checking = Checking…
echo-security-file-checked = { $file }: { $verdict }
echo-security-finger-enrolled = Finger enrolled.
echo-security-finger-removed = Finger removed.
echo-security-finger-sudo-changed = Fingers for the administrator changed.
echo-security-firewall-applied = The firewall is as you asked.
echo-security-key-forgotten = The VirusTotal key is forgotten.
echo-security-key-kept = The VirusTotal key is kept.
echo-security-key-not-a-key = That is not a key VirusTotal takes.
echo-security-key-not-kept = The key could not be kept in your folder.
echo-security-nothing-to-delete = There is no rule or finger under the cursor.
echo-security-no-new-network = No network that came up is waiting to be placed.
echo-security-relaxed = The administrator asks for no password until the time on the modeline.
echo-security-relax-ended = The administrator asks for the password again.
echo-security-rule-not-understood = That is not a rule: allow or deny, then tcp or udp, the ports, and optionally from, via and home, public or everywhere.
echo-security-shields-up = Every incoming connection is refused now.
echo-security-upload-queued = { $file } is on its way to VirusTotal; the transfers list shows how far.
security-title = Security
security-on = on
security-off = off
security-unknown = unknown
security-answer-no = No
security-answer-yes = Yes
security-busy = The administrator is being asked…
security-busy-enrol = Touch the reader when it asks.
security-heading-now = Now
security-heading-firewall = Firewall
security-heading-admin = Administrator
security-heading-finger = Fingerprint
security-heading-device = This machine
security-heading-privacy = Network privacy
security-heading-online = Online checks
security-heading-suggest = Suggestions
security-now-nothing = Nothing is off.
security-now-reading = Reading…
security-now-firewall-failed = The firewall failed
security-now-settings-unreadable = The firewall's settings cannot be read
security-now-firewall-off = Your firewall is off
security-now-sudo-open = No administrator password until
security-now-shields = Every incoming connection is refused
security-now-refused = { $count ->
    [one] One connection refused lately
   *[other] { $count } connections refused lately
  }
security-fw-switch = Your firewall
security-fw-network = Network on { $iface }
security-fw-served = this machine serves it
security-fw-treat-as-home = Treat this network as home
security-fw-shields = Refuse every incoming connection
security-fw-shields-note = Raising them is at once; lowering them asks for your password, as every loosening does.
security-fw-log = Refused connections in the log
security-fw-rule-add = Add a rule
security-fw-blocked = Refused lately:
security-fw-blocked-row = { $proto } { $port } from { $source } on { $iface } ({ $count ->
    [one] once
   *[other] { $count } times
  })
security-fw-watch-silent = The brain is not answering, so the connections refused lately cannot be shown.
security-fw-ports = What this machine answers on:
security-fw-port-row = { $who }: { $proto } { $port }
security-fw-port-unknown = a program of another user
security-fw-port-container = container { $name }
security-fw-verdict-open = open: the firewall is off
security-fw-verdict-allowed = allowed by your rule
security-fw-verdict-partly = allowed only on some networks, or from some addresses
security-fw-verdict-refused = refused, unless the system opens it
security-fw-verdict-shielded = refused: every connection is
security-sudo-password = The administrator's password
security-sudo-asked = asked every time
security-sudo-not-asked-until = not asked until { $time }
security-sudo-end = Ask for it again now
security-sudo-open-for = Do not ask for { $minutes } minutes
security-sudo-note = It asks for your password first, and ends by itself at the time chosen — a restart does not end it — or when you end it.
security-sudo-recent = This week:
security-finger-reader = Reader
security-finger-enrolled = enrolled
security-finger-enrol = Enrol
security-finger-ring-note = The arrows to the sides choose the finger; it asks for your password first, then for a few touches.
security-finger-sudo = Fingers for the administrator
security-finger-sudo-note = In a terminal and in the sudo bar a finger answers in place of your password, which always answers too; never the login.
security-finger-ask-remove = Remove your { $finger }? It cannot be used again until it is enrolled again.
security-device-secure-boot = Secure Boot
security-device-tpm = Security chip (TPM)
security-device-encryption = The system's disk
security-device-encrypted = encrypted
security-device-not-encrypted = not encrypted
security-device-hsi = Firmware security level
security-device-firmware-updates = Firmware updates
security-device-firmware-updates-count = { $count ->
    [0] none known
    [one] one available
   *[other] { $count } available
  }
security-device-firmware-note = Read only: firmware is not installed from here.
security-device-updates = Automatic updates
security-device-lock = Screen lock
security-device-lock-none = none: this system has no lock screen
security-device-autologin = Starting the machine
security-device-autologin-gate = the disk's passphrase is the only gate
security-privacy-mac = Hardware address on { $name }
security-privacy-dns = Encrypted DNS on { $name }
security-privacy-none = No network is up.
security-privacy-ssh = SSH server
security-privacy-ssh-on = something answers on port 22
security-privacy-note = These are read, not changed here: each connection's settings are the network's.
security-online-key-kept = kept
security-online-key-none = none: paste one from your VirusTotal account
security-online-vt-forget = Forget the VirusTotal key
security-online-check-file = Check a file
security-online-file-note = Only its fingerprint (SHA-256) is sent to VirusTotal. A file it has never seen is sent only if you say yes.
security-online-check-email = Check an e-mail address
security-online-email-note = The address itself is sent to XposedOrNot.
security-online-email-clean = { $address } is in no known breach.
security-online-email-found = { $address } is in { $count ->
    [one] one breach
   *[other] { $count } breaches
  }: { $names }
security-online-check-password = Check a password
security-online-password-note = Only five characters of its fingerprint (SHA-1) leave this machine; the rest is compared here.
security-online-password-clean = That password is in no known breach.
security-online-password-found = That password was found { $count ->
    [one] once
   *[other] { $count } times
  } in breaches: do not use it.
security-vt-ask-upload = VirusTotal has never seen { $file }. Send the file itself?
security-vt-upload-warning = A file sent to VirusTotal is shared with its partners and can be downloaded by its paying subscribers. Do not send anything private.
security-rule-word-from = from
security-rule-word-on = via
security-rule-word-home = home
security-rule-word-public = public
security-rule-word-everywhere = everywhere
security-rule-word-any = any
security-about-show = Goes to where this is
security-about-profile = Home or public: a public network lets nothing in that your rules for home would
security-about-log = How many refused connections go in the log
security-about-rule = Change this rule, in words; the delete key removes it
security-about-allow-home = Allows this port on home networks, with your password
security-about-allow-public = Allows this port on public networks too, with your password
security-about-finger = Removes this finger, after a question
security-about-finger-sudo = Lets a finger answer the administrator — in a terminal and in the sudo bar — in place of your password
security-about-report = Opens the file's report on VirusTotal's page
security-about-url = Opens the page in your browser
security-about-answer = Answers the question
security-suggest-vpn = Use a VPN: import its file in the network view
security-suggest-tor = Go through Tor
security-suggest-tailnet = Reach your own machines through your tailnet
security-suggest-hibp = Have I Been Pwned, in your browser (no address goes with it)
# The passwordless window's longer spans, and the one with no limit.
security-sudo-open-for-hours = Do not ask for { $hours } hours
security-sudo-open-no-limit = Do not ask until I say so
security-sudo-not-asked-no-limit = not asked until you end it
security-sudo-until-you-end-it = you end it
cmd-private-passphrase = The passphrase of /private
cmd-private-new-passphrase = A new passphrase for /private
cmd-private-repeat-passphrase = The same passphrase again
cmd-private-cap = How large /private may grow (64G, 1.5T…)
security-heading-private = /private
security-private-about = A folder only you can open: encrypted with a passphrase of its own, it takes on the disk only what it holds, and it locks itself when the machine sleeps.
security-private-missing = This machine does not have the vault's units, so there is no /private here.
security-private-unreadable = What /private is doing could not be read.
security-private-create = Create /private
security-private-create-value = up to { $size }
security-private-state = State
security-private-locked = locked
security-private-open-at = open, at /private
security-private-suspended = suspended while in use: its key left memory at sleep
security-private-open = Open with the passphrase
security-private-open-key = Open with the security key
security-private-resume = Resume with the passphrase
security-private-lock = Lock it now
security-private-show = Show it in the file manager
security-private-size = On the disk
security-private-size-value = { $real } of up to { $cap }
security-private-grow = Raise the cap
security-private-cipher = Encryption
security-private-cipher-value = LUKS2, AES-XTS with a 512-bit key, argon2id
security-private-key = Security key
security-private-key-enrolled = enrolled: a touch opens /private
security-private-enrol = Enrol a security key
security-private-forget = Forget the security key
security-private-passphrase = Change the passphrase
security-private-note = It locks itself when the machine sleeps. Something still using it then is suspended instead: its key leaves memory, and the passphrase resumes it.
security-private-overcommit = The cap is larger than what this disk has free: if the disk fills up, writing to /private fails.
security-private-busy = Working on /private…
security-private-touch = Touch your security key now.
echo-private-not-a-size = That is not a size: write it like 64G or 1.5T.
echo-private-too-small = /private needs a cap of at least { $size }.
echo-private-mismatch = The two passphrases differ, so nothing was changed.
echo-private-already-locked = /private is already locked.
security-now-private-suspended = /private is suspended while in use

## THE SECURITY CENTRE'S KEYS, and the passwordless window's count.

security-heading-keys = Security keys
security-keys-sudo = Answer the administrator
security-keys-on = yes, { $count ->
        [one] one key
       *[other] { $count } keys
    }
security-keys-off = no
security-keys-off-listed = no; your list has { $count ->
        [one] one key
       *[other] { $count } keys
    }
security-keys-key = Key { $n }
security-keys-bring = Bring the keys of your other machines
security-keys-away = { $count ->
        [one] one key
       *[other] { $count } keys
    }
security-keys-register = Register a key
security-keys-touch-now = Touch the key you are registering.
security-keys-stop = Stop registering
security-keys-full = At most { $count } keys answer the administrator.
security-keys-note = A key answers the administrator in place of your password — in a terminal and in the sudo bar — never the login; your password always answers too. Off takes them from this machine and keeps them in your list, which travels with the belongings.
security-keys-not-here = Security keys do not answer the administrator on this machine (ronoctua.yubikey.enable is off).
security-keys-ask-remove = Remove the key { $key }?
security-keys-remove-warning = It stops answering the administrator here, and leaves your list, so it answers on no other machine either.
security-about-key = Removes this key, after asking
security-about-key-register = Waits for a touch of the key, then lets it answer the administrator — here, and wherever the belongings take it
security-about-key-stop = Stops waiting for the touch
security-about-keys-switch = Your keys answer the administrator here, or stop — at once, with no rebuild
security-about-keys-bring = Lets the keys registered on your other machines answer here too
security-about-relax-again = Opens the window again for the same time, from now
security-sudo-left = Time left
security-sudo-again = Start the count again
echo-security-relax-again = The count started again.
echo-security-keys-registered = The key was registered: it answers the administrator now.
echo-security-keys-removed = The key was removed.
echo-security-keys-off = Your security keys no longer answer the administrator here.
echo-security-keys-on = Your security keys answer the administrator here again.
echo-security-keys-brought = The keys of your other machines answer the administrator here now.
cmd-security-encrypt = Encrypt a file or a folder: for you, a passphrase or someone's key
cmd-security-quantum = The security centre's post-quantum section: what each channel does against a quantum computer
security-heading-encrypt = Encrypt
security-encrypt-for-you = For you
security-encrypt-file = Encrypt files…
security-encrypt-folder = Encrypt a folder…
security-encrypt-open = Decrypt a file…
security-encrypt-note = Each one goes to the transfer queue. Nothing is written over, and the original stays; a folder becomes name.tar.age. For you and for someone's key are ML-KEM-768 with X25519; a passphrase is scrypt.
security-encrypt-pick-files = What to encrypt
security-encrypt-pick-folder = The folder to encrypt
security-encrypt-pick-open = The .age to decrypt
security-about-seal-folder = Choose a folder in the file picker; it is sealed as one .tar.age
security-about-seal-open = Choose a .age in the file picker; its header says what opens it
security-about-seal-mode = Seals what was chosen this way, on the transfer queue
security-heading-quantum = Post-quantum
security-quantum-note = A recording made today can be read the day a quantum computer breaks its key, and a signature can then be forged. Each channel says where it stands.
security-quantum-status-post-quantum = post-quantum
security-quantum-status-symmetric = symmetric
security-quantum-status-classical = classical
security-quantum-status-unknown = not known
security-quantum-channel-belongings = Belongings
security-quantum-channel-belongings-transport = Belongings' transport
security-quantum-channel-seal-to-key = Encrypting to a key
security-quantum-channel-seal-passphrase = Encrypting with a passphrase
security-quantum-channel-private = Private vault
security-quantum-channel-root-disk = System disk
security-quantum-channel-https = Our HTTPS
security-quantum-channel-openssl = OpenSSL
security-quantum-channel-ssh = SSH
security-quantum-channel-chrome = Chrome
security-quantum-channel-syncthing = Syncthing
security-quantum-channel-tailscale = Tailscale
security-quantum-channel-tor = Tor
security-quantum-channel-clipcascade = ClipCascade
security-quantum-channel-sops = Machine secrets
security-quantum-channel-extension = Browser extension
security-quantum-channel-f-droid = F-Droid
security-quantum-channel-nix-cache = Binary cache
security-quantum-channel-secure-boot = Secure Boot's keys
security-quantum-why-belongings-pq = Sealed to your own key: ML-KEM-768 with X25519, and both must fall.
security-quantum-why-belongings-unsynced = Not synced on this machine yet: the repository is sealed with ML-KEM-768 and X25519, and so is the first sync.
security-quantum-why-belongings-new = None set up yet; a repository set up now is sealed with ML-KEM-768 and X25519.
security-quantum-why-belongings-transport = SSH and HTTPS with a post-quantum key exchange only: a server that has none is refused.
security-quantum-why-seal-to-key = age's mlkem768x25519: for you, or for someone's age1pq1 key.
security-quantum-why-seal-passphrase = scrypt and ChaCha20-Poly1305: a quantum computer halves a key's strength, and a long passphrase still holds.
security-quantum-why-private = LUKS2 with AES-XTS and a 512-bit key, the passphrase stretched by argon2id.
security-quantum-why-root-disk = The same for the system's disk: LUKS2, AES-XTS-512, argon2id.
security-quantum-why-root-disk-unread = Whether the system's disk is encrypted could not be read.
security-quantum-why-https-all = { $hosts ->
        [one] The one host heard agreed X25519MLKEM768.
       *[other] Every one of the { $hosts } hosts heard agreed X25519MLKEM768.
    }
security-quantum-why-https-some = { $hosts ->
        [one] The one host heard still agrees a classical key: { $names }.
       *[other] { $classical ->
            [one] One of the { $hosts } hosts heard still agrees a classical key: { $names }.
           *[other] { $classical } of the { $hosts } hosts heard still agree a classical key: { $names }.
        }
    }
security-quantum-why-https-none-heard = No host heard yet: each one's first request says what it agrees.
security-quantum-why-openssl = OpenSSL { $version } offers X25519MLKEM768 first: git over HTTPS, curl and what else links it.
security-quantum-why-openssl-old = OpenSSL { $version } has no post-quantum group by default.
security-quantum-why-ssh = OpenSSH { $version } offers { $kex } first, a post-quantum key exchange; a server without one agrees a classical key.
security-quantum-why-ssh-classical = OpenSSH { $version }, as it is configured, offers no post-quantum key exchange (KexAlgorithms).
security-quantum-why-ssh-not-first = OpenSSH { $version }, as it is configured, offers { $kex } first and a post-quantum exchange only after it: a server that knows both agrees the classical one (KexAlgorithms).
security-quantum-why-ssh-unread = ssh did not say what it offers.
security-quantum-why-chrome = Chrome { $version } agrees X25519MLKEM768 by default.
security-quantum-why-chrome-old = Chrome { $version } agrees a classical key by default.
security-quantum-why-syncthing = Built with Go { $go }, which offers X25519MLKEM768, and Syncthing does not narrow it; a device on an older build agrees X25519.
security-quantum-why-syncthing-pinned = Syncthing { $version } narrows its key exchange to classical curves.
security-quantum-why-syncthing-old-go = Syncthing { $version } is built with or for a Go older than 1.24: X25519 only.
security-quantum-why-tailscale = WireGuard agrees its keys with X25519, and Tailscale has no post-quantum mode.
security-quantum-why-tor = Tor's circuits are agreed with X25519; Tor has no post-quantum handshake yet.
security-quantum-why-clipcascade = AES-256-GCM with a key from your password, carried over our TLS.
security-quantum-why-sops = The machine's secrets are sealed to an X25519 key made from the SSH host key.
security-quantum-why-extension = The browser extension is signed with RSA-2048, and its format has no other signature: a forgery needs a quantum computer at the moment of the attack.
security-quantum-why-f-droid = F-Droid signs its index with RSA: a forgery, not a reading, is what a quantum computer would make of it.
security-quantum-why-nix-cache = The binary cache's signatures are ed25519: a forgery needs a quantum computer at the moment of the attack.
security-quantum-why-secure-boot = Secure Boot's keys are RSA-2048: the firmware verifies nothing else.
security-quantum-why-not-built = This build did not say which version it ships.
