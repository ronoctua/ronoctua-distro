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
      --countries ${ne "ne_${declared.scale}_admin_0_countries" "sha256-PkWPwDatCmZBHywebKxJxde/uByxEjvFE7IlEaK3/es="} \
      --regions ${ne "ne_${declared.regionScale}_admin_1_states_provinces" "sha256-ItDjrYXrPifxfKv4ui1Q5VT7wnqHeW/4kdlYGF2mL7U="} \
      --lakes ${ne "ne_${declared.scale}_lakes" "sha256-01C3WXiyb+g5t5fCxSmy+49H+zmDwD9JZONtXfk3ilI="} \
      --places ${
        fetchurl {
          url = "https://download.geonames.org/export/dump/${declared.places}.zip";
          hash = "sha256-U8wchGL8pGPumZJJUt0Feiwx8+IQUZdwM3+GerQgTLU=";
        }
      } \
      --admin1 ${
        fetchurl {
          url = "https://download.geonames.org/export/dump/admin1CodesASCII.txt";
          hash = "sha256-WQZRSYBD9nSszaK39G0hKGzaDikLAvhWHFAF7umlRIw=";
        }
      } \
      --catalogue ${
        fetchurl {
          url = "https://download.geofabrik.de/index-v1.json";
          hash = "sha256-m6kOXFJ35VOlOHEf2wGXyhWKtwxHSiRUBTPktx2McIA=";
        }
      } \
      --credits ${lib.escapeShellArg declared.credits} \
      --out $out/world.atlas
    runHook postBuild
  '';

  dontInstall = true;

  meta = {
    description = "Borders, inland water, named places and the street-pack catalogue, in one table";
    platforms = lib.platforms.all;
  };
}
