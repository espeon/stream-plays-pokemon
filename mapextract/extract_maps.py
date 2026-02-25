#!/usr/bin/env python3
"""
Pokemon Emerald Map Extractor
Renders all Pokemon Emerald maps as PNG images from decomp data.

Usage: python extract_maps.py [output_dir]
"""

import json
import re
import struct
from pathlib import Path
from typing import Dict, List, Tuple, Optional
from dataclasses import dataclass
from PIL import Image
import numpy as np


# Movement types that indicate a stationary object — render its sprite.
# Excludes WANDER_*, WALK_* (non-in-place), JOG/RUN, COPY_PLAYER_*, INVISIBLE.
STANDER_MOVEMENT_TYPES: frozenset = frozenset({
    'MOVEMENT_TYPE_FACE_DOWN', 'MOVEMENT_TYPE_FACE_UP',
    'MOVEMENT_TYPE_FACE_LEFT', 'MOVEMENT_TYPE_FACE_RIGHT',
    'MOVEMENT_TYPE_LOOK_AROUND',
    'MOVEMENT_TYPE_FACE_DOWN_AND_LEFT', 'MOVEMENT_TYPE_FACE_DOWN_AND_RIGHT',
    'MOVEMENT_TYPE_FACE_UP_AND_LEFT', 'MOVEMENT_TYPE_FACE_UP_AND_RIGHT',
    'MOVEMENT_TYPE_FACE_LEFT_AND_RIGHT', 'MOVEMENT_TYPE_FACE_DOWN_AND_UP',
    'MOVEMENT_TYPE_FACE_DOWN_UP_AND_RIGHT', 'MOVEMENT_TYPE_FACE_UP_LEFT_AND_RIGHT',
    'MOVEMENT_TYPE_FACE_DOWN_LEFT_AND_RIGHT',
    'MOVEMENT_TYPE_ROTATE_CLOCKWISE', 'MOVEMENT_TYPE_ROTATE_COUNTERCLOCKWISE',
    'MOVEMENT_TYPE_WALK_IN_PLACE_DOWN', 'MOVEMENT_TYPE_WALK_IN_PLACE_UP',
    'MOVEMENT_TYPE_WALK_IN_PLACE_LEFT', 'MOVEMENT_TYPE_WALK_IN_PLACE_RIGHT',
    'MOVEMENT_TYPE_WALK_SLOWLY_IN_PLACE_LEFT', 'MOVEMENT_TYPE_WALK_SLOWLY_IN_PLACE_RIGHT',
    'MOVEMENT_TYPE_JOG_IN_PLACE_LEFT', 'MOVEMENT_TYPE_JOG_IN_PLACE_RIGHT',
    'MOVEMENT_TYPE_TREE_DISGUISE', 'MOVEMENT_TYPE_MOUNTAIN_DISGUISE',
    'MOVEMENT_TYPE_BERRY_TREE_GROWTH',
    'MOVEMENT_TYPE_BURIED',
})


def build_object_event_sprite_map(base_dir: Path) -> Dict[str, Tuple[Path, int, int]]:
    """Build mapping from OBJ_EVENT_GFX_{NAME} suffix → (png_path, width, height).

    Resolves the three-step chain:
      pointers.h:      OBJ_EVENT_GFX_{X}        → gObjectEventGraphicsInfo_{Y}
      graphics_info.h: gObjectEventGraphicsInfo_{Y} → (width, height)
      graphics.h:      gObjectEventPic_{Y}         → file path  (PicName ≈ InfoName)
    """
    obj_dir = base_dir / "src" / "data" / "object_events"

    # Step 1: GFX name → GraphicsInfo name
    gfx_to_info: Dict[str, str] = {}
    ptr_pattern = re.compile(r'\[OBJ_EVENT_GFX_(\w+)\]\s*=\s*&gObjectEventGraphicsInfo_(\w+)')
    pointers_h = obj_dir / "object_event_graphics_info_pointers.h"
    if pointers_h.exists():
        for m in ptr_pattern.finditer(pointers_h.read_text()):
            gfx_to_info[m.group(1)] = m.group(2)

    # Step 2: GraphicsInfo name → (width, height)
    info_dims: Dict[str, Tuple[int, int]] = {}
    info_pattern = re.compile(
        r'gObjectEventGraphicsInfo_(\w+)\s*=\s*\{[^}]*?\.width\s*=\s*(\d+)[^}]*?\.height\s*=\s*(\d+)',
        re.DOTALL
    )
    info_h = obj_dir / "object_event_graphics_info.h"
    if info_h.exists():
        for m in info_pattern.finditer(info_h.read_text()):
            info_dims[m.group(1)] = (int(m.group(2)), int(m.group(3)))

    # Step 3: Pic name → PNG path  (gObjectEventPic_{Name} → path)
    pic_paths: Dict[str, Path] = {}
    pic_pattern = re.compile(r'gObjectEventPic_(\w+)\s*\[\]\s*=\s*INCBIN_U\d+\("([^"]+)"\)')
    gfx_h = obj_dir / "object_event_graphics.h"
    if gfx_h.exists():
        for m in pic_pattern.finditer(gfx_h.read_text()):
            raw = m.group(2).replace(".4bpp.lz", ".png").replace(".4bpp", ".png")
            pic_paths[m.group(1)] = base_dir / raw

    # Chain everything together
    result: Dict[str, Tuple[Path, int, int]] = {}
    for gfx_name, info_name in gfx_to_info.items():
        dims = info_dims.get(info_name)
        if dims is None:
            continue
        w, h = dims
        # Try InfoName as the pic name directly; some sprites use a different pic
        # (e.g. BrendanNormal → gObjectEventPic_BrendanNormal)
        png = pic_paths.get(info_name)
        if png is None or not png.exists():
            continue
        result[gfx_name] = (png, w, h)
    return result


def render_sprite_frame(png_path: Path, width: int, height: int) -> Optional[Image.Image]:
    """Extract frame 0 from a sprite sheet as an RGBA image (color index 0 = transparent)."""
    try:
        sheet = Image.open(png_path)
    except Exception:
        return None

    frame = sheet.crop((0, 0, width, height))
    palette = sheet.getpalette()  # flat [R, G, B, R, G, B, ...]

    result = Image.new('RGBA', (width, height), (0, 0, 0, 0))
    for y in range(height):
        for x in range(width):
            idx = frame.getpixel((x, y))
            if idx == 0:
                continue
            r, g, b = palette[idx * 3], palette[idx * 3 + 1], palette[idx * 3 + 2]
            result.putpixel((x, y), (r, g, b, 255))
    return result


def composite_object_events(
    map_img: Image.Image,
    map_json_path: Path,
    sprite_map: Dict[str, Tuple[Path, int, int]],
) -> Image.Image:
    """Overlay stationary object events onto a rendered map image."""
    try:
        with open(map_json_path) as f:
            map_data = json.load(f)
    except Exception:
        return map_img

    for event in map_data.get('object_events', []):
        movement_type = event.get('movement_type', '')
        if movement_type == 'MOVEMENT_TYPE_INVISIBLE':
            continue
        if movement_type not in STANDER_MOVEMENT_TYPES:
            continue

        gfx_name = event.get('graphics_id', '').removeprefix('OBJ_EVENT_GFX_')
        entry = sprite_map.get(gfx_name)
        if entry is None:
            continue

        png_path, w, h = entry
        sprite = render_sprite_frame(png_path, w, h)
        if sprite is None:
            continue

        x, y = int(event['x']), int(event['y'])
        # Align sprite feet to the bottom of metatile (x, y)
        paste_x = x * 16
        paste_y = (y + 1) * 16 - h
        map_img.paste(sprite, (paste_x, paste_y), mask=sprite)

    return map_img


def build_tileset_path_map(base_dir: Path) -> Dict[str, Path]:
    """Build a map from gTileset_* names (as used in layouts.json) to tiles.png paths.

    Two-step resolution:
    1. Parse graphics.h + graphics.c to map gTilesetTiles_{TilesName} → tiles.png path.
    2. Parse headers.h to map gTileset_{TilesetName} → gTilesetTiles_{TilesName},
       so that names like gTileset_Building resolve to gTilesetTiles_InsideBuilding.

    Returns {TilesetName: Path(tiles.png)} keyed by the name after stripping gTileset_.
    """
    # Step 1: gTilesetTiles_{name} → tiles.png path
    tile_sources = [
        base_dir / "src" / "data" / "tilesets" / "graphics.h",
        base_dir / "src" / "graphics.c",
    ]
    tiles_pattern = re.compile(
        r'gTilesetTiles_(\w+)\s*\[\]\s*=\s*INCBIN_U\d+\("([^"]+)"\)'
    )
    tiles_map: Dict[str, Path] = {}
    for src in tile_sources:
        if not src.exists():
            continue
        for match in tiles_pattern.finditer(src.read_text()):
            tiles_name, raw_path = match.group(1), match.group(2)
            if tiles_name.endswith("Compressed") or "Unknown" in tiles_name or "unknown" in tiles_name:
                continue
            if "unused_tiles" in raw_path or "unknown_tiles" in raw_path:
                continue
            png_path = raw_path.replace(".4bpp.lz", ".png").replace(".4bpp", ".png")
            tiles_map[tiles_name] = base_dir / png_path

    # Step 2: gTileset_{name} → gTilesetTiles_{tilesName} from headers.h
    headers_h = base_dir / "src" / "data" / "tilesets" / "headers.h"
    header_pattern = re.compile(
        r'gTileset_(\w+)\s*=\s*\{[^}]*?\.tiles\s*=\s*gTilesetTiles_(\w+)',
        re.DOTALL
    )
    result: Dict[str, Path] = {}
    if headers_h.exists():
        for match in header_pattern.finditer(headers_h.read_text()):
            tileset_name, tiles_name = match.group(1), match.group(2)
            if tiles_name in tiles_map:
                result[tileset_name] = tiles_map[tiles_name]

    # Also include direct matches (gTileset_Foo → gTilesetTiles_Foo)
    for name, path in tiles_map.items():
        if name not in result:
            result[name] = path

    return result


@dataclass
class TileReference:
    """A reference to a tile within a metatile"""
    tile_index: int      # Index of 8x8 tile in tileset
    palette: int         # Palette index (0-15)
    hflip: bool         # Horizontal flip
    vflip: bool         # Vertical flip


@dataclass
class Metatile:
    """A 16x16 metatile composed of 8 tiles (2 layers of 2x2)"""
    bottom_layer: Tuple[TileReference, TileReference, TileReference, TileReference]
    top_layer: Tuple[TileReference, TileReference, TileReference, TileReference]


@dataclass
class Tileset:
    """A tileset containing tiles, palettes, and metatiles"""
    name: str
    tiles_png: Image.Image      # 4bpp tile graphics
    palettes: List[List[Tuple[int, int, int]]]  # 16 colors per palette
    metatiles: List[Metatile]  # Metatile definitions
    attributes: List[int]        # Metatile attributes (layer type, behavior)


@dataclass
class Layout:
    """A map layout with dimensions and tilesets"""
    id: str
    name: str
    width: int
    height: int
    primary_tileset: str
    secondary_tileset: str
    border_path: str
    blockdata_path: str


@dataclass
class Map:
    """A complete map with numeric ID, layout, and events"""
    id: int
    group_num: int
    map_num: int
    name: str
    layout_id: str
    layout: Optional[Layout]  # The layout object


def read_jasc_palette(filepath: Path) -> List[Tuple[int, int, int]]:
    """Parse JASC-PAL format palette file"""
    with open(filepath, 'r') as f:
        lines = [line.strip() for line in f.readlines()]

    if not lines[0].startswith('JASC-PAL'):
        raise ValueError(f"Invalid JASC-PAL header in {filepath}")

    num_colors = int(lines[2])
    colors = []

    for line in lines[3:3 + num_colors]:
        r, g, b = map(int, line.split())
        colors.append((r, g, b))

    return colors


def read_palette_set(tileset_dir: Path) -> List[List[Tuple[int, int, int]]]:
    """Read all 16 palettes from a tileset directory"""
    palettes_dir = tileset_dir / "palettes"

    if not palettes_dir.exists():
        # Try different case
        palettes_dir = tileset_dir / "palettes"

    # Find all .pal files (00.pal through 15.pal)
    # Need to ensure we get exactly 16 palettes
    palette_files = []
    for i in range(16):
        pal_file = palettes_dir / f"{i:02d}.pal"
        if pal_file.exists():
            palette_files.append(pal_file)
        else:
            # Try with leading zero removed if file doesn't exist
            pal_file = palettes_dir / f"{i:d}.pal"
            if pal_file.exists():
                palette_files.append(pal_file)

    # Sort by numeric index
    palette_files.sort(key=lambda p: int(p.stem))

    if len(palette_files) != 16:
        print(f"Warning: Expected 16 palettes, found {len(palette_files)} in {palettes_dir}")

    palette_set = []
    for pal_file in palette_files:
        colors = read_jasc_palette(pal_file)
        palette_set.append(colors)

    return palette_set


def decode_4bpp_tile(data: bytes, offset: int) -> np.ndarray:
    """Decode a single 8x8 4bpp tile to 8x8 pixel array"""
    # 4bpp = 4 bits per pixel = 2 pixels per byte
    # Each tile is 8x8 = 64 pixels = 32 bytes

    tile_data = data[offset:offset + 32]
    pixels = np.zeros((8, 8), dtype=np.uint8)

    for row in range(8):
        row_offset = row * 4
        for col in range(8):
            byte_idx = row_offset + (col // 2)
            if col % 2 == 0:
                # First pixel: high nibble
                pixel = (tile_data[byte_idx] >> 4) & 0x0F
            else:
                # Second pixel: low nibble
                pixel = tile_data[byte_idx] & 0x0F
            pixels[row, col] = pixel

    return pixels


def extract_tiles_from_png(png_path: Path) -> Tuple[Image.Image, np.ndarray]:
    """Extract 8x8 tiles from a tileset PNG"""
    img = Image.open(png_path)

    if img.mode != 'P':
        raise ValueError(f"Tileset PNG must be palette mode (P), got {img.mode}")

    # Convert to numpy array for easier processing
    img_array = np.array(img)

    return img, img_array


def decode_metatile_value(value: int) -> TileReference:
    """
    Decode a 16-bit metatile tile reference.

    From tools/gbagfx/gfx.h struct NonAffineTile:
    - Bits 0-9:   Tile index (10 bits)
    - Bit 10:     Horizontal flip
    - Bit 11:     Vertical flip
    - Bits 12-15: Palette index (4 bits, 0-15)
    """
    tile_index = value & 0x03FF       # Bits 0-9
    hflip      = (value >> 10) & 1    # Bit 10
    vflip      = (value >> 11) & 1    # Bit 11
    palette    = (value >> 12) & 0x0F # Bits 12-15

    return TileReference(tile_index=tile_index, palette=palette, hflip=bool(hflip), vflip=bool(vflip))


def read_metatiles(metatile_path: Path, debug: bool = False) -> List[Metatile]:
    """Parse metatiles.bin file.

    Each metatile is NUM_TILES_PER_METATILE (8) × 2 bytes = 16 bytes.
    Layout: 4 bottom-layer tiles then 4 top-layer tiles, each in TL/TR/BL/BR order.
    """
    with open(metatile_path, 'rb') as f:
        data = f.read()

    metatiles = []
    num_metatiles = len(data) // 16  # 8 tiles × 2 bytes each

    for i in range(num_metatiles):
        offset = i * 16
        tiles = [
            decode_metatile_value(struct.unpack('<H', data[offset + j*2:offset + j*2 + 2])[0])
            for j in range(8)
        ]

        # tiles[0-3]: bottom layer TL, TR, BL, BR
        # tiles[4-7]: top layer TL, TR, BL, BR
        bottom = (tiles[0], tiles[1], tiles[2], tiles[3])
        top    = (tiles[4], tiles[5], tiles[6], tiles[7])

        if debug and i < 5:
            print(f"    DEBUG: Metatile {i}:")
            for j, t in enumerate(tiles):
                layer = "bot" if j < 4 else "top"
                pos   = ["TL","TR","BL","BR"][j % 4]
                print(f"      [{layer} {pos}] tile={t.tile_index}, pal={t.palette}, hflip={t.hflip}, vflip={t.vflip}")

        metatiles.append(Metatile(bottom_layer=bottom, top_layer=top))

    return metatiles


def read_metatile_attributes(attr_path: Path) -> List[int]:
    """Parse metatile_attributes.bin file"""
    with open(attr_path, 'rb') as f:
        data = f.read()

    attributes = []
    num_attrs = len(data) // 2

    for i in range(num_attrs):
        attr = struct.unpack('<H', data[i*2:(i+1)*2])[0]
        attributes.append(attr)

    return attributes


def load_tileset(tileset_name: str, tileset_type: str, base_dir: Path,
                 path_map: Dict[str, Path], debug: bool = False) -> Tileset:
    """Load a complete tileset, resolving paths from the graphics.h path map."""
    name = tileset_name.removeprefix("gTileset_")

    tiles_png = path_map.get(name)
    if tiles_png is None:
        raise FileNotFoundError(f"Tileset not found in path map: {tileset_name}")
    if not tiles_png.exists():
        raise FileNotFoundError(f"tiles.png missing: {tiles_png}")

    tiles_dir = tiles_png.parent

    # metatiles.bin may be in tiles_dir or a parent (e.g. SecretBase* share secret_base/)
    metatiles_bin = tiles_dir / "metatiles.bin"
    if not metatiles_bin.exists():
        metatiles_bin = tiles_dir.parent / "metatiles.bin"
    if not metatiles_bin.exists():
        raise FileNotFoundError(f"metatiles.bin not found near {tiles_dir}")

    metatile_attrs_bin = metatiles_bin.parent / "metatile_attributes.bin"

    if debug:
        print(f"  DEBUG: tiles:     {tiles_png}")
        print(f"  DEBUG: metatiles: {metatiles_bin}")

    tiles_img, _ = extract_tiles_from_png(tiles_png)
    if debug:
        w, h = tiles_img.size
        print(f"  DEBUG: Tiles PNG: {w}x{h} = {(w//8)*(h//8)} tiles ({w//8}x{h//8})")

    palettes = read_palette_set(tiles_dir)
    if debug:
        print(f"  DEBUG: Loaded {len(palettes)} palettes")

    metatiles = read_metatiles(metatiles_bin, debug)
    if debug:
        print(f"  DEBUG: Loaded {len(metatiles)} metatiles")

    attributes = read_metatile_attributes(metatile_attrs_bin)

    return Tileset(
        name=name,
        tiles_png=tiles_img,
        palettes=palettes,
        metatiles=metatiles,
        attributes=attributes
    )



def load_layouts(layouts_json: Path) -> Dict[str, Layout]:
    """Load all layouts from layouts.json"""
    with open(layouts_json, 'r') as f:
        data = json.load(f)

    layouts = {}
    for layout_data in data['layouts']:
        layout = Layout(
            id=layout_data['id'],
            name=layout_data['name'],
            width=layout_data['width'],
            height=layout_data['height'],
            primary_tileset=layout_data['primary_tileset'],
            secondary_tileset=layout_data['secondary_tileset'],
            border_path=layout_data.get('border_filepath', ''),
            blockdata_path=layout_data['blockdata_filepath']
        )
        layouts[layout.id] = layout

    return layouts


def load_maps(map_groups_json: Path, base_dir: Path):
    """Load all maps from map_groups.json"""
    with open(map_groups_json, 'r') as f:
        data = json.load(f)

    maps = []
    map_layouts = {}

    # Iterate through groups in order
    for group_idx, group_name in enumerate(data['group_order']):
        group_maps = data[group_name]

        for map_idx, map_name in enumerate(group_maps):
            # Load the map.json file
            map_dir = base_dir / "data" / "maps" / map_name
            map_json_path = map_dir / "map.json"

            with open(map_json_path, 'r') as f:
                map_data = json.load(f)

            # Numeric ID: (map_num | (group_num << 8))
            map_id = map_idx | (group_idx << 8)

            map_obj = Map(
                id=map_id,
                group_num=group_idx,
                map_num=map_idx,
                name=map_data['name'],
                layout_id=map_data['layout'],
                layout=None  # Will be filled in later
            )
            maps.append(map_obj)
            map_layouts[map_data['layout']] = map_obj

    return maps, map_layouts


def read_map_grid(blockdata_path: Path, width: int, height: int) -> np.ndarray:
    """Read map.bin file and decode to metatile ID grid"""
    with open(blockdata_path, 'rb') as f:
        data = f.read()

    grid = np.zeros((height, width), dtype=np.uint16)

    expected_size = width * height * 2
    if len(data) < expected_size:
        print(f"Warning: map.bin size {len(data)} < expected {expected_size}, padding with empty metatiles")
        data = data + b'\x00' * (expected_size - len(data))
    elif len(data) > expected_size:
        print(f"Warning: map.bin size {len(data)} > expected {expected_size}, truncating")

    for y in range(height):
        for x in range(width):
            offset = (y * width + x) * 2
            if offset + 2 > len(data):
                metatile_id = 0  # Empty metatile
            else:
                value = struct.unpack('<H', data[offset:offset+2])[0]

                # Decode per global.fieldmap.h:
                # Bits 0-9: Metatile ID
                # Bits 10-11: Collision
                # Bits 12-15: Elevation
                metatile_id = value & 0x03FF

            grid[y, x] = metatile_id

    return grid


def render_tile(tile_ref: TileReference, tiles_png: Image.Image, palettes: List[List[Tuple[int, int, int]]]) -> Image.Image:
    """Render a single 8x8 tile with palette and flips applied"""
    # Get tile from tileset PNG
    # The PNG is a strip of 8x8 tiles
    # For primary: 128x256 = 16 tiles across, 32 tiles down = 512 tiles
    # For secondary: 128x80 = 16 tiles across, 10 tiles down = 160 tiles (varies)

    img_width, img_height = tiles_png.size
    tiles_across = img_width // 8
    tiles_down = img_height // 8

    tile_idx = tile_ref.tile_index
    palette_idx = tile_ref.palette

    # Calculate tile position in PNG
    tile_row = tile_idx // tiles_across
    tile_col = tile_idx % tiles_across

    if tile_row >= tiles_down or tile_row < 0 or tile_col >= tiles_across or tile_col < 0:
        # Tile index out of range, return empty tile
        return Image.new('RGBA', (8, 8), (0, 0, 0, 0))

    # Extract tile from PNG
    tile_x = tile_col * 8
    tile_y = tile_row * 8
    tile = tiles_png.crop((tile_x, tile_y, tile_x + 8, tile_y + 8))

    # Convert to RGB using palette
    if palette_idx >= len(palettes):
        # Invalid palette, return magenta tile
        return Image.new('RGBA', (8, 8), (255, 0, 255, 0))

    palette = palettes[palette_idx]
    tile_rgba = Image.new('RGBA', (8, 8), (0, 0, 0, 0))

    for y in range(8):
        for x in range(8):
            pixel_idx = tile.getpixel((x, y))
            if pixel_idx == 0:
                pass  # GBA color 0 = transparent
            elif pixel_idx < 16:
                r, g, b = palette[pixel_idx]
                tile_rgba.putpixel((x, y), (r, g, b, 255))
            else:
                tile_rgba.putpixel((x, y), (255, 0, 255, 255))

    if tile_ref.hflip:
        tile_rgba = tile_rgba.transpose(Image.FLIP_LEFT_RIGHT)
    if tile_ref.vflip:
        tile_rgba = tile_rgba.transpose(Image.FLIP_TOP_BOTTOM)

    return tile_rgba


def render_tile_from_either(tile_ref: TileReference, primary_ts: Tileset, secondary_ts: Tileset, debug: bool = False) -> Image.Image:
    """Render a tile from either primary or secondary tileset based on tile index"""
    # Primary tileset: 512 tiles (indices 0-511)
    # Secondary tileset: variable, indices start from 512

    tile_idx = tile_ref.tile_index

    if tile_idx < 512:
        # Use primary tileset
        return render_tile(tile_ref, primary_ts.tiles_png, primary_ts.palettes)
    else:
        # Use secondary tileset, adjust index
        adjusted_idx = tile_idx - 512

        if adjusted_idx < 0:
            if debug:
                print(f"  DEBUG: Invalid tile idx={tile_idx} < 512, treating as invalid")
            return Image.new('RGBA', (8, 8), (0, 0, 0, 0))

        # Check if within secondary tile range
        img_width, img_height = secondary_ts.tiles_png.size
        tiles_across = img_width // 8
        tiles_down = img_height // 8
        max_tiles = tiles_across * tiles_down

        if adjusted_idx >= max_tiles:
            if debug:
                print(f"  DEBUG: Secondary tile idx={tile_idx} (adjusted={adjusted_idx}) out of range, max={max_tiles}")
            return Image.new('RGBA', (8, 8), (0, 0, 0, 0))

        tile_row = adjusted_idx // tiles_across
        tile_col = adjusted_idx % tiles_across

        if debug and tile_idx < 600:
            print(f"  DEBUG: Secondary tile idx={tile_idx} -> adjusted={adjusted_idx}, pos=({tile_col},{tile_row}) in {tiles_across}x{tiles_down}")

        tile_x = tile_col * 8
        tile_y = tile_row * 8

        # Extract and render
        tile = secondary_ts.tiles_png.crop((tile_x, tile_y, tile_x + 8, tile_y + 8))
        palette = secondary_ts.palettes[tile_ref.palette]

        tile_rgba = Image.new('RGBA', (8, 8), (0, 0, 0, 0))
        for y in range(8):
            for x in range(8):
                pixel_idx = tile.getpixel((x, y))
                if pixel_idx == 0:
                    pass  # transparent
                elif pixel_idx < 16:
                    r, g, b = palette[pixel_idx]
                    tile_rgba.putpixel((x, y), (r, g, b, 255))
                else:
                    tile_rgba.putpixel((x, y), (255, 0, 255, 255))

        if tile_ref.hflip:
            tile_rgba = tile_rgba.transpose(Image.FLIP_LEFT_RIGHT)
        if tile_ref.vflip:
            tile_rgba = tile_rgba.transpose(Image.FLIP_TOP_BOTTOM)

        return tile_rgba


def render_metatile(metatile: Metatile, primary_ts: Tileset, secondary_ts: Tileset, debug: bool = False) -> Image.Image:
    """Render a 16x16 metatile from tileset tiles.

    Each metatile = 2 layers of 2x2 8x8 tiles = 16x16 pixels.
    Tile order within each layer: TL(0,0), TR(8,0), BL(0,8), BR(8,8).
    Bottom layer is drawn first; top layer composited on top.
    """
    result = Image.new('RGBA', (16, 16), (0, 0, 0, 0))

    # TL, TR, BL, BR positions for 8x8 tiles in a 16x16 metatile
    tile_positions = [(0, 0), (8, 0), (0, 8), (8, 8)]

    for layer_name, layer in [("bot", metatile.bottom_layer), ("top", metatile.top_layer)]:
        for i, tile_ref in enumerate(layer):
            pos_x, pos_y = tile_positions[i]

            if debug:
                pos_label = ["TL","TR","BL","BR"][i]
                print(f"    DEBUG: [{layer_name} {pos_label}] idx={tile_ref.tile_index}, pal={tile_ref.palette}, hflip={tile_ref.hflip}, vflip={tile_ref.vflip}")

            try:
                tile_img = render_tile_from_either(tile_ref, primary_ts, secondary_ts, debug)
                result.paste(tile_img, (pos_x, pos_y), mask=tile_img)
            except Exception:
                pass

    return result


def render_map(map_grid: np.ndarray, primary_ts: Tileset, secondary_ts: Tileset, debug: bool = False) -> Image.Image:
    """Render full map from metatile grid"""
    height, width = map_grid.shape
    map_width_px = width * 16
    map_height_px = height * 16

    result = Image.new('RGBA', (map_width_px, map_height_px), (0, 0, 0, 0))

    if debug:
        print(f"  DEBUG: Rendering {width}x{height} metatiles = {map_width_px}x{map_height_px} pixels")

    non_empty_count = 0
    primary_count = 0
    secondary_count = 0

    for y in range(height):
        for x in range(width):
            metatile_id = map_grid[y, x]

            # Determine which metatiles to use
            # Primary: 0-511, Secondary: 512+ (approximately)
            if metatile_id < 512:
                primary_count += 1
                metatile = primary_ts.metatiles[metatile_id]
            else:
                adjusted_id = metatile_id - 512
                secondary_count += 1
                if adjusted_id < len(secondary_ts.metatiles):
                    metatile = secondary_ts.metatiles[adjusted_id]
                else:
                    # Invalid metatile, skip
                    continue

            non_empty_count += 1

            # Render metatile
            try:
                metatile_img = render_metatile(metatile, primary_ts, secondary_ts, debug and (x < 2 and y < 2))
                result.paste(metatile_img, (x * 16, y * 16))
            except Exception as e:
                print(e)
                # Skip invalid metatiles for now
                pass

    if debug:
        print(f"  DEBUG: Rendered {non_empty_count} non-empty metatiles ({primary_count} primary, {secondary_count} secondary)")

    return result


def main():
    import sys

    # Add debug mode flag
    DEBUG_MAP = "LittlerootTown" in sys.argv

    base_dir = Path(__file__).parent.parent / "pokeemerald"
    layouts_json = base_dir / "data" / "layouts" / "layouts.json"
    map_groups_json = base_dir / "data" / "maps" / "map_groups.json"

    output_dir = Path(sys.argv[1]) if len(sys.argv) > 1 else Path("output/maps")
    output_dir.mkdir(parents=True, exist_ok=True)

    print("Loading tilesets and layouts...")
    path_map = build_tileset_path_map(base_dir)
    sprite_map = build_object_event_sprite_map(base_dir)
    print(f"Resolved {len(sprite_map)} object event sprite types")
    layouts = load_layouts(layouts_json)
    maps, map_layouts = load_maps(map_groups_json, base_dir)

    # Link maps to layouts
    for map_obj in maps:
        if map_obj.layout_id in layouts:
            map_obj.layout = layouts[map_obj.layout_id]
        else:
            print(f"Warning: Layout {map_obj.layout_id} not found for map {map_obj.name}")

    print(f"Loaded {len(layouts)} layouts and {len(maps)} maps")
    print(f"Output directory: {output_dir}")

    # Load tilesets (on-demand for now)
    print("\nExtracting maps...")

    # Process all maps
    for i, map_obj in enumerate(maps):
        if i % 50 == 0:
            print(f"\nProgress: {i}/{len(maps)} maps processed...")

        # Enable debug for LittlerootTown
        is_debug = DEBUG_MAP and ("LittlerootTown" in map_obj.name or "BrendansHouse" in map_obj.name)

        if map_obj.layout is None:
            print(f"  Skipping {map_obj.name} - no layout found")
            continue

        layout = map_obj.layout

        # Load tilesets
        try:
            if is_debug:
                print(f"\n  DEBUG: Processing {map_obj.name} (ID: {map_obj.id:04X})")
                print(f"  DEBUG: Layout: {layout.name}")
                print(f"  DEBUG: Primary tileset: {layout.primary_tileset}")
                print(f"  DEBUG: Secondary tileset: {layout.secondary_tileset}")

            primary_ts = load_tileset(layout.primary_tileset, "primary", base_dir, path_map, is_debug)
            secondary_ts = load_tileset(layout.secondary_tileset, "secondary", base_dir, path_map, is_debug)

            # Read map grid
            blockdata_path = base_dir / layout.blockdata_path
            map_grid = read_map_grid(blockdata_path, layout.width, layout.height)

            if is_debug:
                print(f"  DEBUG: Map grid size: {layout.width}x{layout.height}")
                print(f"  DEBUG: First 10 metatile IDs: {map_grid.flatten()[:10]}")

            # Render map tiles
            map_img = render_map(map_grid, primary_ts, secondary_ts, is_debug)

            # Overlay stationary object events (rocks, trees, items, stander NPCs)
            map_json_path = base_dir / "data" / "maps" / map_obj.name / "map.json"
            map_img = composite_object_events(map_img, map_json_path, sprite_map)

            # Save to PNG
            output_filename = f"map_{map_obj.id:04X}_{map_obj.name}.png"
            output_path = output_dir / output_filename
            map_img.save(output_path)

            if is_debug:
                print(f"  DEBUG: Saved to {output_filename}")

        except Exception as e:
            print(f"  Error processing {map_obj.name}: {e}")
            if is_debug:
                import traceback
                traceback.print_exc()

    print(f"\nDone! Extracted maps to: {output_dir}")


if __name__ == "__main__":
    main()
