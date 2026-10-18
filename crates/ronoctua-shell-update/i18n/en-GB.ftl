# The texts `ronoctua-shell-update` says, in en-GB (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

brain-needs-root = Runs as root
# THE UPDATE, WATCHED — the modeline's mark, the leader's group for the view
# that watches a run, and the countdown's two answers that view teaches too.
#
# The mark shows only while something runs and goes on its own. The update's
# commands, its steps and the view's own words are its brain module's texts,
# in that module's folder.
group-update = update

## The machine's life (`ronoctua-machine`): why an apply, a migration, a bless or a hold did not do what it was asked (`machine-refused-<why>`).

machine-refused-not-root = Only root changes the machine: nothing was done.
machine-refused-busy = Another change to this machine is running: try again when it ends.
machine-refused-nothing-to-migrate = This machine already has its own flake: there is nothing to migrate.
machine-refused-build-failed = The new system did not build. Nothing changed: the machine runs what it ran, and its folder says so again.
machine-refused-switch-failed = The switch to the new system failed. The next start tries it once, and comes back to the last good system if it does not start.
machine-refused-did-not-boot = This is the system that did not start when it was tried, or that was rolled back. The machine stays on the one it runs until a newer revision of the distro arrives.
machine-refused-last-good-would-fall-out = One more system would push the last good one out of the boot menu. Restart first, so the system on trial can be confirmed.
machine-refused-held = The machine is held at another revision than the one its folder names: nothing was built.
machine-refused-unreadable = A file of the machine's folder cannot be read.
machine-refused-failed = A program the machine needed failed; the journal says which.

## How the boot trial stands (`machine-trial-<outcome>`).

machine-trial-pending = The new system starts for the first time at the next restart. If it does not start, the machine comes back to the last good one by itself.
machine-trial-good = The new system started well: it is the last good one now.
machine-trial-failed = The new system did not start, or was rolled back: the machine is back on the last good one.
machine-trial-boot-identical = The new system starts exactly like the last good one: no restart is needed to trust it.

## Why the new system has no boot trial, until it has started once (`machine-no-trial-<why>`).

machine-no-trial-no-efi-variables = This machine's boot variables cannot be written, so the new system has no way back but the boot menu.
machine-no-trial-not-in-menu = The boot menu does not name the new system or the last good one in a way the trial can use, so the new system has no way back but the boot menu.
machine-no-trial-no-last-good = No system is known to have started well on this machine yet, so the new system has no way back but the boot menu.
# THE PERSON'S BINARY CACHE — a page of the update's view (`c`, `LEADER s u c`),
# in both doors, and what the installation and root's netrc say of it.
binary-cache-open = binary cache
cmd-binary-cache-open = Your binary cache: your own Cachix cache, which this machine downloads from and pushes to after each update
cmd-binary-cache-name = Names your binary cache — its name, then its public key
cmd-binary-cache-key = Gives your binary cache its public key, exactly as cachix.org shows it
cmd-binary-cache-token = Gives your binary cache its push token, kept encrypted with your belongings
cmd-binary-cache-netrc = Lets this machine read your private binary cache: root's netrc, written through sudo
ask-binary-cache-name = Name of your Cachix cache
ask-binary-cache-key = Public key of { $name }
binary-cache-heading = Your binary cache
binary-cache-row-cache = cache
binary-cache-row-key = public key
binary-cache-row-visibility = who reads it
binary-cache-row-token = push token
binary-cache-row-access = this machine reads it
binary-cache-row-machine = on this machine
binary-cache-row-last = last push
binary-cache-row-forget-token = forget the push token on this machine
binary-cache-row-remove = stop using this cache
binary-cache-about-cache = Your own Cachix cache — never the distro's, which has none. This machine downloads from it before cache.nixos.org, and pushes to it after each update that switched it. The choice travels with your belongings to your other machines. Choosing it asks its name, then its key.
binary-cache-about-key = The cache's signing key, exactly as cachix.org shows it on the cache's page: every download from it is checked against this key. Choosing it asks it.
binary-cache-about-visibility = A public cache receives only the distro's own outputs — never a path of yours, never an unfree program; anybody may download them. A private cache receives this machine's whole system, and reading it needs your token. Choosing it switches between the two.
binary-cache-about-token = The token Cachix gives you to push. It is kept encrypted in your belongings, readable by you alone, and never shown, written on a command line or logged. Choosing it asks a new one.
binary-cache-about-access = A private cache is read by this machine's Nix through a netrc only root reads, holding your token. It is written through sudo when you set the token or make the cache private; choosing it writes it again.
binary-cache-about-machine = What this machine uses now. A cache you name here reaches this machine at its next update, when your belongings' signed copy is applied.
binary-cache-about-last = What the last push carried, or why it was refused. A push follows each update that switched this machine, once.
binary-cache-about-forget-token = The token goes from this machine and from your belongings at their next sync; nothing more is pushed until you give it a new one.
binary-cache-about-remove = This machine stops downloading from and pushing to the cache at its next update. The cache itself, on Cachix, stays as it is.
binary-cache-none = none
binary-cache-key-none = none
binary-cache-public = public: the distro's own outputs, for anybody
binary-cache-private = private: this machine's system, read with your token
binary-cache-token-none = none: nothing is pushed
binary-cache-token-kept = kept, encrypted with your belongings
binary-cache-token-refused = not used
binary-cache-copy-unreadable = cannot be read
binary-cache-copy-newer = of a newer format
binary-cache-down = next row
about-binary-cache-down = Puts the cursor on the next row of the binary cache's page.
binary-cache-up = previous row
about-binary-cache-up = Puts the cursor on the previous row of the binary cache's page.
binary-cache-enter = change
about-binary-cache-enter = Does what the row under the cursor says: asks a name, a key or the token, switches who reads the cache, writes root's netrc, or asks before a gesture that does not come back.
binary-cache-steps = the steps
about-binary-cache-steps = Back to the update's steps.
binary-cache-access-written = yes, through root's netrc
binary-cache-access-missing = no: root's netrc is not written
binary-cache-machine-in-use = in use, before cache.nixos.org
binary-cache-machine-next = from the next update
binary-cache-machine-leaving = still the one before, until the next update
binary-cache-machine-none = no cache
binary-cache-last-never = never
binary-cache-last-pushed = { $when }: { $paths ->
        [one] one path
       *[other] { $paths } paths
    } ({ $size })
binary-cache-last-refused = { $when }: nothing pushed
binary-cache-why-no-cache = This machine names no binary cache.
binary-cache-why-no-token = There is no push token on this machine.
binary-cache-why-token-open = The token's file is readable by somebody else, so it is not used: give the token again.
binary-cache-why-not-a-token = That is not a Cachix token.
binary-cache-why-not-let = A host the push reaches is not allowed in your consent: naming the cache again allows them.
binary-cache-why-nothing-new = No update switched this machine since the last push: there was nothing new to push.
binary-cache-why-no-tools = This machine has no cachix or nix where the push looks for them.
binary-cache-why-store-failed = Nix did not answer about the store.
binary-cache-why-cachix-failed = Cachix refused the push, or it did not finish: the update's output says why.
binary-cache-why-unwritable = The file could not be written.
binary-cache-why-newer = Your options file is of a newer format than this machine reads: update this machine first.
binary-cache-why-unreadable = Your options file does not read, so it is not rewritten.
binary-cache-confirm-forget = Forget the push token on this machine? It goes from your belongings too, and nothing is pushed until you give a new one.
binary-cache-confirm-remove = Stop using this binary cache on your machines? Each stops downloading from it and pushing to it at its next update.
binary-cache-answer-no = no, keep it
binary-cache-answer-yes-forget = yes, forget the token
binary-cache-answer-yes-remove = yes, stop using it
binary-cache-said-not-a-name = A Cachix name is lower-case letters, digits and dashes, starting with a letter or a digit.
binary-cache-said-not-the-key = That key is not this cache's: it is the name, .cachix.org-1: and 44 characters, as cachix.org shows it.
binary-cache-said-not-let = The cache was named, and the consent for its hosts could not be kept: nothing will be pushed until it is.
binary-cache-said-named = The cache is { $name }; this machine uses it from its next update.
binary-cache-said-public = The cache is public: it receives only the distro's own outputs.
binary-cache-said-private = The cache is private: it receives this machine's system, read with your token.
binary-cache-said-token = The push token is kept, encrypted with your belongings.
binary-cache-said-forgotten = The push token is forgotten on this machine.
binary-cache-said-removed = No binary cache any more, from the next update.
binary-cache-said-netrc-written = This machine reads your private cache now: root's netrc is written.
binary-cache-said-netrc-removed = Root's netrc for the cache is gone: nothing holds your token there any more.
binary-cache-said-netrc-footer = Root's netrc is asked for in the sudo bar, above the modeline, if sudo wants you.
