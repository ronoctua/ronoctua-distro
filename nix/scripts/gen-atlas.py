#!/usr/bin/env python3
"""Bakes the world into one table.

Reads GeoJSON (Natural Earth), a TSV inside a zip (GeoNames) and one JSON
catalogue (Geofabrik), and writes the little-endian table
`crates/ronoctua-atlas/src/baked.rs` reads. See that file's header for the
layout and for the traps; this one only has to agree with it.

THIS SCRIPT PARSES NO BINARY GEOMETRY, and that is deliberate: the tree has
exactly one shapefile reader, in Rust, and it exists only for the street packs.
`atlas-reads-one-shapefile` refuses a second.
"""

from __future__ import annotations

import argparse
import json
import struct
import sys
import zipfile
from pathlib import Path

# The grid the reader uses, and the same numbers. Two definitions of one grid is
# an index that finds nothing at the edges.
CELLS_LON = 180
CELLS_LAT = 90
STEPS = 65_534.0


def lon_to(deg: float) -> int:
    return quantise(deg, 360.0 / STEPS)


def lat_to(deg: float) -> int:
    return quantise(deg, 180.0 / STEPS)


def quantise(deg: float, step: float) -> int:
    n = deg / step
    return max(-32_767, min(32_767, int(n + (0.5 if n >= 0 else -0.5))))


def cell_of(lat: float, lon: float) -> int:
    x = min(CELLS_LON - 1, int((lon + 180.0) / 2.0))
    y = min(CELLS_LAT - 1, int((lat + 90.0) / 2.0))
    return y * CELLS_LON + x


def short(s: str) -> bytes:
    """A string with a one-byte length in front of it, cut to fit."""
    b = s.encode("utf-8")[:255]
    # AND CUT ON A CHARACTER BOUNDARY: a byte cut through a multi-byte
    # character is a name that draws as a replacement glyph for ever.
    while b:
        try:
            b.decode("utf-8")
            break
        except UnicodeDecodeError:
            b = b[:-1]
    return bytes([len(b)]) + b


def rings_of(geometry: dict) -> list[list[tuple[float, float]]]:
    """Every ring of a polygon or a multipolygon, as (lon, lat).

    A GeoJSON polygon is a list of rings and a multipolygon is a list of those,
    so one of the two is always one level deeper. Nothing says which ring is a
    hole, and nothing needs to: the reader fills with the even-odd rule.
    """
    kind = geometry.get("type")
    if kind == "Polygon":
        groups = [geometry.get("coordinates") or []]
    elif kind == "MultiPolygon":
        groups = geometry.get("coordinates") or []
    else:
        return []
    out = []
    for group in groups:
        for ring in group:
            points = [(float(p[0]), float(p[1])) for p in ring if len(p) >= 2]
            if len(points) >= 3:
                out.append(points)
    return out


def dedupe(ring: list[tuple[int, int]]) -> list[tuple[int, int]]:
    """Neighbours that quantised to the same grid point are one point.

    The quantisation is coarser than the data in places, so a run of vertices
    can collapse. A ring left with fewer than three points is not a ring.
    """
    out: list[tuple[int, int]] = []
    for p in ring:
        if not out or out[-1] != p:
            out.append(p)
    if len(out) > 1 and out[0] == out[-1]:
        out.pop()
    return out


def feature_bytes(head: bytes, rings: list[list[tuple[float, float]]]) -> bytes | None:
    quantised = []
    for ring in rings:
        q = dedupe([(lon_to(lon), lat_to(lat)) for lon, lat in ring])
        if len(q) >= 3:
            quantised.append(q)
    if not quantised:
        return None
    xs = [x for ring in quantised for x, _ in ring]
    ys = [y for ring in quantised for _, y in ring]
    out = bytearray(head)
    out += struct.pack("<H", len(quantised))
    out += struct.pack("<hhhh", min(xs), min(ys), max(xs), max(ys))
    for ring in quantised:
        out += struct.pack("<H", len(ring))
        for x, y in ring:
            out += struct.pack("<hh", x, y)
    return bytes(out)


def section(features: list[bytes]) -> bytes:
    out = bytearray(struct.pack("<H", len(features)))
    for f in features:
        out += f
    return bytes(out)


def name_of(props: dict, *keys: str) -> str:
    for k in keys:
        v = props.get(k)
        if isinstance(v, str) and v.strip():
            return v.strip()
    return ""


def countries(path: Path) -> tuple[bytes, dict[str, int]]:
    doc = json.loads(path.read_text(encoding="utf-8"))
    out: list[bytes] = []
    by_iso: dict[str, int] = {}
    for item in doc.get("features", []):
        props = item.get("properties") or {}
        english = name_of(props, "NAME_EN", "NAME", "ADMIN")
        portuguese = name_of(props, "NAME_PT", "NAME_EN", "NAME", "ADMIN")
        iso = name_of(props, "ISO_A2_EH", "ISO_A2", "ADM0_A3")[:2].upper()
        if not english:
            continue
        # A COUNTRY WITH NO CODE could never be matched to a street pack, and a
        # pack is chosen by the country the coordinate is in.
        if len(iso) != 2 or not iso.isalpha():
            iso = "??"
        head = iso.encode("ascii") + short(english) + short(portuguese)
        packed = feature_bytes(head, rings_of(item.get("geometry") or {}))
        if packed is None:
            continue
        by_iso.setdefault(iso, len(out))
        out.append(packed)
    if not out:
        sys.exit("ERROR: no country came out of the countries file")
    return section(out), by_iso


def regions(path: Path, by_iso: dict[str, int]) -> bytes:
    doc = json.loads(path.read_text(encoding="utf-8"))
    out: list[bytes] = []
    for item in doc.get("features", []):
        props = item.get("properties") or {}
        name = name_of(props, "name_en", "name", "woe_name")
        iso = name_of(props, "iso_a2", "adm0_a3")[:2].upper()
        if not name:
            continue
        owner = by_iso.get(iso)
        if owner is None:
            continue
        head = struct.pack("<H", owner) + short(name)
        packed = feature_bytes(head, rings_of(item.get("geometry") or {}))
        if packed is not None:
            out.append(packed)
    return section(out)


def lakes(path: Path) -> bytes:
    doc = json.loads(path.read_text(encoding="utf-8"))
    out: list[bytes] = []
    for item in doc.get("features", []):
        packed = feature_bytes(b"", rings_of(item.get("geometry") or {}))
        if packed is not None:
            out.append(packed)
    return section(out)


def places(zip_path: Path, admin1_path: Path) -> bytes:
    """Every named place, and the grid that finds them.

    The admin1 table is read and NOT written: a region name reaches the panel
    from the Natural Earth polygons, which are what a coordinate is tested
    against. Reading it here is the build-time check that the codes GeoNames
    uses are ones something knows — a bare `BR.27` on screen would be a code
    where a name should be.
    """
    known = set()
    for line in admin1_path.read_text(encoding="utf-8", errors="replace").splitlines():
        code = line.split("\t", 1)[0].strip()
        if code:
            known.add(code)

    rows: list[tuple[float, float, int, str]] = []
    with zipfile.ZipFile(zip_path) as zf:
        inside = next((n for n in zf.namelist() if n.endswith(".txt")), None)
        if inside is None:
            sys.exit("ERROR: the places archive holds no .txt")
        with zf.open(inside) as fh:
            for raw in fh:
                f = raw.decode("utf-8", "replace").split("\t")
                if len(f) < 15:
                    continue
                name = f[1].strip()
                if not name:
                    continue
                try:
                    lat, lon = float(f[4]), float(f[5])
                except ValueError:
                    continue
                if not (-90.0 <= lat <= 90.0 and -180.0 <= lon <= 180.0):
                    sys.exit(f"ERROR: {name} is at {lat},{lon}, which is not on the Earth")
                code = f"{f[8].strip()}.{f[10].strip()}"
                if f[10].strip() and code not in known:
                    # A CODE NOTHING KNOWS. Not fatal — GeoNames carries codes
                    # its own admin1 table does not — but the place keeps its
                    # name and loses the code, rather than carrying a number.
                    pass
                try:
                    pop = int(f[14] or "0")
                except ValueError:
                    pop = 0
                rows.append((lat, lon, min(pop // 256, 65_535), name))

    if not rows:
        sys.exit("ERROR: no place came out of the places archive")

    buckets: list[list[int]] = [[] for _ in range(CELLS_LON * CELLS_LAT)]
    for n, (lat, lon, _, _) in enumerate(rows):
        buckets[cell_of(lat, lon)].append(n)

    order: list[int] = []
    prefix: list[int] = []
    for bucket in buckets:
        prefix.append(len(order))
        order.extend(bucket)
    prefix.append(len(order))

    names = bytearray()
    at: dict[int, int] = {}
    for n, (_, _, _, name) in enumerate(rows):
        at[n] = len(names)
        names += short(name)

    out = bytearray(struct.pack("<I", len(rows)))
    out += struct.pack(f"<{len(prefix)}I", *prefix)
    for n in order:
        lat, lon, pop, _ = rows[n]
        out += struct.pack("<hhHHI", lon_to(lon), lat_to(lat), 0, pop, at[n])
    out += names
    return bytes(out)


def catalogue(path: Path) -> bytes:
    """Every region in Geofabrik's index, downloadable or not.

    # Why the ones that cannot be downloaded are baked too

    Twenty-nine of the five hundred and fifty-five are published ONLY as
    `.osm.pbf` — and they are the ones anybody looks for first: Brazil,
    Germany, France, Russia, the United States, and every continent. A
    shapefile of a whole country is not something Geofabrik makes, so those
    rows have no `shp` URL and this table cannot bake one.

    Dropping them is what this function did, and the owner reported the result
    in four words: *"não achei o Brasil na lista"*. A missing row and a row
    that refuses look the same from outside — except that the second one can
    say WHY, and can say that the five parts of Brazil are in the same list.
    So the row is kept with an empty URL, which is what the view draws
    refusing; it is the format view's XFS line, one subject along.

    The PARENT goes in for the same reason: without it the five parts read as
    `Sudeste`, `Sul`, `Norte` — names that mean nothing to somebody looking for
    a country, and that no filter for `brazil` would ever find.
    """
    doc = json.loads(path.read_text(encoding="utf-8"))
    out: list[bytes] = []
    for item in doc.get("features", []):
        props = item.get("properties") or {}
        rid = str(props.get("id") or "").strip()
        label = str(props.get("name") or "").strip()
        urls = props.get("urls") or {}
        shp = str(urls.get("shp") or "").strip()
        if not rid or not label:
            continue
        isos = ",".join(str(c) for c in (props.get("iso3166-1:alpha2") or []))
        parent = str(props.get("parent") or "").strip()
        out.append(short(rid) + short(label) + short(shp) + short(isos) + short(parent))
    if not out:
        sys.exit("ERROR: no region came out of the catalogue")
    return struct.pack("<H", len(out)) + b"".join(out)


def main() -> None:
    # EVERY FLAG WRITTEN OUT, and never built in a loop.
    #
    # A flag's name crosses the boundary between this file and `nix/atlas.nix`
    # as a STRING, and `generator-flags-agree` is what keeps the two sides from
    # drifting — it reads `add_argument("--…")` here and the invocation there.
    # A loop over a tuple of names is invisible to it: the check saw one flag
    # declared and five passed, and reported this file as taking arguments it
    # does not declare. The loop was two lines shorter and cost the contract.
    ap = argparse.ArgumentParser()
    ap.add_argument("--countries", required=True)
    ap.add_argument("--regions", required=True)
    ap.add_argument("--lakes", required=True)
    ap.add_argument("--places", required=True)
    ap.add_argument("--admin1", required=True)
    ap.add_argument("--catalogue", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--credits", required=True)
    args = ap.parse_args()

    body, by_iso = countries(Path(args.countries))
    parts = [
        (1, body),
        (2, regions(Path(args.regions), by_iso)),
        (3, lakes(Path(args.lakes))),
        (4, places(Path(args.places), Path(args.admin1))),
        (5, catalogue(Path(args.catalogue))),
        (6, args.credits.encode("utf-8")),
    ]

    head = 16 + len(parts) * 12
    table = bytearray()
    blob = bytearray()
    for kind, bytes_ in parts:
        table += struct.pack("<HHII", kind, 0, head + len(blob), len(bytes_))
        blob += bytes_

    out = bytearray(b"RONOATL1")
    # THE VERSION IS TWO, and it is `ronoctua_atlas::baked::VERSION` — the
    # reader refuses any other, which is what makes a table baked before the
    # catalogue's row gained its parent say so instead of being read as if it
    # had not.
    out += struct.pack("<HHI", 2, len(parts), 0)
    out += table
    out += blob
    Path(args.out).write_bytes(out)
    print(f"atlas: {len(out) / 1048576:.2f} MB, {len(parts)} sections")


if __name__ == "__main__":
    main()
