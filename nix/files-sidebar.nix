# WHAT THE FILE MANAGER'S SIDEBAR OPENS WITH, in ONE place.
#
# KEYS, never paths. Each one names an entry of `nix/user-dirs.nix` — except
# `start`, which is `nix/files-start.nix`, the directory the manager itself
# opens in. So the day somebody moves where projects live, the sidebar moves
# with them; written as a second list of paths, it would keep pointing at the
# old one and nothing would say so.
#
# It is also invariant 13: a path here would be `/home/<somebody>/…`, and this
# repository is public. A key is not a machine.
#
# WHY A FILE OF ITS OWN, and not a literal in `nix/modules/files.nix`
# -------------------------------------------------------------------
# Same reason as `nix/files-start.nix`, which says it: two different kinds of
# thing read it. `nix/modules/files.nix` makes it a NixOS option's default, and
# `nix/dev.nix` exports it into the nested loop, which is not a NixOS system and
# cannot read an option. Written twice, the loop and the installed system would
# show different sidebars the first time somebody changed one.
#
# WHAT IS NOT HERE
# ----------------
# The trash and the devices. Neither is a directory somebody chose: the trash is
# wherever the freedesktop spec puts it (`ronoctua-files`'s `trash::Bin`), and a
# device is whatever is plugged in right now. Listing them here would be
# pretending they are configuration.
#
# And a key whose directory is not on the machine simply does not appear. That
# is the same silence `listing::start_at` already keeps for the start directory:
# a row that leads nowhere is worse than a row that is missing.
[
  # The home of what one does — `main`. It is `nix/files-start.nix`, which is
  # also where the manager opens, so the first row of the sidebar is the place
  # one is already standing.
  "start"
  # `main/temp` — and it is the DOWNLOAD directory, which is the whole reason
  # it earns a row: it is where a browser, a transfer and a capture-to-file all
  # put things one then has to do something with.
  "download"
  # `main/prj`.
  "projects"
  # `main/media/screenshot`.
  "screenshot"
]
