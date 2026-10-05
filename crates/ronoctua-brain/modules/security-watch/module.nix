# The security watch declares nothing of its own: it runs no unit. What it
# reads is declared elsewhere — the journal feed by the brain's core (its
# `journalctl` by store path), the folders and the log prefix by
# `nix/modules/security.nix` — and `ronoctua.brain.modules.<id>.enable =
# false` takes the watch away with the brain's gate around this file.
{ ... }:
{ }
