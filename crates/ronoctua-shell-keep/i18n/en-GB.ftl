# The texts `ronoctua-shell-keep` says, in en-GB (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

## file info editing — everything the info panel can change

belongings-keep-local = keep mine
belongings-take-remote = take theirs
belongings-keep-both = keep both
belongings-prefs-refused = belongings.json does not read as settings, so nothing is written over it: correct it by hand, or delete it.
belongings-no-places = This session has no home or no runtime folder, so the belongings have nowhere to live.
belongings-decided-keep-local = keep this machine's
belongings-decided-take-remote = take the repository's
belongings-decided-keep-both = keep both
belongings-decide-keep-local = Keep this machine's
belongings-decide-take-remote = Take the repository's
belongings-decide-keep-both = Keep both
belongings-auth-ssh = an SSH deploy key
belongings-auth-token = an HTTPS token
belongings-period-days = { $n ->
    [one] every day
   *[other] every { $n } days
    }
belongings-period-never = never
belongings-empty-secret = Nothing was typed.
belongings-not-written = A file of the belongings could not be written; the system log says why.
belongings-never-kept = That is never kept: it is how this machine reaches the repository.
belongings-outside-home = Only what is in your home can be kept.
belongings-now-kept = { $name } is kept in the belongings now.
belongings-stopped-keeping = { $name } is no longer kept in the belongings; it stays where it is.
belongings-op-prepare = the sync
belongings-op-apply = the sync, with your decisions
belongings-op-abandon = dropping the sync
belongings-op-keygen = making the deploy key
belongings-op-rekey = the new passphrase
belongings-reason-preference = your settings
belongings-reason-programs = the programs you installed
belongings-reason-generated = made again from your settings on each machine
belongings-reason-access = how this machine reaches the repository
belongings-reason-own-state = what the belongings keep about themselves
belongings-reason-identity = the machine's own identity: two alike would collide
belongings-reason-machine = this machine's own state
belongings-reason-history = a history of this machine
belongings-reason-cache = a cache, made again when needed
belongings-reason-system-managed = a link the system makes: on another machine it would point nowhere
belongings-repair-why-wrong-type = a value of the wrong kind
belongings-repair-why-missing = a value that was not there
belongings-repair-why-invalid = a value that is not allowed
belongings-op-compact = compacting the history
belongings-op-sign = signing your plain part
belongings-held-unsigned = held: the repository's plain part carries no signature
belongings-held-no-key = held: no key here to check it against — pin yours below
belongings-held-bad-signature = held: not signed by your key
belongings-held-rolled-back = held: an older version than this machine already took
belongings-held-not-as-signed = held: not what your key signed
belongings-root-refused-not-signed = What was handed over carried no signature, and this machine kept the data it had.
belongings-root-refused-bad-signature = Not signed by the key this machine trusts, and this machine kept the data it had.
belongings-root-refused-rolled-back = An older version than this machine already took, and this machine kept the data it had.
belongings-kept-back-not-as-signed = kept as this machine has it: not what you signed
belongings-kept-back-no-key-pinned = kept as this machine has it: no key is pinned here
belongings-op-sync-plain = the plain part's sync
belongings-op-restore = the restore
belongings-op-probe = checking the access
belongings-op-init = the first sync
belongings-op-verify-passphrase = checking the passphrase
belongings-decided-take-merged-ours = merged, this machine's where they clash
belongings-decided-take-merged-theirs = merged, the repository's where they clash
belongings-decide-take-merged-ours = Merge, keeping this machine's where they clash
belongings-decide-take-merged-theirs = Merge, taking the repository's where they clash
belongings-plain-already = That travels already, in clear, with the plain part.
belongings-op-forget = forgetting the session's key
belongings-target-git = git's settings and your registered security keys
belongings-target-gnupg = your GnuPG keyring
belongings-target-shell = your shell's own file
belongings-target-emacs = your Emacs configuration
# What a target leaves out, each by its own reason. Short: the listing's
# widest value moves its column.
belongings-reason-socket = a socket of a program running now
belongings-reason-connection = an SSH connection shared while it is open
belongings-reason-random-seed = the agent's random seed: each machine has its own
belongings-reason-lock = a lock held by a program running now
belongings-reason-packages = packages, fetched again from your configuration
belongings-reason-compiled = compiled again from what travels
belongings-reason-editor-state = what Emacs keeps of this machine's sessions
belongings-reason-editor-draft = an editor's backup, autosave or lock
belongings-reason-git-folder = a git repository's own folder: a clone brings it
belongings-state-synced = in step
belongings-state-changed = changed
belongings-state-new = new
belongings-state-conflict = in conflict
belongings-state-unreadable = unreadable here
belongings-state-repaired = repaired here
belongings-state-deleted = deleted here: it leaves the repository and your other machines
belongings-reason-too-large = too large to keep in clear
