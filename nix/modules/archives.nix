# Everything the terminal needs to open a compressed file.
#
# WHY IT IS A LIST AND NOT `atool` ALONE
# --------------------------------------
# `atool` and `bsdtar` dispatch by format, but they dispatch TO these binaries:
# without `unrar` installed, `atool` on a `.rar` fails with a message about
# atool. The `extract` function in `assets/shell/shellrc.sh` calls each tool
# directly for exactly that reason — the error then names the format that is
# missing, which is the one useful thing to say.
#
# `bsdtar` is the last resort in that function: it reads more formats than all
# the others together, and it is what keeps `extract` from ever being the thing
# that cannot open a file.
#
# `unrar` IS A PART (`nix/modules/parts.nix`): not free, so never in the
# system's closure — the session finds it in the parts' view once it has
# arrived, and `ronoctua-archive` asks for it by the part's stable path
# (`ronoctua_parts::file_of`). Until then a `.rar` is refused with its
# reason, and every other format opens.
{
  config,
  lib,
  options,
  pkgs,
  ...
}:

let
  # The parts' one nixpkgs that admits it (`nix/parts-pkgs.nix`).
  partsPkgs = config.ronoctua.partsPkgs or (import ../parts-pkgs.nix { inherit pkgs; });
in
{
  config = lib.mkMerge [
    # ── anchor: L25 ──
    # A PART in every profile, declared outside any condition: the archives
    # are everybody's.
    (lib.optionalAttrs (options.ronoctua ? parts) {
      ronoctua.parts.unrar = {
        wanted = true;
        kind = "program";
        unfree = true;
        name = "unrar";
        package = partsPkgs.unfree.unrar;
      };
    })
    {
      environment.systemPackages = with pkgs; [
        # ---- the classic ones ----
        gnutar
        gzip
        bzip2
        xz
        zstd
        lz4
        lzip
        cpio

        # ---- the everyday ones ----
        zip
        unzip
        # `7zz`: 7-Zip itself (also .iso, .dmg, .msi, .cab). Not `p7zip`, a fork
        # frozen at 16.02 — this is the one `ronoctua-archive` runs, by store path
        # (`nix/archive-tools.nix`), so the terminal and the file manager open an
        # archive with the same program.
        _7zz

        # ---- the dispatchers ----
        libarchive # `bsdtar`, the fallback of `extract`
        atool

        # ---- what a downloaded file turns out to be ----
        binutils # `ar`, for .deb
        dpkg
        rpm
        cabextract
        innoextract # installers built with Inno Setup
        squashfsTools # .snap, and the ISO's own squashfs
      ];
    }
  ];
}
