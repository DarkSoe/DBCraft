# DBCraft app icon

A **beaver engraving portrait**: Crafting Apps framing (full-bleed field, head and body
filling the tile), three-colour palette.

## Palette

| Colour | Hex | Used for |
|---|---|---|
| Ink | `#0b0b0c` | line work and contours |
| Paper | `#efe9dc` | the figure |
| DBCraft blue | `#1B6CA8` | the full-bleed field |

## Files

- `source.png` / `dbcraft-1024.png`: master raster
- `dbcraft.ico`: Windows (16–256)
- `hicolor/<size>/apps/ai.storyteller.dbcraft.png`: Linux / window icon sizes
- `hicolor/scalable/...svg`: geometric placeholder until a vector trace exists

Window icon is loaded at runtime from the 256×256 PNG (`crates/ui-egui`).
