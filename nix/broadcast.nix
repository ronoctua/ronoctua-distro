# The player's channels by theme, from Nix to Rust — the twin of `nix/world.nix`.
#
# `assets/channels.nix` becomes three things here:
#
#   broadcast.rs  the table `ronoctua_broadcast::table` compiles: the desks'
#                 categories, and the catalogue's themes (with their aliases),
#                 default sources (with the categories each asks for, its
#                 consent hosts and its poster hosts), adult words and bounds
#   keys.txt      the text keys BOTH languages have to carry — a theme's
#                 `player-channels-<id>` and a default source's
#                 `player-channels-source-<id>`
#   a refusal     an iptv-org category that does not exist takes the build down,
#                 and so does a theme nothing reaches, an alias two themes
#                 claim, a host that is not a host's name
#
# The third is the one that matters: the index is filtered by those ids at run
# time, and a typo would be a TV row that is always empty.
{
  lib,
  stdenvNoCC,
  python3,
  writeText,
  fetchurl,
  channelsFile,
}:

let
  # IPTV-ORG'S OWN LIST OF CATEGORIES, from a CAPTURE.
  #
  # The file is published at a live address that is rebuilt every day
  # (`iptv-org.github.io/api/…`), so a hash pinned against it holds only until
  # the store copy is collected — the defect `nix/atlas.nix` records. A
  # Wayback Machine capture with `id_` answers the original bytes and does not
  # change.
  #
  # The stamp is the CAPTURE'S OWN, the one the archive answers with a 200 and
  # no redirect. A rounded date (`20260601000000`, which this read first) is
  # resolved by the archive to whatever capture is NEAREST on the day it is
  # asked — here through a second capture that was itself a redirect — so the
  # bytes behind it are the archive's choice, not this file's. Refreshing it is
  # choosing a newer capture (`curl -sI` on a rounded stamp names it in
  # `location:`) and its hash.
  #
  # AND THE BYTES ARE UNWRAPPED. Since 2026-10 the capture answers the body
  # GitHub sent it — gzip — with no `content-encoding` to say so, where it used
  # to answer it decoded: the same JSON in another wrapping, and a hash
  # mismatch. Undone here when the body starts with gzip's magic, so the hash
  # is the JSON's whichever way the archive answers.
  categories = fetchurl {
    url = "https://web.archive.org/web/20260523230739id_/https://iptv-org.github.io/api/categories.json";
    hash = "sha256-KvRdQ7HatUPMAESUrL/QnJD0y7GiJOEh1FWssAGGcbg=";
    postFetch = ''
      if [ "$(head -c 2 "$out" | od -An -tx1 | tr -d ' \n')" = 1f8b ]; then
        gzip -dc < "$out" > "$out.json"
        mv "$out.json" "$out"
      fi
    '';
  };
in
stdenvNoCC.mkDerivation {
  pname = "ronoctua-broadcast-rs";
  version = "1";

  nativeBuildInputs = [ python3 ];
  dontUnpack = true;
  dontConfigure = true;

  buildPhase = ''
    runHook preBuild
    mkdir -p $out
    python3 ${./scripts/gen-broadcast.py} \
      --channels ${writeText "channels.json" (builtins.toJSON (import channelsFile))} \
      --iptv-categories ${categories} \
      --out $out/broadcast.rs \
      --out-keys $out/keys.txt
    runHook postBuild
  '';

  dontInstall = true;

  # What was downloaded to make this, for `nix/kept-sources.nix` to root.
  passthru.sources = [ categories ];

  meta = {
    description = "The player's channels by theme, generated from assets/channels.nix";
    platforms = lib.platforms.all;
  };
}
