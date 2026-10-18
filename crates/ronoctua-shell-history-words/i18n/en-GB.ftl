# The texts `ronoctua-shell-history-words` says, in en-GB (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

echo-atuin-synced = The history is synced
echo-atuin-refused = atuin said: { $reason }
echo-atuin-logged-out = Logged out, and the key is forgotten
atuin-word-auto = as atuin decides
atuin-word-compact = compact
atuin-word-directory = this directory
atuin-word-emacs = emacs
atuin-word-full = full
atuin-word-fulltext = anywhere in the line
atuin-word-fuzzy = fuzzy
atuin-word-global = everything
atuin-word-host = this machine
atuin-word-prefix = the start of the line
atuin-word-return-original = what you had typed
atuin-word-return-query = what you searched for
atuin-word-session = this session
atuin-word-skim = skim
atuin-word-subl = sublime
atuin-word-vim-insert = vim, insert
atuin-word-vim-normal = vim, normal
atuin-word-workspace = this project
