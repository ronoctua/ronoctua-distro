# The texts `ronoctua-belongings` says, in en-GB (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

## file info editing — everything the info panel can change

belongings-invalid-empty = Nothing was typed.
belongings-invalid-credentials-in-address = The address carries a user and a password: give the address alone, and the key or the token under Access.
belongings-invalid-plain-http = An http:// address sends the access in the clear: use https:// or SSH.
belongings-invalid-not-remote = A local path or file:// is not a remote repository.
belongings-invalid-unknown = The address is git@host:path, ssh://… or https://….
belongings-refused-no-repository = No repository is set: give its address under Status.
belongings-refused-no-passphrase = The passphrase was not there, or somebody else could read its file; it was not used.
belongings-refused-wrong-passphrase = That passphrase does not open these belongings.
belongings-refused-needs-access = The repository wants access this machine was not given: a deploy key or a token, under Access.
belongings-refused-refused = The repository refused this machine's key or token.
belongings-refused-unreachable = The repository could not be reached.
belongings-refused-raced = Another machine kept syncing while this one did; try again.
belongings-refused-not-ours = That repository is not a belongings repository this system reads.
belongings-refused-empty = The repository is empty: the first sync sets the passphrase.
belongings-refused-corrupt = Something in the repository does not decrypt or does not read.
belongings-refused-undecided = A conflict has no decision yet.
belongings-refused-no-session = There is no prepared sync to apply.
belongings-refused-no-tools = This installation does not have git or ssh-keygen.
belongings-refused-failed = A file here could not be read or written; the system log says which.
belongings-refused-busy = Another run of the belongings is going on: wait for it to end.
belongings-refused-not-post-quantum = The repository's server offered no post-quantum key exchange, and the belongings ask for one: nothing was fetched or pushed.
belongings-refused-choice-unread = The installer's choice of programs does not read: nothing was declined or brought back, and nothing was sent.
belongings-refused-copy-unwritten = The copy of your declarations could not be written where the installer keeps it; the system log says where.
belongings-refused-not-let = The repository's host may not be asked yet: allow it on the row under Sync. Nothing was fetched or sent.
belongings-refused-newer = The repository is in a newer format than this machine reads: update this machine first.
belongings-invalid-unnamed-host = The host has to be a name or an IPv4 address: an IPv6 address or a name with an underscore cannot be allowed to be asked.
belongings-refused-plain-too-large = Too large to keep in clear.
