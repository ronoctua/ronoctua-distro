# The world, baked into one table this system can read.
#
# The shape of `nix/width.nix`, one subject along: a file somebody else
# publishes, a Python script, and a compact binary the Rust reads at RUN time —
# never at compile time. See `assets/atlas.nix` for the sources and their
# licences, and `crates/ronoctua-atlas/src/baked.rs` for the layout.
{
  lib,
  stdenvNoCC,
  python3,
  fetchurl,
}:

let
  declared = import ../assets/atlas.nix;
  tag = "v5.1.2";
  ne =
    layer: hash:
    fetchurl {
      url = "https://raw.githubusercontent.com/nvkelso/natural-earth-vector/${tag}/geojson/${layer}.geojson";
      inherit hash;
    };

  # THREE OF THE SOURCES HAVE NO VERSION, and they are fetched from a SNAPSHOT.
  #
  # GeoNames and Geofabrik regenerate these files every day under the same
  # URL, so a hash pinned against the live address holds only until the store
  # copy is gone. It went on 2026-09-22: a garbage collection on the host
  # removed it, the next `nix run .#dev` downloaded the file again, and the
  # build died on `hash mismatch in fixed-output derivation` for data that had
  # nothing wrong with it. All three had moved on upstream, measured on the day.
  #
  # A Wayback Machine capture with `id_` answers the ORIGINAL BYTES, and a
  # capture does not change: the admin-1 and catalogue captures below hash to
  # exactly the values this file pinned against the live URLs before. The
  # archive rate-limits (429) when asked too often, which matters only on a
  # machine that has never built this, once — and the source root in
  # `nix/kept-sources.nix` is what keeps it from being asked twice.
  #
  # Refreshing the data is choosing a newer capture and its hash. Every
  # download in the tree has to name a version, a commit or a capture; the
  # `downloads-are-pinned` check refuses the rest.
  snapshot =
    stamp: url: hash:
    fetchurl {
      url = "https://web.archive.org/web/${stamp}id_/${url}";
      inherit hash;
    };

  places =
    snapshot "20260911002105" "https://download.geonames.org/export/dump/${declared.places}.zip"
      "sha256-xOxF8YkhlI/CvOWx6JvO+YwCNse+qYaRiZmz4/cb0+8=";
  admin1 =
    snapshot "20260911002039" "http://download.geonames.org/export/dump/admin1CodesASCII.txt"
      "sha256-WQZRSYBD9nSszaK39G0hKGzaDikLAvhWHFAF7umlRIw=";
  catalogue =
    snapshot "20260909165117" "https://download.geofabrik.de/index-v1.json"
      "sha256-m6kOXFJ35VOlOHEf2wGXyhWKtwxHSiRUBTPktx2McIA=";

  countries = ne "ne_${declared.scale}_admin_0_countries" "sha256-PkWPwDatCmZBHywebKxJxde/uByxEjvFE7IlEaK3/es=";
  regions = ne "ne_${declared.regionScale}_admin_1_states_provinces" "sha256-ItDjrYXrPifxfKv4ui1Q5VT7wnqHeW/4kdlYGF2mL7U=";
  lakes = ne "ne_${declared.scale}_lakes" "sha256-01C3WXiyb+g5t5fCxSmy+49H+zmDwD9JZONtXfk3ilI=";
in
stdenvNoCC.mkDerivation {
  name = "ronoctua-atlas-data";
  nativeBuildInputs = [ python3 ];
  dontUnpack = true;
  dontConfigure = true;

  buildPhase = ''
    runHook preBuild
    mkdir -p $out
    python3 ${./scripts}/gen-atlas.py \
      --countries ${countries} \
      --regions ${regions} \
      --lakes ${lakes} \
      --places ${places} \
      --admin1 ${admin1} \
      --catalogue ${catalogue} \
      --credits ${lib.escapeShellArg declared.credits} \
      --out $out/world.atlas
    runHook postBuild
  '';

  dontInstall = true;

  # What was downloaded to make this, for `nix/kept-sources.nix` to root.
  passthru.sources = [
    countries
    regions
    lakes
    places
    admin1
    catalogue
  ];

  meta = {
    description = "Borders, inland water, named places and the street-pack catalogue, in one table";
    platforms = lib.platforms.all;
  };
}
