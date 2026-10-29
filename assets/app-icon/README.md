# PdfCraft app icon

<img src="pdfcraft-logo.png" alt="PdfCraft app icon: a golden retriever puppy on a navy tile" width="128">

**Creature:** a golden retriever puppy, in a frontal portrait, filling the tile.

**Style:** a flat illustration with two-tone fur, on the full-bleed Crafting Apps tile: a colour field
with rounded corners, no frame or roundel, nothing outside the tile.

**Palette:** four colours.

| Colour | Hex | Used for |
|---|---|---|
| Navy | `#011c72` | the full-bleed field |
| Golden fur | `#f8c147` | the puppy |
| Fur shade | `#eaae34` | ears, muzzle and shoulder shading |
| Ink | `#08143b` | eyes, nose and mouth |

**Tile:** a 1254 px rounded square; transparent outside its corners. Windows and Linux icons use
the full-bleed tile. macOS icons put it on Apple's grid (an 824 px body centred on a transparent
1024 px canvas).

**Provenance:** artwork provided by the project owner on 2026-10-29 and kept here as
`pdfcraft-logo.png`. No third-party material. Licence: [LICENSE.txt](LICENSE.txt)
(`MIT OR Apache-2.0`, like the repo).

## Files

| File | What it is |
|---|---|
| `pdfcraft-logo.png` | the master raster (1254 px); every other app-icon file is rendered from it |
| `pdfcraft-1024.png` | 1024 px on Apple's grid; also the runtime Dock icon on macOS |
| `pdfcraft.icns` | macOS icon (16–1024 px) |
| `pdfcraft.ico` | Windows icon (16–256 px), embedded in `pdfcraft.exe` by `apps/pdfcraft/build.rs` |
| `pdfcraft-document.svg` | Windows PDF file icon master: a Lucide document outline, green badge and outlined PDF lettering; no font dependency |
| `pdfcraft-document.ico` | Windows PDF file icon (16–256 px), embedded as resource 2 in `pdfcraft.exe` |
| `hicolor/<n>x<n>/apps/ai.storyteller.pdfcraft.png` | Linux hicolor theme, 16–512 px; the 256 px one is the runtime icon on Windows and Linux |
| `hicolor/scalable/apps/ai.storyteller.pdfcraft.svg` | Linux scalable icon: a self-contained SVG holding the 512 px raster |

Where it shows: `apps/pdfcraft/src/main.rs` sets the window icon (Dock, taskbar, Alt-Tab, launcher) and
the Wayland app id `ai.storyteller.pdfcraft`; `packaging/linux/ai.storyteller.pdfcraft.desktop` names the
hicolor icon.

Windows PDF files use resource 2 (`"pdfcraft.exe",-2`) when PdfCraft is chosen as the default
reader; the app keeps resource 1. The document outline is adapted from Lucide `file-text`
1.49.0 (ISC); its badge and outlined lettering are original (`MIT OR Apache-2.0`), covered by
[LICENSE-document.txt](LICENSE-document.txt). Both icons are embedded, requiring no runtime
sidecar. This provides a file type icon; first-page thumbnails still need a thumbnail provider.

## Regenerate

```sh
packaging/icons.sh        # needs python3 and Pillow; resvg only for pdfcraft-document.ico
cargo xtask assets        # then update the sha256 values in ATTRIBUTION.toml and run with --write
```
