# The texts `ronoctua-shell-git-line` says, in en-GB (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

files-copied = copied
files-not-copied = could not copy

## file info editing — everything the info panel can change

# THE GIT LINE under the file manager's path, and the git browser `C-h`
# opens (`git_line.rs`, `git_browser.rs`). What is quoted is data: a branch,
# a subject, a path.
files-git = history
about-files-git = Open the repository's history: its changes, commits, branches, stashes and remotes, beside what each one changed.
files-git-detached = detached at { $id }
files-git-unborn = { $branch }, no commits yet
files-git-changed = { $n ->
        [one] 1 changed
       *[other] { $n } changed
    }
files-git-staged = { $n ->
        [one] 1 staged
       *[other] { $n } staged
    }
files-git-untracked = { $n ->
        [one] 1 untracked
       *[other] { $n } untracked
    }
files-git-conflicted = { $n ->
        [one] 1 in conflict
       *[other] { $n } in conflict
    }
files-git-clean = nothing changed
files-git-list-item = { $item },
files-git-subject = “{ $subject }”
files-git-line-explain = On { $branch }: { $changed } changed, { $staged } staged, { $untracked } untracked. { $keys } or a click opens the history.
files-git-line-refused = This folder is in a repository git would not read. { $keys } opens the history, which says why.
files-git-age-now = just now
files-git-age-minutes = { $n ->
        [one] a minute ago
       *[other] { $n } minutes ago
    }
files-git-age-hours = { $n ->
        [one] an hour ago
       *[other] { $n } hours ago
    }
files-git-age-days = { $n ->
        [one] yesterday
       *[other] { $n } days ago
    }
files-git-age-months = { $n ->
        [one] a month ago
       *[other] { $n } months ago
    }
files-git-age-years = { $n ->
        [one] a year ago
       *[other] { $n } years ago
    }
# Why git answered nothing (`ronoctua_git::Why`), one per reason.
files-git-why-no-git = This session was not given git, so the repository cannot be read.
files-git-why-no-ssh = This remote is reached over SSH and this session was not given ssh.
files-git-why-no-askpass = This remote wants a token and the helper that hands it over is missing.
files-git-why-unstartable = git could not be started.
files-git-why-failed = git could not read this.
files-git-why-garbled = git answered something this browser does not understand.
files-git-why-needs-credentials = The remote wants a name and a password or a key, and fetching never asks for one.
files-git-why-refused = The remote refused the credentials it was given.
files-git-why-unreachable = The remote could not be reached.
files-git-why-too-large = It is larger than a view shows.
files-git-why-not-post-quantum = The remote's server offered no post-quantum key exchange, and this connection asks for one: nothing was sent.
files-git-why-no-common-key-exchange = The remote's server offers only old key exchanges that this machine no longer accepts: it was reached, and nothing was sent.
