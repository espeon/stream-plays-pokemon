#!/usr/bin/env python3
"""
Generate browser-source/src/emerald-map-data.ts from pokeemerald source.

Usage: python mapextract/extract_map_names.py
Requires: pokeemerald/ cloned in the project root (see justfile `setup` recipe).
"""

import json
import sys
from pathlib import Path

ROOT = Path(__file__).parent.parent
BASE = ROOT / "pokeemerald" / "data" / "maps"
SECTIONS_FILE = ROOT / "pokeemerald" / "src" / "data" / "region_map" / "region_map_sections.json"
OUT = ROOT / "browser-source" / "src" / "emerald-map-data.ts"

MAPSEC_TABLE = """\
  0:  { name: "Littleroot Town", tx: 4,  ty: 11, tw: 1, th: 1 },
  1:  { name: "Oldale Town",     tx: 4,  ty: 9,  tw: 1, th: 1 },
  2:  { name: "Dewford Town",    tx: 2,  ty: 14, tw: 1, th: 1 },
  3:  { name: "Lavaridge Town",  tx: 5,  ty: 3,  tw: 1, th: 1 },
  4:  { name: "Fallarbor Town",  tx: 3,  ty: 0,  tw: 1, th: 1 },
  5:  { name: "Verdanturf Town", tx: 4,  ty: 6,  tw: 1, th: 1 },
  6:  { name: "Pacifidlog Town", tx: 17, ty: 10, tw: 1, th: 1 },
  7:  { name: "Petalburg City",  tx: 1,  ty: 9,  tw: 1, th: 1 },
  8:  { name: "Slateport City",  tx: 8,  ty: 10, tw: 1, th: 2 },
  9:  { name: "Mauville City",   tx: 8,  ty: 6,  tw: 2, th: 1 },
  10: { name: "Rustboro City",   tx: 0,  ty: 5,  tw: 1, th: 2 },
  11: { name: "Fortree City",    tx: 12, ty: 0,  tw: 1, th: 1 },
  12: { name: "Lilycove City",   tx: 18, ty: 3,  tw: 2, th: 1 },
  13: { name: "Mossdeep City",   tx: 24, ty: 5,  tw: 2, th: 1 },
  14: { name: "Sootopolis City", tx: 21, ty: 7,  tw: 1, th: 1 },
  15: { name: "Ever Grande City",tx: 27, ty: 8,  tw: 1, th: 2 },
  16: { name: "Route 101",       tx: 4,  ty: 10, tw: 1, th: 1 },
  17: { name: "Route 102",       tx: 2,  ty: 9,  tw: 2, th: 1 },
  18: { name: "Route 103",       tx: 4,  ty: 8,  tw: 4, th: 1 },
  19: { name: "Route 104",       tx: 0,  ty: 7,  tw: 1, th: 3 },
  20: { name: "Route 105",       tx: 0,  ty: 10, tw: 1, th: 3 },
  21: { name: "Route 106",       tx: 0,  ty: 13, tw: 2, th: 1 },
  22: { name: "Route 107",       tx: 3,  ty: 14, tw: 3, th: 1 },
  23: { name: "Route 108",       tx: 6,  ty: 14, tw: 2, th: 1 },
  24: { name: "Route 109",       tx: 8,  ty: 12, tw: 1, th: 3 },
  25: { name: "Route 110",       tx: 8,  ty: 7,  tw: 1, th: 3 },
  26: { name: "Route 111",       tx: 8,  ty: 0,  tw: 1, th: 6 },
  27: { name: "Route 112",       tx: 6,  ty: 3,  tw: 2, th: 1 },
  28: { name: "Route 113",       tx: 4,  ty: 0,  tw: 4, th: 1 },
  29: { name: "Route 114",       tx: 1,  ty: 0,  tw: 2, th: 3 },
  30: { name: "Route 115",       tx: 0,  ty: 2,  tw: 1, th: 3 },
  31: { name: "Route 116",       tx: 1,  ty: 5,  tw: 4, th: 1 },
  32: { name: "Route 117",       tx: 5,  ty: 6,  tw: 3, th: 1 },
  33: { name: "Route 118",       tx: 10, ty: 6,  tw: 2, th: 1 },
  34: { name: "Route 119",       tx: 11, ty: 0,  tw: 1, th: 6 },
  35: { name: "Route 120",       tx: 13, ty: 0,  tw: 1, th: 4 },
  36: { name: "Route 121",       tx: 14, ty: 3,  tw: 4, th: 1 },
  37: { name: "Route 122",       tx: 16, ty: 4,  tw: 1, th: 2 },
  38: { name: "Route 123",       tx: 12, ty: 6,  tw: 5, th: 1 },
  39: { name: "Route 124",       tx: 20, ty: 3,  tw: 4, th: 3 },
  40: { name: "Route 125",       tx: 24, ty: 3,  tw: 2, th: 2 },
  41: { name: "Route 126",       tx: 20, ty: 6,  tw: 3, th: 3 },
  42: { name: "Route 127",       tx: 23, ty: 6,  tw: 3, th: 3 },
  43: { name: "Route 128",       tx: 23, ty: 9,  tw: 4, th: 1 },
  44: { name: "Route 129",       tx: 24, ty: 10, tw: 2, th: 1 },
  45: { name: "Route 130",       tx: 21, ty: 10, tw: 3, th: 1 },
  46: { name: "Route 131",       tx: 18, ty: 10, tw: 3, th: 1 },
  47: { name: "Route 132",       tx: 15, ty: 10, tw: 2, th: 1 },
  48: { name: "Route 133",       tx: 12, ty: 10, tw: 3, th: 1 },
  49: { name: "Route 134",       tx: 9,  ty: 10, tw: 3, th: 1 },
  50: { name: "Underwater (Rt 124)",      tx: 20, ty: 3,  tw: 4, th: 3 },
  51: { name: "Underwater (Rt 126)",      tx: 20, ty: 6,  tw: 3, th: 3 },
  52: { name: "Underwater (Rt 127)",      tx: 23, ty: 6,  tw: 3, th: 3 },
  53: { name: "Underwater (Rt 128)",      tx: 23, ty: 9,  tw: 4, th: 1 },
  54: { name: "Underwater (Sootopolis)", tx: 21, ty: 7,  tw: 1, th: 1 },
  55: { name: "Granite Cave",    tx: 1,  ty: 13, tw: 1, th: 1 },
  56: { name: "Mt. Chimney",     tx: 6,  ty: 2,  tw: 1, th: 1 },
  57: { name: "Safari Zone",     tx: 16, ty: 2,  tw: 1, th: 1 },
  58: { name: "Battle Frontier", tx: 22, ty: 12, tw: 1, th: 1 },
  59: { name: "Petalburg Woods", tx: 0,  ty: 8,  tw: 1, th: 1 },
  60: { name: "Rusturf Tunnel",  tx: 2,  ty: 5,  tw: 1, th: 1 },
  61: { name: "Abandoned Ship",  tx: 6,  ty: 14, tw: 1, th: 1 },
  62: { name: "New Mauville",    tx: 8,  ty: 7,  tw: 1, th: 1 },
  63: { name: "Meteor Falls",    tx: 0,  ty: 3,  tw: 1, th: 1 },
  65: { name: "Mt. Pyre",        tx: 16, ty: 4,  tw: 1, th: 1 },
  67: { name: "Shoal Cave",      tx: 24, ty: 4,  tw: 1, th: 1 },
  68: { name: "Seafloor Cavern", tx: 24, ty: 9,  tw: 1, th: 1 },
  70: { name: "Victory Road",    tx: 27, ty: 9,  tw: 1, th: 1 },
  72: { name: "Cave of Origin",  tx: 21, ty: 7,  tw: 1, th: 1 },
  73: { name: "Southern Island", tx: 12, ty: 14, tw: 1, th: 1 },
  78: { name: "Sealed Chamber",  tx: 11, ty: 10, tw: 1, th: 1 },
  80: { name: "Scorched Slab",   tx: 13, ty: 0,  tw: 1, th: 1 },
  82: { name: "Desert Ruins",    tx: 8,  ty: 3,  tw: 1, th: 1 },
  85: { name: "Sky Pillar",      tx: 19, ty: 10, tw: 1, th: 1 },
  197: { name: "Team Aqua Hideout", tx: 19, ty: 3, tw: 1, th: 1 },
  198: { name: "Team Magma Hideout", tx: 6, ty: 3, tw: 1, th: 1 },
  199: { name: "Mirage Tower",   tx: 8,  ty: 2,  tw: 1, th: 1 },
  202: { name: "Artisan Cave",   tx: 22, ty: 12, tw: 1, th: 1 },
  209: { name: "Desert Underpass", tx: 2, ty: 0, tw: 1, th: 1 },
  212: { name: "Trainer Hill",   tx: 8,  ty: 4,  tw: 1, th: 1 },"""

def main() -> None:
    if not SECTIONS_FILE.exists():
        print("error: pokeemerald not found. run `just setup` first.", file=sys.stderr)
        sys.exit(1)

    with open(SECTIONS_FILE) as f:
        sections_data = json.load(f)

    mapsec_id = {s["id"]: i for i, s in enumerate(sections_data["map_sections"])}

    with open(BASE / "map_groups.json") as f:
        groups = json.load(f)

    blocks = []
    total = 0
    for group_idx, group_name in enumerate(groups["group_order"]):
        group_maps = groups[group_name]
        group_label = group_name.removeprefix("gMapGroup_")
        entries = []
        for map_idx, map_dir in enumerate(group_maps):
            map_json_path = BASE / map_dir / "map.json"
            try:
                with open(map_json_path) as f:
                    m = json.load(f)
            except Exception:
                continue
            mapsec_name = m.get("region_map_section", "MAPSEC_DYNAMIC")
            idx = mapsec_id.get(mapsec_name)
            if idx is None:
                continue
            key = group_idx * 1000 + map_idx
            entries.append(f"  {key}: {idx},  // {map_dir}")
        if entries:
            blocks.append(f"  // Bank {group_idx}: {group_label}\n" + "\n".join(entries))
            total += len(entries)

    mapping_body = "\n\n".join(blocks)

    ts = f"""// Pokémon Emerald map data for the region map display.
// Sources:
//   - pokeemerald decomp: src/data/region_map/region_map_sections.json
//   - pokeemerald decomp: data/maps/map_groups.json
//
// Map image: 224×120 pixels (28×15 tiles at 8px/tile).
// The world map PNG from the decomp is at graphics/pokenav/region_map/map.png.
//
// MAPSEC tile coordinates are from region_map_sections.json.
// (bank, map_num) → MAPSEC is derived from map_groups.json + each map's region_map_section.
// Generated by mapextract/extract_map_names.py — do not edit by hand.

export const REGION_MAP_TILE_W = 28;
export const REGION_MAP_TILE_H = 15;
export const REGION_MAP_PX_W = 224;
export const REGION_MAP_PX_H = 120;

export interface MapsecEntry {{
  name: string;
  // Tile coordinates on the 28×15 world map grid (origin top-left)
  tx: number;
  ty: number;
  tw: number;
  th: number;
}}

// MAPSEC ID → tile coordinates and display name.
// IDs match the order in region_map_sections.constants.json.txt (0-indexed).
// Only entries with Hoenn world map coordinates are included.
export const MAPSEC: Record<number, MapsecEntry> = {{
{MAPSEC_TABLE}
}};

// (map_bank * 1000 + map_num) → MAPSEC ID for all Emerald maps.
// Generated from pokeemerald data/maps/map_groups.json + each map's region_map_section field.
// Maps with MAPSEC_DYNAMIC or MAPSEC_SECRET_BASE are omitted (lookupLocation returns null).
export const EMERALD_MAP_TO_MAPSEC: Record<number, number> = {{
{mapping_body}
}};

/** Look up the MAPSEC entry for a given (map_bank, map_num) pair. */
export function lookupLocation(bank: number, num: number): MapsecEntry | null {{
  const key = bank * 1000 + num;
  const mapsecId = EMERALD_MAP_TO_MAPSEC[key];
  if (mapsecId === undefined) return null;
  return MAPSEC[mapsecId] ?? null;
}}

/** Convert tile coordinates to pixel center within the world map image. */
export function tileCenterPx(tx: number, ty: number, tw: number, th: number): {{ x: number; y: number }} {{
  return {{
    x: (tx + tw / 2) * 8,
    y: (ty + th / 2) * 8,
  }};
}}
"""

    OUT.write_text(ts)
    print(f"wrote {OUT} ({total} map entries)")


if __name__ == "__main__":
    main()
