# Pokemon Emerald Map Extractor

Extracts all Pokemon Emerald maps as PNG images from the pokeemerald decomp.

## Setup

```bash
cd mapextract
uv venv
source .venv/bin/activate  # On Linux/Mac
.venv\Scripts\activate  # On Windows
```

## Usage

```bash
# Extract all maps
python extract_maps.py

# Extract to custom output directory
python extract_maps.py /path/to/output
```

## Output

Maps are saved as `map_XXXX_MapName.png` where:
- `XXXX` is the numeric map ID (group << 8 | map_num)
- `MapName` is the map name from the decomp

Example: `map_0000_PetalburgCity.png`

## Map ID Format

The numeric map ID is structured as: `(map_num << 8) | group_num`

- **Bits 0-7**: Map number within group (0-255)
- **Bits 8-15**: Group number (0-33)

## Data Structure

### Files Read

1. **pokeemerald/data/layouts/layouts.json** - Map layout definitions
2. **pokeemerald/data/maps/map_groups.json** - Map group organization
3. **pokeemerald/data/maps/{MapName}/map.json** - Individual map metadata
4. **pokeemerald/data/layouts/{LayoutName}/map.bin** - Map metatile grid

### Tilesets

Located in `pokeemerald/data/tilesets/{primary,secondary}/{name}/`

- **tiles.png** - 4bpp tile graphics (8x8 tiles)
- **palettes/*.pal** - JASC-PAL format palettes (16 colors each)
- **metatiles.bin** - Metatile definitions (4 bytes each)
- **metatile_attributes.bin** - Layer type and behavior data

### Metatile Structure

Each metatile is a 32×32 pixel block composed of four 8×8 tiles:

```
┌───────┬───────┐
│   TL   │   TR   │ Top layer
├───────┼───────┤
│   BL   │   BR   │ Bottom layer
└───────┴───────┘
```

The map grid references metatiles, which reference tiles in tilesets.

### Rendering Process

1. Read map grid from `map.bin`
2. For each metatile in grid:
   - Decode metatile tile references
   - Render each 8×8 tile with palette and flip
   - Composite into 32×32 block
3. Render full map at full resolution

## Limitations

- Rendering is approximate - some tileset layouts may not be perfectly accurate
- Some maps may fail to render if tileset names don't match
- Animation tiles are rendered as static frames
- Borders are not rendered

## Examples

```bash
# Extract maps to ~/maps/
python extract_maps.py ~/maps

# Process first 10 maps for testing
# Edit extract_maps.py, change `enumerate(maps)` to `enumerate(maps[:10])`
```

## Troubleshooting

**Missing tileset errors**: The script attempts case-insensitive lookup, but some tileset names in layouts.json may not match the actual directory names exactly.

**Blank/incorrect maps**: The tile and palette decoding may need adjustment. The current encoding is an initial hypothesis based on GBA hardware conventions.

## Dependencies

- Python 3.8+
- Pillow >= 10.0.0
- NumPy >= 1.20.0

Install with:
```bash
uv pip install pillow numpy
```
