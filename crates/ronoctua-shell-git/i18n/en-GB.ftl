# The texts `ronoctua-shell-git` says, in en-GB (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

## file info editing — everything the info panel can change

files-git-title = History of { $name }
# The browser's columns.
files-git-kind-changes = Changes
files-git-kind-history = History
files-git-kind-refs = Branches and tags
files-git-kind-stashes = Stashes
files-git-kind-remotes = Remotes
files-git-kind-count = { $kind } { $n }
files-git-file-history-title = History of { $path }
files-git-reading = Reading…
files-git-empty-changes = Nothing changed in the work tree.
files-git-empty-history = No commits yet.
files-git-empty-refs = No branches and no tags.
files-git-empty-stashes = Nothing is stashed.
files-git-empty-remotes = No remotes.
files-git-empty-file = No commit touched this file.
files-git-pick-item = Pick something on the left to see it here.
files-git-pick-file = Pick a file to see what changed in it.
files-git-diff-no-files = A remote changes no files.
files-git-diff-nothing = No change in the content.
files-git-diff-staged = Staged
files-git-diff-unstaged = Not staged
files-git-diff-untracked = Not in the repository yet
files-git-diff-binary = Not text: nothing to show line by line.
files-git-diff-no-content = Only the file's details changed, not what is in it.
files-git-diff-link = A symbolic link, and where it points:
files-git-diff-special = Not a file — a pipe, a socket or a device — so nothing in it is read.
files-git-blame-title = Who wrote each line of { $path }
files-git-files = { $n ->
        [one] 1 file changed
       *[other] { $n } files changed
    }
files-git-remote-branches = Its branches, as last fetched
files-git-no-remote-branches = None fetched yet.
files-git-fact-path = file
files-git-fact-state = state
files-git-fact-from = renamed from
files-git-fact-name = name
files-git-fact-kind = kind
files-git-fact-upstream = follows
files-git-fact-fetch = fetches from
files-git-fact-push = pushes to
files-git-fact-stash = stash
files-git-fact-commit = commit
files-git-fact-author = author
files-git-fact-date = date
files-git-fact-parents = parents
files-git-ref-branch = a branch
files-git-ref-remote = a remote's branch
files-git-ref-tag = a tag
files-git-mark-branch = branch
files-git-mark-remote = remote
files-git-mark-tag = tag
files-git-change-added = added
files-git-change-modified = changed
files-git-change-deleted = deleted
files-git-change-renamed = renamed
files-git-change-copied = copied
files-git-change-type = changed its type
files-git-change-unmerged = unmerged
files-git-state-untracked = not in the repository yet
files-git-state-conflicted = in conflict
files-git-state-staged = staged: { $what }
files-git-state-unstaged = not staged: { $what }
# What the browser says after an action.
files-git-fetched = Fetched: the remotes' branches are up to date.
files-git-fetch-refused = Nothing was fetched. { $why }
files-git-fetching = A fetch is already under way.
files-git-no-file = There is no file under the cursor.
files-git-blame-gone = The file is not in the work tree any more, so nobody wrote its lines there.
files-git-nothing-to-copy = There is no commit under the cursor.
files-git-no-version = There is no older version of this file to open.
files-git-kept = Opened { $path } as { $id } has it — a copy that cannot be changed.
files-git-keep-failed = The copy could not be made: { $why }
files-git-keep-why-disk = The disk would not take it: it is full, or the cache cannot be written there.
# The browser's keys: the word the hint line shows, and what the pointer
# resting on it is told.
files-git-down = down
files-git-up = up
files-git-enter = open
files-git-next = next column
files-git-previous = previous column
files-git-page-down = page down
files-git-page-up = page up
files-git-changes = changes
files-git-history = history
files-git-refs = branches
files-git-stashes = stashes
files-git-remotes = remotes
files-git-file-history = file's history
files-git-blame = who wrote it
files-git-copy = copy id
files-git-open-version = open this version
files-git-fetch = fetch
files-git-terminal = terminal
files-git-left = left
files-git-right = right
about-files-git-down = Move the cursor one row down; in the diff, move the text one line.
about-files-git-up = Move the cursor one row up; in the diff, move the text one line.
about-files-git-enter = Go one column deeper: from an item to what it is, from a file to what changed in it.
about-files-git-next = Give the keyboard to the column on the right.
about-files-git-previous = Give the keyboard to the column on the left.
about-files-git-page-down = Move a whole screen down in the column that has the keyboard.
about-files-git-page-up = Move a whole screen up in the column that has the keyboard.
about-files-git-changes = List what the work tree has that the last commit does not.
about-files-git-history = List the commits, newest first.
about-files-git-refs = List the branches, the remotes' branches and the tags.
about-files-git-stashes = List what was stashed away.
about-files-git-remotes = List the remotes and where each one is.
about-files-git-file-history = List only the commits that changed the file under the cursor, across its renames.
about-files-git-blame = Show who last wrote each line of the file as the work tree has it; the same key goes back to the diff.
about-files-git-copy = Copy the id of the commit under the cursor.
about-files-git-open-version = Open the file as the commit had it, as a copy that cannot be changed.
about-files-git-fetch = Bring the remotes' new commits here, without changing any of yours. It never asks for a password.
about-files-git-terminal = Open a terminal at the repository's top folder.
about-files-git-left = Move the diff's text to the left.
about-files-git-right = Move the diff's text to the right, to read a long line.
cmd-dev-clone = Clone the distro's repository into a folder
ask-dev-clone = Clone from { $host } into
dev-clone-cloning = { $host } is now allowed. Cloning the distro from { $host } into { $folder }…
dev-clone-running = A clone into { $folder } is still running.
dev-clone-done = The distro is in { $folder }.
dev-clone-failed = Nothing was cloned into { $folder }: { $why }
dev-clone-occupied = Something is already at { $folder }: nothing was cloned.
dev-clone-no-address = This build was not told the distro's address, so there is nothing to clone.
dev-clone-no-folder = No folder was given: nothing was cloned.
dev-clone-not-let = { $host } could not be let, so nothing was asked.
dev-clone-unheard = The footer could not hear the clone's answer, so it was not started.
dev-clone-unkept-no-home = There is no home folder to keep the consent in: nothing was cloned.
dev-clone-unkept-unparseable = The consent file (consent.json) will not parse; it was left as it is, and nothing was cloned.
dev-clone-unkept-unwritable = The consent file could not be written: nothing was cloned.
dev-clone-elsewhere = Clone from { $host } (not { $from }) into
dev-clone-unread = git could not read where your settings send the distro's address, so nothing was asked.
dev-clone-reading = Reading where your git settings send the distro's address…
dev-clone-no-host = Your git settings send the distro's address to { $to }, which is no host to ask: nothing was cloned.
