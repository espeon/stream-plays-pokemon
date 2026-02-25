# Pokemon Emerald Map Extractor - Findings

## Data Structure

### Map Organization
- **maps.json**: Master list of 518 maps across 34 groups
- **layouts.json**: 441 layout definitions with dimensions and tilesets
- **Map ID Format**: `(map_num | (group_num << 8))` where group_num is 0-33, map_num is per-group
- **Output naming**: `map_XXXX_MapName.png` where XXXX is the map ID

### File Hierarchy
```
pokeemerald/data/
├── layouts/layouts.json       # Map layouts (441 layouts)
├── maps/map_groups.json      # Map organization (34 groups, 518 maps)
├── maps/{MapName}/
│   ├── map.json           # Map metadata
│   └── {events, scripts, connections}  # Not needed for rendering
└── layouts/{LayoutName}/
    └── map.bin          # Metatile grid (16-bit values)
```

## Tileset Structure

### Primary Tileset
- **Name**: `gTileset_General`
- **Location**: `data/tilesets/primary/general/`
- **Size**: 128×256 pixels = 512 tiles (16×32 grid)
- **Palettes**: 16 palettes of 16 colors each
- **Metatiles**: 2048 metatiles (primary: 0-511)

### Secondary Tilesets
- **Example**: `gTileset_Petalburg` (varies by map)
- **Location**: `data/tilesets/secondary/{name}/`
- **Size**: Varies (e.g., Petalburg: 128×80 = 160 tiles = 16×10 grid)
- **Palettes**: 16 palettes of 16 colors each
- **Metatiles**: Varies (e.g., Petalburg: 576 metatiles)

### Tileset File Formats
- **tiles.png**: Palette-mode PNG with 8×8 tile graphics (4bpp indexed)
- **metatiles.bin**: 4 bytes per metatile (tile references)
- **metatile_attributes.bin**: 2 bytes per metatile (layer type, behavior)
- **palettes/*.pal**: JASC-PAL format (16 RGB colors)

## Metatile Encoding

### 16-Bit Tile Reference Format (from tools/gbagfx/gfx.h `struct NonAffineTile`)
```
Bits 0-9:   Tile index (0-1023)
Bit 10:     Horizontal flip
Bit 11:     Vertical flip
Bits 12-15: Palette index (0-15, 4 bits)
```

### Tiles Per Metatile (from include/fieldmap.h `NUM_TILES_PER_METATILE`)
- **8 tiles per metatile** (not 4)
- **2 layers of 2×2** = 16 bytes per metatile entry in metatiles.bin
- Bottom layer: tiles[0-3] — drawn first
- Top layer: tiles[4-7] — composited on top with transparency

### Metatile Tile Layout (8 tiles, 2 layers of 2×2)
Each layer is a 2×2 grid of 8×8 tiles = 16×16 pixels:
```
Position:  (0,0)   (8,0)   (0,8)   (8,8)
Tiles:      TL      TR      BL      BR
Index:      0       1       2       3   (bottom layer)
            4       5       6       7   (top layer)
```
- Metatile renders to **16×16 pixels** (not 32×32)

### Map Grid Format (map.bin)
- **Format**: 16-bit little-endian values
- **Bit layout**:
  - Bits 0-9: Metatile ID (0-1023)
  - Bits 10-11: Collision (0-3)
  - Bits 12-15: Elevation (0-15)

## Tile Index Mapping

### Primary Tileset
- **Indices**: 0-511
- **Source**: Primary tileset (`gTileset_General`)

### Secondary Tileset
- **Indices**: 512+ (subtract 512 to get actual index)
- **Source**: Secondary tileset (varies by map: Petalburg, Lavaridge, etc.)
- **Limitation**: Check against tileset's actual tile count (not all have 480+ tiles)

### Metatile ID Mapping
- **0-511**: Primary tileset metatiles
- **512+**: Secondary tileset metatiles (adjusted: ID - 512)

## Rendering Pipeline

1. Read map.bin → Extract metatile IDs
2. For each metatile ID:
   - Get metatile from tileset (primary or secondary)
   - Decode 4 tile references
   - Check for empty tile flag (bit 10)
   - Render each 8×8 tile with palette and flips
   - Compose 4 tiles into 32×32 metatile
3. Paste metatiles into full map
4. Save as PNG

## Palette Handling

### Palette Selection
- **Per-tile palette index**: Bits 12-15 of tile reference (4 bits → 0-15)
- **16 colors per palette**: Maps to 4bpp indexed color
- **Primary tilesets**: 16 palette files (00-15.pal); hardware uses slots 0-5
- **Secondary tilesets**: 16 palette files (00-15.pal); hardware uses slots 6-12
- Palette index in tile reference is absolute (0-15); use it directly against the owning tileset's palette list

### Color Conversion
- **Format**: GBA 15-bit RGB → 24-bit RGB
- **R**: 5-bit value × 255/31
- **G**: 5-bit value × 255/31
- **B**: 5-bit value × 255/31
- **Formula**: `color_24bit = (gba_value << 3) | (gba_value >> 2) | (gba_value >> 7)`

## Known Issues and Limitations

### Rendering Issues
1. **Empty tile detection**: Must check bit 10 (0x0400) for empty tiles
2. **Tileset name matching**: Need flexible matching (case-insensitive, underscore handling)
3. **Secondary tile count**: Varies by tileset, need to check bounds
4. **Tile flip bits**: Bits 13 (vertical) and 14 (horizontal) - order may vary
5. **Top layer rendering**: Currently unused (tile indices 3, 4 are 0)

### Tileset Issues
1. **Missing tilesets**: Some layouts reference non-existent tilesets (SecretBase*, GenericBuilding)
2. **File format**: All assets are PNG format (no .4bpp.lz files to decompress)
3. **Path inconsistency**: Tileset directories may use different naming conventions

### Debug Mode
- **Enable**: Pass `LittlerootTown` as argument to `extract_maps.py`
- **Outputs**:
  - Tileset directory paths
  - Tileset dimensions (PNG size → tile count)
  - Palette counts
  - Metatile counts
  - Map grid dimensions
  - First 10 metatile IDs
  - Per-tile debugging (indices, palettes, flips)
  - Rendering statistics (primary vs secondary count)

## Current Status

### We have tried so far
- ✓ Full map rendering (metatile grid → PNG)
- ✓ Debug mode for LittlerootTown

### Needs to be fixed: 
- Incorrect colors
- Incorrect sizing for map (zoomed out?) possibly taken from incorrect spot on spritesheet

### Map Examples
- **PetalburgCity**: 30×30 metatiles = 480×480 pixels (16px per metatile)
- **MtChimney**: 40×47 metatiles = 640×752 pixels
- **LittlerootTown_BrendansHouse_1F**: 11×9 metatiles = 176×144 pixels

## File Format Notes

### JASC-PAL Format
```
JASC-PAL
0100
16
255 255 255
255 255 255
... (16 RGB triples)
```
- Line 1: Format identifier
- Line 2: Version (0100 = 1.0)
- Line 3: Color count (16)
- Lines 4-19: RGB values (0-255)

### PNG Format
- **Mode**: Palette mode (P)
- **Dimensions**: Strip of 8×8 tiles
- **Pixels**: 4bpp indexed (0-15 per pixel)
- **Size**: Example: 128×256 = 512 tiles = 16×32 grid

## Optimization Opportunities

1. **Batch rendering**: Pre-render common metatiles
2. **Caching**: Cache rendered metatiles
3. **Parallel processing**: Render multiple maps simultaneously
4. **Palette lookup tables**: Pre-convert GBA colors to 24-bit

## Future Improvements

1. **Top layer rendering**: Implement top layer (tile indices 3, 4)
2. **Metatile attributes**: Use layer type to determine rendering order
3. **Animation support**: Render animated tiles (if needed)
4. **Border rendering**: Extract and render map borders
5. **Error handling**: Better handling of missing tilesets and invalid data
6. **Validation**: Cross-reference with in-game screenshots

## Technical Debt

1. **Tile order within metatile**: TL/TR/BL/BR assumed based on GBA tilemap row-major convention; needs visual confirmation
2. **Top-layer transparency**: Index 0 of top layer tiles is transparent; currently skipped by pasting opaque tiles — verify correct compositing

## References

- **Pokeemerald decomp**: `/Users/natalie/code/stream-plays-emerald/pokeemerald/`
- **GBA hardware docs**: 4bpp graphics, tile modes, layer types
- **Metatile documentation**: GBATEK documentation on map/metadata
- **Graphics tools**: gbagfx (PNG conversion, palette handling)

## Debug Commands

### Run LittlerootTown with debug
```bash
cd mapextract
source .venv/bin/activate
python extract_maps.py LittlerootTown
```

### Run full extraction (no debug)
```bash
cd mapextract
source .venv/bin/activate
python extract_maps.py
```

### Check rendered output
```bash
# Check map file size
ls -lh output/maps/map_0000_PetalburgCity.png

# Check dimensions (should be width×32, height×32)
source .venv/bin/activate && python3 << 'EOF'
from PIL import Image
img = Image.open('output/maps/map_0000_PetalburgCity.png')
print(f"Size: {img.size[0]}x{img.size[1]} pixels")
EOF
```

## Key Learnings
1. **Interior maps are dense**: Buildings/houses have more content
2. **Tileset reuse**: Many maps share the same tilesets
3. **Debug mode is essential**: Helps identify rendering and decoding issues quickly

## Dependencies

- **Python 3.8+**
- **Pillow >= 10.0.0**: PNG I/O and image manipulation
- **NumPy >= 1.20.0**: Array operations
- **uv**: Package manager for dependencies

Install with:
```bash
cd mapextract
uv venv
source .venv/bin/activate
uv pip install pillow numpy
```
