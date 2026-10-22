# The texts `ronoctua-shell-belongings` says, in en-GB (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

## file info editing — everything the info panel can change

# THE BELONGINGS (`LEADER s v`, `crate::belongings`): the person's
# configuration, kept encrypted in a private repository they name.
group-belongings = belongings
belongings-title = Belongings
cmd-belongings-open = Belongings: the configuration kept in your own repository
cmd-belongings-window = Belongings, in a window
cmd-belongings-sync = Sync the belongings now: asks the passphrase
cmd-belongings-up = Previous row
cmd-belongings-down = Next row
cmd-belongings-less = The previous value of the row
cmd-belongings-more = The next value of the row
cmd-belongings-enter = What the row under the cursor does
cmd-belongings-keep-local = Keep this machine's side of the conflict: it goes over the repository's
cmd-belongings-take-remote = Take the repository's side of the conflict: this machine's is set aside
cmd-belongings-keep-both = Keep both sides: the repository's lands beside this one, with the other machine's name
cmd-belongings-apply = Apply the decisions and finish the sync
cmd-belongings-abandon = Drop the prepared sync: nothing is written, and the passphrase is forgotten
cmd-belongings-all-local = Every conflict: keep this machine's side
cmd-belongings-all-remote = Every conflict: take the repository's side
cmd-belongings-repository = Set the repository's address
cmd-belongings-add = Keep another file or folder of your home
cmd-belongings-auth = Reach the repository by an SSH deploy key, or by a token
cmd-belongings-keygen = Make this machine's SSH deploy key
cmd-belongings-copy-key = Copy the deploy key's public half, to give it to the repository
cmd-belongings-token = Keep a token for the repository, readable by you alone
cmd-belongings-user = The user name the repository's host asks for
cmd-belongings-passphrase = Change the passphrase: asks the one in use, then the new one twice
cmd-belongings-period = How often you are reminded to sync
belongings-up = up
belongings-down = down
belongings-less = less
belongings-more = more
belongings-enter = do
belongings-sync = sync
ask-belongings-sync = Passphrase of the belongings
ask-belongings-repository = The repository's address: git@host:path, ssh://… or https://…
ask-belongings-add = A file or folder of your home to keep
ask-belongings-token = The token, as the repository's host gave it
ask-belongings-user = The user name the host asks for
ask-belongings-passphrase = The passphrase in use
ask-belongings-passphrase-new = The new passphrase
ask-belongings-passphrase-again = The new passphrase again
ask-belongings-sync-again = The passphrase again — a new repository keeps it for good
about-belongings-conflict = Shows what differs between the two sides, when both are text
about-belongings-rule = Takes it out of what is kept on every machine you sync
about-belongings-kept = Stops keeping it: the file stays where it is
about-belongings-excluded = Keeps it again
about-belongings-never = Keeps it anyway: it goes to every machine you sync
belongings-status = Status
belongings-repository = Repository
belongings-none-yet = none yet
belongings-last-sync = Last sync
belongings-never-here = never on this machine
belongings-next = Next reminder
belongings-due-now = now
belongings-no-reminder = no reminders
belongings-kept-now = Kept now
belongings-files-weigh = { $n ->
    [one] one file, { $size }
   *[other] { $n } files, { $size }
    }
belongings-last-run = Last run
belongings-first-run = This machine has not synced with this repository yet. The first sync restores what the repository keeps; where a file here differs, the repository's side starts chosen.
belongings-outcome-synced = In step: { $pulled } written here, { $pushed } sent, { $deleted } deleted.
belongings-outcome-waiting = { $n ->
    [one] Waiting for your decision on one conflict.
   *[other] Waiting for your decisions on { $n } conflicts.
    }
belongings-outcome-abandoned = The prepared sync was dropped.
belongings-prepared-gone = A sync was prepared here and is gone — the session ended since — so nothing waits for you: sync again.
belongings-outcome-keyed = A deploy key was made.
belongings-outcome-rekeyed = The passphrase was changed.
belongings-large = { $name } weighs { $size }: every sync carries it.
belongings-unreadable = { $n ->
    [one] One file here could not be read, and was left as it is on both sides.
   *[other] { $n } files here could not be read, and were left as they are on both sides.
    }
belongings-unwritten = { $n ->
    [one] One file from the repository could not be written here; the next sync tries again.
   *[other] { $n } files from the repository could not be written here; the next sync tries again.
    }
belongings-sync-heading = Sync
belongings-sync-now = Sync now
belongings-apply = Apply the decisions
belongings-decided-of = { $n } of { $of } decided
belongings-abandon = Drop this sync
belongings-waiting-note = The sync is prepared and waits for you: nothing here was written yet.
belongings-plan-waits = A sync is prepared and waits for your decisions: apply it or drop it first.
belongings-conflicts = { $n ->
    [one] One conflict
   *[other] { $n } conflicts
    }
belongings-conflicts-note = Changed here and in the repository since the last sync. Decide each: keep this machine's, take the repository's, or — for what you added — keep both.
belongings-all-keep-local = All: keep this machine's
belongings-all-take-remote = All: take the repository's
belongings-undecided-left = { $n ->
    [one] One conflict has no decision yet.
   *[other] { $n } conflicts have no decision yet.
    }
belongings-undecided = undecided
belongings-chosen = chosen
belongings-sides = here: { $here } · repository: { $there }, changed on { $machine }
belongings-side = { $size }, { $when }
belongings-side-deleted = deleted
belongings-diff-title = Difference in { $name }
belongings-diff-note = The lines that begin with - are this machine's; the ones that begin with + are the repository's.
belongings-what-leaks = Everything but the plain part below is encrypted with your passphrase, post-quantum, before it leaves this machine, cut into pieces. Of that, the repository shows how many pieces there are, their sizes, which content repeats and when each sync happened; never the files' names, their places or what is in them.
belongings-added-by-you = added by you
belongings-added-elsewhere = added on another machine
belongings-taken-out = taken out by you
belongings-add = Keep something else
belongings-large-note = What you add is kept whole, folders and links included. Past { $size }, the page says so: every sync carries it.
belongings-never = Never kept
belongings-access = Access
belongings-auth = Reached by
belongings-deploy-key = Deploy key
belongings-copy-it = copy the public key
belongings-make-it = make one
belongings-deploy-note = Give this public key write access to the repository, as a deploy key. The private half stays on this machine, readable by you alone, and is never kept in the belongings.
belongings-keygen-note = This machine has no deploy key yet. Making one takes a moment on the queue; then its public half appears here to copy.
belongings-token = Token
belongings-token-is-kept = kept, readable by you alone
belongings-user = User name
belongings-use-ssh = Use the SSH address
belongings-user-any = any: the server knows you by the token
belongings-token-note = The token is kept in a file only you can read, and reaches git through its path alone — never on a command line.
belongings-passphrase = Passphrase
belongings-change-passphrase = Change the passphrase
belongings-passphrase-note = The passphrase opens one small file, and that file opens the rest: changing it encrypts that file again, on every machine at once. Nothing recovers a passphrase that is lost.
belongings-period = Reminder
belongings-remind-every = Remind me to sync
belongings-needs-key = There is no deploy key yet: make one under Access, and give it write access to the repository.
belongings-needs-token = There is no token yet: keep one under Access.
belongings-nothing-prepared = There is no prepared sync.
belongings-no-conflict-here = The cursor is not on a conflict.
belongings-both-only-added = Both sides can be kept only for what you added yourself.
belongings-no-difference = The difference of this file is not here any more; sync again to see it.
belongings-not-text = This file is not text, so there is no difference to show: decide by its size and time.
belongings-key-already = This machine has a deploy key already.
belongings-no-key-yet = There is no deploy key yet.
belongings-passphrase-first = The first sync sets the passphrase: there is none to change yet.
belongings-token-kept = The token is kept.
belongings-passphrases-differ = The two new passphrases differ, so nothing was changed.
belongings-first-passphrases-differ = The two passphrases differ, so nothing was sent.
belongings-queued = Sent to the queue: { $what }.
belongings-reason-draft = a draft, or a running program's number
belongings-repair-why-unknown-variant = a word this machine does not know — most often a newer machine's
belongings-outcome-compacted = The history was compacted: { $before } became { $after } here. The host may keep the old copies until its own cleanup.
belongings-outcome-nothing-to-compact = There was nothing to compact: every version in the history is from the last { $days ->
        [one] day
       *[other] { $days } days
    }, and no piece is left over.
belongings-consent-for = Syncing the belongings reaches the repository on this host.
belongings-unsigned = changed here, waiting for your passphrase to be signed
belongings-sign = Sign now
cmd-belongings-sign = Sign what you changed here with your key now, with no network, so this machine and the others take it
ask-belongings-sign = Passphrase of the belongings, to sign
belongings-signature = Your signature
belongings-signature-note = What travels in clear is signed with your key, made with your repository and kept under your passphrase. A machine takes in by itself only what that key signed — the key it pinned, never the copy in the repository; anything else waits, with the reason.
belongings-signature-key = Your key
belongings-signature-key-unknown = not known here yet: a sync with your passphrase shows it
belongings-signature-machine = This machine
belongings-signature-pinned = trusts your key
belongings-signature-pinned-some = trusts a key: a sync with your passphrase says whether it is yours
belongings-signature-other = trusts another key: pin yours
belongings-signature-none = trusts no key yet: your options and brain modules wait
belongings-signature-last = Last delivery
belongings-signature-refused = refused
belongings-signature-sync-first = Sync once with your passphrase first: that is what shows your key here.
belongings-signature-already = This machine already trusts your key.
belongings-pinning = Pinning
belongings-pinning-now = asking sudo…
belongings-pinning-done = this machine trusts your key now
belongings-pinning-not = not taken: the journal says why
belongings-pinning-busy = A pin is on its way.
belongings-pin = Pin your key on this machine
cmd-belongings-pin = Pin the key your passphrase opened on this machine, through sudo: this machine then takes by itself what that key signed
belongings-outcome-plain-synced = The plain part is in step: { $pulled } written here, { $pushed } sent.
cmd-belongings-take-merged-ours = Merge the conflict, keeping this machine's side where the two clash
cmd-belongings-take-merged-theirs = Merge the conflict, taking the repository's side where the two clash
belongings-reason-person-data = what you declared: programs, machines, options
belongings-reason-brain-module = your own brain modules
belongings-plain-note = Kept in clear, not encrypted: the machine reads it before anybody types the passphrase. Whoever can read the repository can read it; whoever can write to it cannot make a machine take a change your key did not sign — that waits, with the reason. A file past { $size } stays out.
belongings-no-merge-here = No merge reached this conflict: keep one side, or take the other.
belongings-sides-unnamed = here: { $here } · repository: { $there }
belongings-plain-conflicts = { $n ->
        [one] One file of it changed here and in the repository: the next sync asks you.
       *[other] { $n } files of it changed here and in the repository: the next sync asks you.
    }
belongings-merge-hunks = { $n ->
        [one] Merged, but one part of the text changed on both sides.
       *[other] Merged, but { $n } parts of the text changed on both sides.
    }
belongings-merge-fields = { $n ->
        [one] Merged, but one field changed on both sides: { $fields }
       *[other] Merged, but { $n } fields changed on both sides: { $fields }
    }
# The predefined targets, by id (`assets/belongings.nix`'s `targets`): what the
# list of what goes to the repository names them.
belongings-target-ssh = your SSH keys and settings
# Beside the targets and the passphrase: what the passphrase guards now.
belongings-targets-note = Your SSH keys, your GnuPG keyring and your security keys travel encrypted under this passphrase: it now guards them too, so make it a strong one.
# What a sync that brought a target's files offers ("no" drawn first).
belongings-offer-ssh-signing = Sign your git commits and tags with { $key }? Every machine that has this key will; the others go on unsigned.
belongings-offer-pam-u2f = Bring the security keys you registered on your other machines to this one? Each answers sudo here in place of your password, which keeps working.
belongings-offer-vscode-extensions = { $n ->
    [one] Reinstall the VS Code extension of your list that this machine lacks?
   *[other] Reinstall the { $n } VS Code extensions of your list that this machine lacks?
  } They come from the Visual Studio Marketplace, one at a time, in the transfer queue, and yes lets { $hosts ->
    [one] this host:
   *[other] these { $hosts } hosts:
  }
# Your VS Code extensions on this machine: what the last install did, and what is missing.
belongings-extensions = Your VS Code extensions
belongings-extensions-missing = { $n ->
    [one] One extension of your list is missing here. Installing it lets { $hosts ->
        [one] this host:
       *[other] these { $hosts } hosts:
      }
   *[other] { $n } extensions of your list are missing here. Installing them lets { $hosts ->
        [one] this host:
       *[other] these { $hosts } hosts:
      }
  }
belongings-extensions-install = { $n ->
    [one] Install it
   *[other] Install them
  }
belongings-extensions-installing = Installing in the transfer queue…
belongings-extensions-stopped = The last install stopped before it ended: what entered is installed, and the rest is still missing.
belongings-extensions-entered = { $n ->
    [0] No extension entered.
    [one] One extension entered.
   *[other] { $n } extensions entered.
  }
belongings-extensions-failed = { $n ->
    [one] One could not be installed, and VS Code said why:
   *[other] { $n } could not be installed, and VS Code said why:
  }
belongings-extensions-refused = Not installed: { $why }
belongings-extensions-refused-no-code = VS Code is not on this machine
belongings-extensions-refused-newer = your list of extensions was written by a newer machine: update this one first
belongings-extensions-refused-malformed = your list of extensions does not read, and nothing was installed
belongings-extensions-refused-not-let = these hosts were not let, and nothing was asked:
belongings-extensions-refused-busy = another install of them was running
belongings-extensions-refused-files = a file of VS Code's here could not be read or written; the system log says which
# The identity kept for the session (`keep_unlocked`).
belongings-unlock-switch = Remember the passphrase for this session
belongings-unlock-note = Kept in the kernel's memory, never on disk, until you log out, the machine sleeps or { $hours } hours pass.
belongings-unlock-kept = Remembered until { $until }: a sync asks nothing.
# The one listing of what goes to the repository (`inventory`), its groups,
# its rows and the states of a file.
belongings-listing = What goes to your repository
belongings-counting = Counting what goes to the repository…
belongings-item = { $state } · { $size }
belongings-here-not-here = not on this machine
belongings-here-nowhere = on no machine
belongings-never-because = never: { $reason }
# A predefined target: where it is kept, and what may be done about it.
belongings-target-everywhere = on every machine
belongings-target-partly-here = on every machine, part of it not on this one
belongings-target-asked-nowhere = on no machine, from the next sync
belongings-target-asked-everywhere = on every machine again, from the next sync
belongings-target-act-not-here = Not on this machine
belongings-target-act-nowhere = Not on any machine
belongings-target-act-here-again = Keep it on this machine again
belongings-target-act-everywhere = Keep it on every machine again
belongings-nowhere-question = Stop keeping { $target } on every machine? Its copies leave the repository at the next sync; every other machine forgets them and keeps its own files.
about-belongings-not-here = Stops keeping it on this machine: the repository's copy stays for the others
about-belongings-nowhere = Stops keeping it on every machine — asks first
about-belongings-everywhere = Keeps it on every machine again, from the next sync
# Where this machine's Emacs reads its configuration, when it is not the
# one place the belongings carry (`ronoctua_belongings::emacs`): said, never
# acted on.
belongings-emacs-reads = Emacs here reads { $place }, but your belongings carry { $carried }: move your configuration there and it travels.
belongings-emacs-hidden = { $place } holds nothing of yours, only what Emacs makes, and while it is there Emacs reads it in place of { $carried }, which your belongings carry.
belongings-emacs-no-file = { $place } is there but is no file Emacs loads, and while it is there Emacs reads { $folder } in place of { $carried }, which your belongings carry.
belongings-emacs-xdg = With your $XDG_CONFIG_HOME, Emacs here looks for its configuration in { $place } and in your home, never in { $carried }, which your belongings carry.
# A question on a page of its own, "no" first.
belongings-answer-no = No
belongings-answer-yes = Yes
about-belongings-answer-no = Leaves everything as it is
about-belongings-answer-yes = Does what the question says
# What the last sync offered to turn on.
belongings-offers = Offered by the last sync
belongings-offer-turning = Turning it on…
belongings-offer-taken = Turned on.
belongings-offer-refused = Not turned on: { $why }
about-belongings-offer-no = Leaves it off: it is offered again only when the key changes on another machine
about-belongings-offer-yes = Turns it on, on this machine
about-belongings-offer-no-extensions = Leaves them out: offered again only when your list changes on another machine, and the page keeps a row that installs them
about-belongings-offer-yes-extensions = Lets the hosts above and installs them on this machine, in the transfer queue
about-belongings-extensions-install = Lets the hosts above and installs them on this machine, in the transfer queue, one at a time
# The files a lenient read repaired.
belongings-repairs = Repaired when read
belongings-repairs-note = This machine could not read part of these files. What it could not read stays in each file as written, and the machine went on without it. Write the fix to put this machine's values in its place, or leave the file as it is.
belongings-repairs-newest = This machine runs the newest system of your machines: the next sync writes these fixes by itself, and sends them.
belongings-repairs-older = Another of your machines runs a newer system and may know these values: decide here.
belongings-repair-count = { $n ->
    [one] one value
   *[other] { $n } values
    }
belongings-repair-line = { $pointer }: { $why }
belongings-repair-whole-file = the whole file
belongings-repair-leave = Leave it
belongings-repair-write = Write the fix
belongings-repair-written = The fix of { $file } is written.
belongings-repair-stale = { $file } changed since it was read: what is left to repair is asked again when it is read.
about-belongings-write-fix = Writes this machine's values in place of what it could not read
about-belongings-leave-it = Leaves the file as it is, and stops asking until it changes
# The history: its weight and its compaction.
belongings-history = History
belongings-weighs = Weighs
belongings-weighs-value = { $tree } of files, { $history } with their history
belongings-weight-heavy = The history weighs { $history }: past { $recommend }, compacting it is recommended.
belongings-weight-compact = Compacting the history is recommended: it weighs { $history }, much more than what it keeps.
belongings-compact = Compact the history
belongings-compact-keeps = keeps { $days ->
    [one] the last day
   *[other] the last { $days } days
    }
belongings-compact-note = Every file stays as it is now; older versions leave the repository, for every machine. The host may keep the old copies until its own cleanup.
belongings-compact-question = Older versions leave the repository for good, for every machine; every file as it is now stays, and the host may keep the old copies until its own cleanup. Compact the history to { $days ->
    [one] its last day?
   *[other] its last { $days } days?
    }
ask-belongings-compact = The passphrase, to compact the history
about-belongings-compact = Compacts the history — asks first
about-belongings-use-ssh = Swaps the address for the same repository's SSH one, which a deploy key reaches
# Whether the session remembers the passphrase.
belongings-unlock-on = yes
belongings-unlock-off = no
about-belongings-unlock = Whether a sync asks the passphrase once a session, or every time
# Every field of a JSON conflict, decided on its own.
belongings-field-ours = This machine's value
belongings-field-theirs = The repository's value
belongings-field-none = none
belongings-field-chosen = { $value } · chosen
belongings-field-before = before: { $value }
belongings-decided-fields = merged field by field
belongings-fields-decided-of = { $n } of { $of } fields decided
belongings-field-only-two = A field takes this machine's value or the repository's.
about-belongings-field = Decides this field on its own: its two values are the rows below
about-belongings-field-ours = Keeps this machine's value of this field
about-belongings-field-theirs = Takes the repository's value of this field
