#!/usr/bin/env python3
"""Debug script to check metatile and tile rendering"""

from PIL import Image
import struct

# Check metatile 468 from LittlerootTown
with open('pokeemerald/data/tilesets/secondary/petalburg/metatiles.bin', 'rb') as f:
    data = f.read()

offset = 468 * 4
print(f"=== Metatile 468 Analysis ===")
print(f"Offset: 0x{offset:X}")

br_val = struct.unpack('<H', data[offset:offset+2])[0]
bl_val = struct.unpack('<H', data[offset+2:offset+4])[0]
print(f"BR value: 0x{br_val:04X}")
print(f"BL value: 0x{bl_val:04X}")

# Decode tile references
def decode_val(val):
    tile_idx = val & 0x03FF
    palette = (val >> 10) & 0x03
    hflip = (val >> 12) & 1
    vflip = (val >> 13) & 1
    return tile_idx, palette, hflip, vflip

tl_idx, tl_pal, tl_hflip, tl_vflip = decode_val(br_val)
tr_idx, tr_pal, tr_hflip, tr_vflip = decode_val(bl_val)

print(f"\nTile 0 (TL, pos 0,0):")
print(f"  tile_idx: {tl_idx}, palette: {tl_pal}, hflip: {tl_hflip}, vflip: {tl_vflip}")

print(f"\nTile 1 (TR, pos 16,0):")
print(f"  tile_idx: {tr_idx}, palette: {tr_pal}, hflip: {tr_hflip}, vflip: {tr_vflip}")

# Load tiles PNG
tiles_img = Image.open('pokeemerald/data/tilesets/secondary/petalburg/tiles.png')
tiles_across = 128 // 8
tiles_down = 80 // 8
print(f"\n=== Tileset Info ===")
print(f"PNG size: {tiles_img.size[0]}x{tiles_img.size[1]}")
print(f"Tiles: {tiles_across}x{tiles_down} = {tiles_across * tiles_down} tiles")

# Check where tile indices 2 and 2 would be
if tl_idx < tiles_across * tiles_down:
    row = tl_idx // tiles_across
    col = tl_idx % tiles_across
    tile_x = col * 8
    tile_y = row * 8
    print(f"\n=== Tile {tl_idx} Position ===")
    print(f"Row: {row}, Col: {col}")
    print(f"Pixel pos: ({tile_x},{tile_y}) to ({tile_x+8},{tile_y+8})")

    # Extract and show tile
    tile = tiles_img.crop((tile_x, tile_y, tile_x + 8, tile_y + 8))
    print(f"\nTile pixel indices (first row):")
    for x in range(8):
        pixel_idx = tile.getpixel((x, 0))
        print(f"  ({x},0): {pixel_idx}")

# Load palette 8
with open('pokeemerald/data/tilesets/secondary/petalburg/palettes/08.pal', 'r') as f:
    lines = f.readlines()

palette = []
for line in lines[3:19]:
    r, g, b = map(int, line.split())
    palette.append((r, g, b))

print("\n=== Palette 8 ===")
for i, color in enumerate(palette[:10]):
    print(f"  {i:2d}: {color}")
