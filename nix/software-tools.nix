# THE PROGRAMS THE SOFTWARE CENTRE RUNS, BY PATH — one table, three readers.
#
# `ronoctua-software` never runs a program by name (`crate::Tools`): a user
# unit's `PATH` used to be minimal, and a bare name fails far from its cause.
# Every path arrives as a `RONOCTUA_*` variable, and THIS is where each
# variable gets its value. Three places need the table and none may carry a
# copy of it:
#
#   - `nix/modules/software.nix`, for the installed system: the CLI's wrapper,
#     the footer's unit and the queue's unit;
#   - `nix/dev.nix`, for the nested loop, which has no unit to read them from;
#   - `nix/shot.nix`, whose footer is started by hand and inherits nothing
#     from any module (`AGENTS.md` records that trap for the tag reader).
#
# A support the machine does not have is a NULL here and an ABSENT variable
# there, never a path to something missing: the crate turns an absent tool into
# `Refusal::SourceAbsent`, a sentence on screen, instead of an `exec` failing.
# That is also what keeps the reduced profile's closure honest — an attribute
# left out references nothing.
{
  lib,
  pkgs,
  # The Nix that fills this machine's store (`config.nix.package` on a system).
  nix,
  # The search index (`nix/software-index.nix`) and the revision it was built
  # from — run-time DATA, never a compilation input (`nix/rust.nix`).
  index,
  nixpkgsRev,
  flatpak ? null,
  # `programs.appimage.package`, which is an FHS environment named
  # `appimage-run`. `unsquashfs` travels with it: without a way to run an
  # AppImage there is no reason to open one.
  appimageRun ? null,
  waydroid ? null,
  # Chrome is a BOOLEAN, because what it becomes is a STABLE path and never a
  # store path: a web app's `.desktop` written today has to open the Chrome of
  # the next generation too, and a store path would open a deleted one.
  chrome ? false,
  # Our own binary, for "open this in a terminal". Stable for the same reason,
  # and because the shell's store path would drag the whole desktop into the
  # closure of the CLI — which the update runs as root.
  shell ? "/run/current-system/sw/bin/ronoctua-shell",
  # The superpack's theme root and every name in it, when the machine has it.
  icons ? null,
  iconNames ? null,
}:

{
  RONOCTUA_NIX = "${nix}/bin/nix";
  # The sandboxed trial (`Traces::Sandbox`).
  RONOCTUA_BWRAP = "${pkgs.bubblewrap}/bin/bwrap";
  # A trial is a transient user service, and a status is a question to the
  # user manager.
  RONOCTUA_SYSTEMD_RUN = "${pkgs.systemd}/bin/systemd-run";
  RONOCTUA_SYSTEMCTL = "${pkgs.systemd}/bin/systemctl";
  RONOCTUA_SHELL = shell;
  RONOCTUA_NIX_INDEX = "${index}/index.tsv";
  RONOCTUA_NIXPKGS_REV = nixpkgsRev;
}
// lib.optionalAttrs (flatpak != null) {
  RONOCTUA_FLATPAK = "${flatpak}/bin/flatpak";
}
// lib.optionalAttrs (appimageRun != null) {
  RONOCTUA_APPIMAGE_RUN = "${appimageRun}/bin/appimage-run";
  # Only `.desktop`, `.DirIcon` and `hicolor` come out of an AppImage, to
  # integrate it; the program itself is never unpacked.
  RONOCTUA_UNSQUASHFS = "${pkgs.squashfsTools}/bin/unsquashfs";
}
// lib.optionalAttrs (waydroid != null) {
  RONOCTUA_WAYDROID = "${waydroid}/bin/waydroid";
}
// lib.optionalAttrs chrome {
  # CHROME IS A PART (not free): reached through the session's view, at a
  # path that exists only once it arrived — whatever its interface.
  RONOCTUA_CHROME = (import ./parts-names.nix).viewBin "google-chrome-stable";
}
// lib.optionalAttrs (icons != null) {
  RONOCTUA_ICONS = icons;
}
// lib.optionalAttrs (iconNames != null) {
  RONOCTUA_ICON_NAMES = "${iconNames}";
}
