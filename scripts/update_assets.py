#!/usr/bin/env python3
"""Refresh assets/Maps and assets/MapIcons from a local clapfoot/warapi clone.

    scripts/update_assets.py --warapi ~/src/warapi
    scripts/update_assets.py --warapi ~/src/warapi --dry-run
    scripts/update_assets.py --audit          # no source needed

Why a script instead of `cp`: upstream and this repo disagree about names, in
three ways that have each already cost a bug or a confusing hour.

  * Case. warapi ships `MapDeadlandsHex.TGA`; `regions.rs` says `DeadLandsHex`.
    A plain copy onto a case-insensitive filesystem leaves the old name on disk
    with the new bytes inside it, git reports clean, and the region stops
    rendering. Every map here is written out under the table's spelling.
  * Layout. Icons are `MapIconTownBaseTier1Colonial.TGA` upstream and
    `56Colonials.png` here, because the renderer addresses them by the API's
    numeric `iconType` and team, which is the only name the API ever gives it.
  * Format. Icons are TGA upstream, PNG here.
  * Faction art. Upstream mostly ships one *neutral* icon per structure and the
    game tints it, so `MapIconStorageFacilityColonial.TGA` does not exist to be
    copied. The renderer asks for `33Colonials.png` by name and falls back to
    the debug icon, so those are generated here -- see tint_linear_burn.

Nothing is guessed silently. An icon this script cannot confidently match is
reported and skipped, never approximated -- the whole point is that a bad name
fails loudly at your terminal instead of quietly at render time.

Standard library only, so it runs anywhere the repo is checked out.
"""

from __future__ import annotations

import argparse
import re
import struct
import sys
import zlib
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
MAPS_DIR = REPO / "assets" / "Maps"
ICONS_DIR = REPO / "assets" / "MapIcons"
REGIONS_RS = REPO / "src" / "utils" / "regions.rs"

# Art that lives in assets/Maps without being a world-conquest hex. Not part of
# the grid, not in the region table, and not an error.
NON_CONQUEST_MAPS = {"BGOneWorldMap.TGA", "MapHomeRegionC.TGA", "MapHomeRegionW.TGA"}

# The API's `iconType` -> the names upstream might file that icon under.
#
# Several candidates per id on purpose: upstream's filenames are descriptive and
# predate the enum, so they only mostly agree with it (61 is "Coal Field" in the
# docs and `MapIconCoal.TGA` on disk). The script tries each in order and tells
# you when none of them hit, which is the signal that Foxhole renamed something.
#
# ids and doc names: https://github.com/clapfoot/warapi (MapIconType)
ICON_SOURCES: dict[int, tuple[str, ...]] = {
    5: ("StaticBase1",),
    6: ("StaticBase2",),
    7: ("StaticBase3",),
    8: ("ForwardBase1",),
    9: ("ForwardBase2",),
    10: ("ForwardBase3",),
    11: ("Hospital",),
    12: ("VehicleFactory",),
    13: ("Armory",),
    14: ("SupplyStation",),
    15: ("Workshop",),
    16: ("ManufacturingPlant",),
    17: ("Refinery",),
    18: ("Shipyard",),
    19: ("TechCenter", "EngineeringCenter"),
    20: ("SalvageField", "Salvage"),
    21: ("ComponentField", "Components", "Component"),
    22: ("FuelField", "Fuel"),
    23: ("SulfurField", "Sulfur"),
    24: ("WorldMapTent", "MapTent"),
    25: ("TravelTent", "Tent"),
    26: ("TrainingArea",),
    27: ("Keep", "SpecialBase"),
    28: ("ObservationTower",),
    29: ("Fort",),
    30: ("TroopShip",),
    32: ("SulfurMine",),
    33: ("StorageFacility",),
    34: ("Factory",),
    35: ("GarrisonStation",),
    36: ("AmmoFactory",),
    37: ("RocketSite",),
    38: ("SalvageMine",),
    39: ("ConstructionYard",),
    40: ("ComponentMine",),
    41: ("OilWell",),
    45: ("RelicBase1", "RelicBase"),
    46: ("RelicBase2",),
    47: ("RelicBase3",),
    51: ("MassProductionFactory",),
    52: ("Seaport",),
    53: ("CoastalGun",),
    54: ("SoulFactory",),
    56: ("TownBaseTier1", "TownBase1"),
    57: ("TownBaseTier2", "TownBase2"),
    58: ("TownBaseTier3", "TownBase3"),
    59: ("StormCannon",),
    60: ("IntelCenter",),
    61: ("Coal", "CoalField"),
    62: ("Oil", "OilField"),
    70: ("RocketTarget",),
    71: ("RocketGroundZero",),
    72: ("RocketSiteWithRocket",),
    75: ("FacilityMineOilRig", "MineOilRig"),
    83: ("WeatherStation",),
    84: ("MortarHouse",),
    88: ("AircraftDepot",),
    89: ("AircraftFactory",),
    90: ("AircraftRadar",),
    91: ("AircraftRunwayT1",),
    92: ("AircraftRunwayT2",),
}

# The faction colours the neutral art is tinted with, as the colour pure white
# becomes. See tint_linear_burn below for how they were recovered.
TEAM_TINTS = {
    "Colonials": (101, 135, 94),
    "Wardens": (72, 125, 169),
}

# Icon types upstream simply doesn't ship art for. They are sourced by hand and
# live in assets/MapIcons already; the script never touches them.
#
# Listed so that "not found upstream" can mean two different things. Everything
# here is expected and reported as a one-liner. Anything *not* here that goes
# missing is a real signal -- Foxhole renamed a file -- and gets shouted about,
# which only works if the expected misses aren't drowning it out.
HAND_SOURCED = {
    11,  # Hospital
    12,  # Vehicle Factory
    13,  # Armory
    14,  # Supply Station
    16,  # Manufacturing Plant
    17,  # Refinery
    24,  # World Map Tent
    25,  # Travel Tent
    26,  # Training Area
    35,  # Garrison Station
    46,  # Relic Base 2 -- retired in Update 52
    47,  # Relic Base 3 -- retired in Update 52
    62,  # Oil Field
}

# Our filename suffix -> upstream's. Upstream is singular and drops the suffix
# entirely for neutral structures.
TEAMS = {"None": "", "Colonials": "Colonial", "Wardens": "Warden"}


# --- regions.rs, the single source of truth for map names -------------------


def read_region_names() -> list[str]:
    """The `api_name` of every region in the table, in table order.

    Parsed rather than duplicated: a copy here would be one more place to forget
    when a region is added, and this script exists to catch exactly that class
    of mistake.
    """
    source = REGIONS_RS.read_text(encoding="utf-8")
    names = re.findall(r'^\s*region\("([A-Za-z]+)"', source, re.MULTILINE)

    if not names:
        sys.exit(f"found no region(...) entries in {REGIONS_RS} -- has the table moved?")

    return names


# --- TGA in, PNG out --------------------------------------------------------


def decode_tga(data: bytes) -> tuple[int, int, bytearray]:
    """Minimal TGA reader: uncompressed (2) and RLE (10) truecolor, 24 or 32bpp.

    Returns RGBA rows top-down. Foxhole's art is bottom-left origin, which is
    the format's default and the reason for the flip below.
    """
    id_len, cmap_type, img_type = data[0], data[1], data[2]
    width, height, depth, descriptor = struct.unpack("<HHBB", data[12:18])

    if cmap_type != 0 or img_type not in (2, 10):
        raise ValueError(f"unsupported TGA type {img_type} (colormap {cmap_type})")
    if depth not in (24, 32):
        raise ValueError(f"unsupported TGA depth {depth}")

    stride = depth // 8
    pos = 18 + id_len
    pixels = bytearray()
    want = width * height * stride

    if img_type == 2:
        pixels = bytearray(data[pos : pos + want])
    else:
        while len(pixels) < want:
            packet = data[pos]
            pos += 1
            count = (packet & 0x7F) + 1

            if packet & 0x80:  # run: one pixel, repeated
                pixels += data[pos : pos + stride] * count
                pos += stride
            else:  # literal run
                pixels += data[pos : pos + count * stride]
                pos += count * stride

        del pixels[want:]

    if len(pixels) < want:
        raise ValueError("truncated TGA pixel data")

    # BGR(A) -> RGBA
    rgba = bytearray(width * height * 4)
    for i in range(width * height):
        b, g, r = pixels[i * stride], pixels[i * stride + 1], pixels[i * stride + 2]
        a = pixels[i * stride + 3] if stride == 4 else 255
        rgba[i * 4 : i * 4 + 4] = bytes((r, g, b, a))

    # Bit 5 of the descriptor set means the rows are already top-down.
    if not descriptor & 0x20:
        row = width * 4
        rgba = bytearray(b"".join(
            bytes(rgba[y * row : (y + 1) * row]) for y in reversed(range(height))
        ))

    return width, height, rgba


def decode_png(data: bytes) -> tuple[int, int, bytearray]:
    """The inverse of encode_png, for the icons already on disk.

    Only what this script writes: 8-bit RGBA, no interlacing. It still has to
    undo all five scanline filters, because the neutral icons in the repo
    predate this script and were saved by an image editor.
    """
    pos, idat, width, height = 8, b"", 0, 0

    while pos < len(data):
        length = struct.unpack(">I", data[pos : pos + 4])[0]
        tag = data[pos + 4 : pos + 8]
        payload = data[pos + 8 : pos + 8 + length]

        if tag == b"IHDR":
            width, height, depth, colour, _, _, interlace = struct.unpack(">IIBBBBB", payload[:13])
            if (depth, colour, interlace) != (8, 6, 0):
                raise ValueError(f"unsupported PNG: depth {depth}, colour type {colour}")
        elif tag == b"IDAT":
            idat += payload

        pos += 12 + length

    raw = zlib.decompress(idat)
    stride = width * 4
    rgba = bytearray()
    prev = bytearray(stride)
    pos = 0

    for _ in range(height):
        method = raw[pos]
        line = bytearray(raw[pos + 1 : pos + 1 + stride])
        pos += 1 + stride

        for i in range(stride):
            left = line[i - 4] if i >= 4 else 0
            up = prev[i]
            up_left = prev[i - 4] if i >= 4 else 0

            if method == 1:
                line[i] = (line[i] + left) & 0xFF
            elif method == 2:
                line[i] = (line[i] + up) & 0xFF
            elif method == 3:
                line[i] = (line[i] + (left + up) // 2) & 0xFF
            elif method == 4:  # Paeth
                estimate = left + up - up_left
                da, db, dc = abs(estimate - left), abs(estimate - up), abs(estimate - up_left)
                nearest = left if (da <= db and da <= dc) else (up if db <= dc else up_left)
                line[i] = (line[i] + nearest) & 0xFF
            elif method != 0:
                raise ValueError(f"unknown PNG filter {method}")

        rgba += line
        prev = line

    return width, height, rgba


def tint_linear_burn(rgba: bytes, colour: tuple[int, int, int]) -> bytearray:
    """Recolour neutral icon art for a faction: `out = neutral + colour - 255`.

    Foxhole ships one neutral icon per structure and tints it in game, so the
    faction art the renderer wants (`13Colonials.png`) does not exist upstream
    to copy -- it has to be produced here.

    Linear burn rather than a blend or a multiply, and not by taste: the faction
    icons already in this repo, made by hand years ago, are *exactly* this
    function of their neutral sibling. Checked pixel for pixel: of the 66
    faction icons on disk, the 38 whose structure upstream ships neutral-only
    match at 100% of opaque pixels with zero tolerance. (The other 28 are the
    ones upstream does ship faction art for -- 5-9, 19, 28-30 -- plus a handful
    drawn from different source art, and none of them are touched.) The two
    colours above were recovered from that fit: they are what pure white maps to.

    The black outline survives for free, which is the property that matters.
    Black is 0, the subtraction clamps at 0, so every outline pixel maps to
    itself; a plain blend would wash it grey and cost the icon its edge against
    a dark hex. Alpha is never touched, so the soft edge stays soft.
    """
    out = bytearray(rgba)

    for i in range(0, len(out), 4):
        for channel in range(3):
            out[i + channel] = max(0, min(255, out[i + channel] + colour[channel] - 255))

    return out


def encode_png(width: int, height: int, rgba: bytes) -> bytes:
    """8-bit RGBA PNG, no filtering. Icons are small; the size difference is not
    worth a filter heuristic."""
    row = width * 4
    raw = b"".join(b"\x00" + rgba[y * row : (y + 1) * row] for y in range(height))

    def chunk(tag: bytes, payload: bytes) -> bytes:
        body = tag + payload
        return struct.pack(">I", len(payload)) + body + struct.pack(">I", zlib.crc32(body))

    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(raw, 9))
        + chunk(b"IEND", b"")
    )


# --- copying ----------------------------------------------------------------


def index_dir(path: Path) -> dict[str, Path]:
    """Lowercased filename -> real path, so lookups survive upstream's mixed
    `.TGA`/`.tga` and any case drift in the stem."""
    return {entry.name.lower(): entry for entry in path.iterdir() if entry.is_file()}


def update_maps(source: Path, regions: list[str], dry_run: bool) -> int:
    index = index_dir(source)
    written = renamed = missing = 0

    for name in regions:
        target_name = f"Map{name}.TGA"
        found = index.get(target_name.lower())

        if found is None:
            print(f"  MISSING  {target_name} -- not in {source}")
            missing += 1
            continue

        if found.name != target_name:
            # The DeadLands case exactly. Worth saying out loud every time.
            print(f"  rename   {found.name} -> {target_name}")
            renamed += 1

        data = found.read_bytes()
        target = MAPS_DIR / target_name

        if target.exists() and target.read_bytes() == data:
            continue

        if not dry_run:
            # Drop any differently-cased copy first, or a case-insensitive
            # filesystem keeps the stale name and this whole exercise is moot.
            for existing in MAPS_DIR.iterdir():
                if existing.name.lower() == target_name.lower() and existing.name != target_name:
                    existing.unlink()
            target.write_bytes(data)

        print(f"  update   {target_name}")
        written += 1

    print(f"\nmaps: {written} updated, {renamed} name-corrected, {missing} missing")
    return missing


def update_icons(source: Path, dry_run: bool) -> int:
    index = index_dir(source)
    written = unmatched = 0
    unmatched_ids: list[int] = []

    for icon_type, candidates in sorted(ICON_SOURCES.items()):
        hits = 0

        for suffix, upstream_suffix in TEAMS.items():
            found = None
            for candidate in candidates:
                for ext in ("tga", "TGA"):
                    found = index.get(f"mapicon{candidate}{upstream_suffix}.{ext}".lower())
                    if found:
                        break
                if found:
                    break

            if found is None:
                continue

            try:
                width, height, rgba = decode_tga(found.read_bytes())
            except ValueError as err:
                print(f"  SKIP     {found.name}: {err}")
                continue

            png = encode_png(width, height, bytes(rgba))
            target = ICONS_DIR / f"{icon_type}{suffix}.png"
            hits += 1

            if target.exists() and target.read_bytes() == png:
                continue

            if not dry_run:
                target.write_bytes(png)

            print(f"  update   {target.name}  <- {found.name} ({width}x{height})")
            written += 1

        if hits == 0:
            unmatched += 1
            unmatched_ids.append(icon_type)

    by_hand = [i for i in unmatched_ids if i in HAND_SOURCED]
    unexpected = [i for i in unmatched_ids if i not in HAND_SOURCED]

    print(f"\nicons: {written} updated, {unmatched} icon types not found upstream")

    if by_hand:
        print(f"  hand-sourced, as expected ({len(by_hand)}): " + ", ".join(map(str, by_hand)))

    if unexpected:
        print(
            "\n  UNEXPECTED -- upstream has no file for: "
            + ", ".join(map(str, unexpected))
            + "\n  These are not on the hand-sourced list, so either Foxhole renamed the art or"
            "\n  it is newly retired. If renamed, add the new name to ICON_SOURCES; if you have"
            "\n  been supplying it by hand all along, add the id to HAND_SOURCED."
        )

    # Neither case fails the run: a retired icon is normal, and the icons on disk
    # are still fine. The audit's exit code is about maps, which do break renders.
    return 0


def derive_team_icons(dry_run: bool) -> int:
    """Write the faction icons that upstream has no art for, from the neutral.

    Runs over what is on disk rather than over the upstream clone on purpose:
    the icon types that need this most are the hand-sourced ones (13, 17, 24,
    25, 26, ...), which have no upstream file to derive from at all. Anything
    with a neutral icon in assets/MapIcons gets its two faction siblings,
    whether that neutral came from warapi a moment ago or from a human in 2022.

    Every icon type, not a curated list of the capturable ones. A resource field
    is never faction-held and its derived pair is dead weight -- two kilobytes,
    against a list that goes stale the first time Foxhole lets you claim
    something new, and a stale list here means the debug icon on a live map.

    Existing art always wins. Nothing hand-made is ever overwritten by a
    generated approximation of it.
    """
    written = kept = 0

    neutrals = sorted(
        (int(match.group(1)), entry)
        for entry in ICONS_DIR.iterdir()
        if entry.is_file() and (match := re.fullmatch(r"(\d+)None\.png", entry.name))
    )

    for icon_type, source in neutrals:
        decoded = None

        for suffix, colour in TEAM_TINTS.items():
            target = ICONS_DIR / f"{icon_type}{suffix}.png"

            if target.exists():
                kept += 1
                continue

            if decoded is None:
                decoded = decode_png(source.read_bytes())

            width, height, rgba = decoded
            png = encode_png(width, height, bytes(tint_linear_burn(rgba, colour)))

            if not dry_run:
                target.write_bytes(png)

            print(f"  derive   {target.name}  <- {source.name} (linear burn {colour})")
            written += 1

    print(f"\nderived: {written} faction icons written, {kept} left alone (art already on disk)")
    return written


# --- audit ------------------------------------------------------------------


def audit(regions: list[str]) -> int:
    """Cross-check assets/Maps against the region table.

    The check that would have caught both of this repo's asset bugs: a region
    whose art is missing or differently cased renders as a hole in the full map,
    and a leftover file from an older art drop renders as year-old terrain.
    """
    print("\naudit: assets/Maps vs the regions.rs table")

    on_disk = {entry.name for entry in MAPS_DIR.iterdir() if entry.is_file()}
    lowered = {name.lower(): name for name in on_disk}
    problems = 0

    for name in regions:
        expected = f"Map{name}.TGA"

        if expected in on_disk:
            continue

        actual = lowered.get(expected.lower())
        if actual:
            print(f"  CASE     {actual} on disk, table wants {expected}")
        else:
            print(f"  MISSING  {expected} -- {name} will render as a hole")
        problems += 1

    expected_all = {f"Map{name}.TGA" for name in regions} | NON_CONQUEST_MAPS
    for name in sorted(on_disk - expected_all):
        print(f"  EXTRA    {name} -- not in the table; stale art from an older drop?")
        problems += 1

    if problems == 0:
        print(f"  OK -- {len(regions)} regions, one file each, names exact")

    return problems


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--warapi", type=Path, help="path to a local clapfoot/warapi clone")
    parser.add_argument("--maps", action="store_true", help="update maps only")
    parser.add_argument("--icons", action="store_true", help="update icons only")
    parser.add_argument("--audit", action="store_true", help="audit only, copy nothing")
    parser.add_argument(
        "--derive-icons",
        action="store_true",
        help="tint neutral icons into their faction pair; no source needed",
    )
    parser.add_argument("--dry-run", action="store_true", help="report without writing")
    args = parser.parse_args()

    regions = read_region_names()
    print(f"{len(regions)} regions in {REGIONS_RS.relative_to(REPO)}")

    if args.audit:
        return 1 if audit(regions) else 0

    if args.derive_icons:
        if args.dry_run:
            print("(dry run -- nothing will be written)")
        print("\nicons:")
        derive_team_icons(args.dry_run)
        return 0

    if not args.warapi:
        parser.error("--warapi is required unless --audit or --derive-icons is given")

    images = args.warapi / "Images"
    if not images.is_dir():
        sys.exit(f"{images} not found -- is {args.warapi} a warapi clone?")

    both = not (args.maps or args.icons)
    if args.dry_run:
        print("(dry run -- nothing will be written)")

    if args.maps or both:
        print("\nmaps:")
        update_maps(images / "Maps", regions, args.dry_run)

    if args.icons or both:
        print("\nicons:")
        update_icons(images / "MapIcons", args.dry_run)
        derive_team_icons(args.dry_run)

    # Always audit afterwards: the point is to find out now, not at render time.
    return 1 if audit(regions) else 0


if __name__ == "__main__":
    sys.exit(main())
