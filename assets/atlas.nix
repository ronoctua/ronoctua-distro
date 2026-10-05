# WHERE THE WORLD DATA COMES FROM, and what it costs.
#
# # Nothing personal is in here (invariant 13)
#
# It is the world: borders, inland water, the names of places, and a catalogue
# of downloadable regions. The owner's own coordinates never reach this file —
# they are in the photographs on his disk, and they stay there.
#
# # The three sources, and their licences
#
# NATURAL EARTH is public domain, and its own terms say so in as many words.
# GEONAMES is **CC BY 4.0**, which REQUIRES attribution; the `credits` string
# below travels inside the baked table, so a copy of the file can never be
# separated from its licence, and it is also in `LICENSES/LICENSE-DATA.txt`,
# in the header of this file and on the screen. GEOFABRIK's catalogue and the
# packs it points at are **ODbL**, which becomes true on a machine that
# installs one.
#
# This repository is MIT + OFL (invariant 2), so none of those obligations may
# be silent — `atlas-credits-are-carried` reads all four places.
#
# # Why GeoJSON and not the shapefiles
#
# So that this tree has exactly ONE shapefile reader, in Rust, existing only for
# the street packs. The generator reads JSON and TSV and parses no binary
# geometry at all; `atlas-reads-one-shapefile` refuses a second one.
{
  # 1:50 m for the outlines. At 1:110 m a country is a dozen vertices and reads
  # as a cartoon; at 1:10 m the countries alone are 8.4 MB of `.shp` and the
  # extra detail is under the 611 m quantisation anyway.
  scale = "50m";

  # BUT 1:10 m FOR THE REGIONS, and it is coverage and not detail.
  #
  # Natural Earth's 1:50 m admin-1 layer is a SUBSET: 294 features, which are
  # the provinces of the handful of large federal countries. Measured — London
  # came back with a country and no region, because the United Kingdom is not
  # in it at all. The 1:10 m layer has 4 596, of which 232 are British.
  #
  # It costs about 1.3 million vertices before quantisation; how much survives
  # the 611 m grid is measured at bake time and printed, because a size written
  # in a comment is a size that goes stale.
  regionScale = "10m";

  # `cities500` and not `cities1000`: the thousand-person cut has no village
  # below a thousand people, which in the interior of Brazil or of Portugal is
  # exactly where a photograph gets taken — and the panel would then name a town
  # forty kilometres away as if it were the place. It costs about 2 MB baked.
  places = "cities500";

  # WHAT THE SCREEN AND THE FILE BOTH SAY. One string, so the two cannot drift.
  credits = "places from GeoNames (CC BY 4.0) · borders from Natural Earth (public domain) · streets from OpenStreetMap contributors (ODbL)";
}
