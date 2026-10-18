# The texts `ronoctua-shell-report` says, in en-GB (invariant 8).
#
# A key lives in the crate that says it, or in the nearest crate that every
# crate saying it stands on. Every declared language (`assets/i18n/languages.nix`)
# has a file here with exactly the same keys, the same arguments in each value
# and no empty value, and no key is in two fragments (`nix/scripts/gen-i18n.py`).
# The names are compiled (`key`, written from them by `nix/text-keys.nix`); the
# sentences are read at run time (`RONOCTUA_TEXTS`), so editing one compiles nothing.

files-no-editor = Emacs is not installed in this profile
files-nothing-opens = no application declares that it opens { $type }

## file info editing — everything the info panel can change

# Why an archive's work did not happen (`ronoctua_archive::Why`).
files-archive-why-no-tool = { $name }: this session was not given the program this archive needs
files-archive-why-unsupported = { $name }: this format cannot do that — a tar has no password and changes only by conversion
files-archive-why-needs-password = { $name } is encrypted, and needs its password
files-archive-why-wrong-password = That password does not open { $name }
files-archive-why-corrupt = { $name } is damaged, or is not an archive
files-archive-why-exists = { $name } already exists; nothing was replaced
files-archive-why-unstartable = the program for { $name } could not be started
files-archive-why-failed = the work on { $name } did not finish
files-archive-why-stopped = the work on { $name } was stopped, and what it had written was removed
security-online-vt-key = VirusTotal key
