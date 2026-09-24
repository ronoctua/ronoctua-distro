# WHERE THE PERSON'S THINGS LIVE — section 2, in ONE place.
#
# Names RELATIVE to home, never paths. An absolute path would name somebody's
# machine, and this repository is public (invariant 13). It is the same rule
# `nix/files-start.nix` states, and for the same reason.
#
# WHY A FILE OF ITS OWN
# ---------------------
# Because three different kinds of thing read it, and only one of them is a
# NixOS module:
#
#   - `nix/modules/home.nix` turns it into `xdg.userDirs`, which is what writes
#     `~/.config/user-dirs.dirs` — the file a dialog, a browser download and
#     `xdg-open` actually read;
#   - `nix/modules/scroll.nix` takes `screenshot` for `RONOCTUA_SCREENSHOTS`,
#     which is where the footer builds a capture's name — so a capture lands
#     where the distro says;
#   - `nix/files-sidebar.nix` names WHICH of these the file manager's sidebar
#     opens with, by KEY — so the sidebar cannot drift from the directory.
#
# Written in each of those places instead, the day somebody moved `projects`
# the sidebar would keep pointing at the old one and nothing would say so.
# It is the same shape as `nix/keyboard-xkb.nix`: one file, several readers.
#
# `screenshot` IS NOT A FREEDESKTOP DIRECTORY, and it is here anyway.
# -------------------------------------------------------------------
# The spec has eight; `projects` was already a ninth, for the reason written in
# `nix/modules/home.nix`. This is the tenth, and it exists because a capture
# has to land SOMEWHERE and nothing else declares where. The compositor takes
# no screenshots at all — `grim` does, started by the footer — so there is no
# upstream default to contradict any more; what there is instead is a name the
# footer builds, and it builds it from here. A directory the system writes into
# and never declares is a directory nobody can point at.
#
# SINGULAR, like its four siblings under `media/`: `image`, `video`,
# `audio-music`, `video-music`. A plural among them would read as a different
# kind of thing.
{
  desktop = "main/temp";
  documents = "main/archive";
  download = "main/temp";
  music = "main/media/audio-music";
  pictures = "main/media/image";
  publicShare = "main/share/public";
  templates = "templates";
  videos = "main/media/video";
  # MUSIC VIDEOS, the eleventh: the player's third place, beside `music` and
  # `videos`. Named like its sibling `audio-music` from the other side.
  musicVideo = "main/media/video-music";
  projects = "main/prj";
  screenshot = "main/media/screenshot";
}
