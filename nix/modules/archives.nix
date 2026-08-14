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
{ pkgs, ... }:

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
    p7zip # `7z`: also opens .iso, .dmg, .msi, .cab
    unrar # not free software, and the only thing that opens .rar properly

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
