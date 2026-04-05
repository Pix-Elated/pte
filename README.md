# Pixelated's Tibia Editor (PTE)

A modern, GPU-accelerated map and sprite editor for Open Tibia servers. Built in Rust with [egui](https://github.com/emilk/egui) for a fast, cross-platform native experience.

> **Status:** Early alpha — actively developed, expect rough edges. Feedback and bug reports are very welcome!

![License](https://img.shields.io/badge/license-MIT-blue)
![Rust](https://img.shields.io/badge/rust-2021%20edition-orange)
![Platform](https://img.shields.io/badge/platform-Windows%20x64-lightgrey)

---

## Features

### Map Editor
- **OTBM map loading & saving** — full support for the standard Open Tibia Binary Map format
- **Protobuf appearance system** — loads `appearances.dat` + `catalog-content.json` + CIP sprite sheets (LZMA-compressed BMP)
- **Legacy format support** — reads/writes `Tibia.dat` + `Tibia.spr` for older protocol versions (8.60, 10.98)
- **Camera navigation** — Ctrl+drag or middle-mouse to pan, scroll wheel to zoom
- **Z-level navigation** — full floor support (0–15) with ground floor at z=7
- **Animated sprites** — rendered live in the viewport with proper frame timing
- **LOD rendering** — automatic detail reduction at low zoom levels for smooth performance
  - Tiles < 1px → skipped
  - Tiles < 4px → minimap colors
  - Tiles < 8px → ground-only sprites  
  - Tiles ≥ 8px → full detail with grid, items, and overlays

### Tools
| Tool | Hotkey | Description |
|------|--------|-------------|
| Brush | `B` | Paint ground tiles and items with auto-bordering |
| Eraser | `E` | Remove items from tiles |
| Selective Eraser | — | Remove specific item types (ground, walls, items, etc.) |
| Fill | `F` | Flood-fill an area with the selected brush |
| Select | `S` | Rectangle selection for multi-tile operations |
| Eyedropper | `I` | Pick an item from the map |
| Door Brush | — | Place doors (normal, locked, magic, quest, hatch, window) |
| Creature Brush | — | Place creatures/monsters on the map |
| Spawn Brush | — | Place spawn areas with configurable radius |
| Waypoint Brush | — | Place named waypoints |

### Brush System
- **Ground brushes** with automatic border generation
- **Wall brushes** with auto-joining
- **Carpet, Table, and Doodad brushes**
- **Raw brush** for direct item ID placement
- **Brush palette** with category filters and search
- **Configurable brush shapes** (square, circle) and sizes

### Sprite Management
- **Sprite picker** — searchable grid with pagination and category filters
- **Sprite detail panel** — view frame groups, animation phases, appearance flags, and properties
- **Pixel art sprite editor** with:
  - Pencil, eraser, fill, line, and rectangle tools
  - Color palette with eyedropper
  - Undo/redo stack
  - Zoom (1×–40× with scroll wheel) and Ctrl+drag panning
  - Grid overlay toggle
  - Animation frame navigation
- **Sprite CRUD** — create blank sprites, duplicate existing ones, delete
- **PNG import/export** — bring in external artwork or export sprites
- **Save to disk** — write modified sprites back to CIP sprite sheets

### Project Management
- **Asset scanner** — auto-detects OT project structure (client data, maps, server configs)
- **New Project wizard** — create a fresh project from scratch with:
  - Blank `appearances.dat` + sprite sheets (protobuf format), or `Tibia.dat` + `Tibia.spr` (legacy)
  - Blank OTBM map with configurable dimensions
  - `config.lua`, `spawn.xml`, `houses.xml`
  - Optional: auto-fetch and set up [Canary](https://github.com/opentibiabr/canary) server
  - Optional: auto-fetch and set up [OTClient](https://github.com/mehah/otclient)
- **Multiple maps** — open and switch between multiple maps via the map switcher
- **Map import** — merge maps at configurable offsets

### UI & Quality of Life
- **Dark theme** throughout
- **Toolbar** with tool selection and brush size
- **Status bar** with tile coordinates, tool info, and performance stats
- **Undo/redo** — 100-action stack with stroke batching (a full drag = one undo action)
- **Keyboard shortcuts** for all major tools and actions
- **Context menu** — right-click on tiles for quick actions
- **View overlays** — toggleable grid, client viewport box, light overlay, shade
- **Minimap export** — export map floors to PNG at configurable scale
- **Town editor** — create and manage towns with temple positions
- **Map properties** — edit map description, dimensions, spawn/house file references
- **Go-to-position** dialog with navigation history
- **Performance monitor** overlay (FPS, frame time, tile counts)
- **Auto-updater** — checks GitHub releases for new versions

---

## Architecture

PTE is a Rust workspace with five crates:

```
pte/
├── crates/
│   ├── assets/          # CIP sprite sheet loader (LZMA + BMP decoding)
│   │                     # Catalog parsing, sprite extraction, and sheet saving
│   ├── appearances/     # Protobuf appearances.dat reader/writer
│   │                     # Generated from otclient.protobuf.appearances.proto
│   ├── otbm/            # OTBM map format reader/writer
│   │                     # Tiles, items, towns, waypoints, spawns
│   ├── spr_dat/         # Legacy Tibia.spr + Tibia.dat reader/writer
│   │                     # For protocol versions ≤ 10.98
│   └── editor/          # The GUI application (egui + eframe)
│                         # All UI panels, tools, brushes, and rendering
├── assets/
│   └── fonts/           # Bundled fonts
├── Cargo.toml           # Workspace manifest
└── ROADMAP.md           # Feature gap analysis vs RME
```

### Key Dependencies
- **[egui](https://github.com/emilk/egui)** / **[eframe](https://github.com/emilk/egui/tree/master/crates/eframe)** — immediate-mode GUI framework
- **[prost](https://github.com/tokio-rs/prost)** — protobuf encoding/decoding for appearances
- **[lzma-rs](https://github.com/gendx/lzma-rs)** — LZMA decompression for CIP sprite sheets
- **[rayon](https://github.com/rayon-rs/rayon)** — parallel sprite sheet loading
- **[reqwest](https://github.com/seanmonstar/reqwest)** — HTTP client for auto-updater and GitHub release fetching
- **[rfd](https://github.com/PolyMeilex/rfd)** — native file dialogs

---

## Building from Source

### Prerequisites
- **Rust 1.75+** (install via [rustup](https://rustup.rs))
- **Windows 10/11 x64** (primary target; Linux/macOS support planned)
- **Visual Studio Build Tools** or full Visual Studio with C++ workload (for MSVC linker)

### Build
```bash
# Clone the repository
git clone https://github.com/Pix-Elated/pte.git
cd pte

# Debug build (faster compile, slower runtime)
cargo build

# Release build (slower compile, optimized runtime — recommended)
cargo build --release
```

The binary will be at:
- Debug: `target/debug/pte.exe`
- Release: `target/release/pte.exe`

### Run
```bash
# Run directly
cargo run --release

# Or run the built binary
./target/release/pte.exe
```

---

## Getting Started

### Opening an Existing OT Project

1. Launch PTE — you'll see the welcome screen
2. Click **"Open Project…"**
3. Navigate to your OT project root (the directory containing your client `data/` folder, maps, and/or server configs)
4. PTE will scan the directory and detect:
   - Client assets (`catalog-content.json` / `appearances.dat` / sprite sheets, or `Tibia.dat` / `Tibia.spr`)
   - OTBM maps
   - Server configuration files
5. Select the project from the scan results to load it

### Creating a New Project from Scratch

1. Click **"New Project…"** on the welcome screen
2. Configure:
   - **Project name** — used for directory name, map name, and config
   - **Location** — parent directory where the project folder will be created
   - **Protocol version** — determines the asset format:
     - 13.40+ → Protobuf (catalog-content.json + appearances.dat + .cip sheets)
     - 12.90 → Protobuf
     - 10.98 → Legacy (Tibia.dat + Tibia.spr)
     - 8.60 → Legacy
   - **Map size** — width × height in tiles (default 2048×2048)
3. Optionally enable:
   - **Fetch Canary server** — downloads the latest release and configures it with your map
   - **Fetch OTClient** — downloads the latest release and copies your assets into it
4. Click **"Create Project"** — PTE generates all files and opens the project

### Project Structure Created
```
my-ot-server/
├── config.lua              # Server configuration
├── data/
│   ├── catalog-content.json  # (protobuf) or Tibia.dat + Tibia.spr (legacy)
│   ├── appearances.dat       # (protobuf only)
│   ├── 0.cip                 # (protobuf only) sprite sheet
│   └── world/
│       ├── my-ot-server.otbm  # Blank map with 10×10 ground patch at center
│       ├── spawn.xml           # Empty spawn file
│       └── houses.xml          # Empty house file
├── server/                   # (if Canary was fetched)
│   └── ...                   # Canary server files with map configured
└── client/                   # (if OTClient was fetched)
    └── ...                   # OTClient files with assets configured
```

### Typical Workflow

1. **Paint the ground** — select a ground brush from the palette, use the brush tool (`B`) to paint terrain
2. **Add walls and structures** — switch to wall/doodad brushes, PTE auto-joins walls as you paint
3. **Place items** — use the raw brush or search by item ID in the sprite picker
4. **Add creatures & spawns** — use creature/spawn brushes to populate the map
5. **Set up towns** — use the town editor to define towns and temple positions
6. **Place waypoints** — mark important locations with named waypoints
7. **Save** — `Ctrl+S` saves the map back to OTBM format

---

## Keyboard Shortcuts

### Tools
| Shortcut | Action |
|----------|--------|
| `B` | Brush tool |
| `E` | Eraser tool |
| `F` | Fill tool |
| `S` | Select tool |
| `I` | Eyedropper |

### Navigation
| Shortcut | Action |
|----------|--------|
| `Ctrl + Drag` | Pan the map |
| `Middle Mouse Drag` | Pan the map |
| `Scroll Wheel` | Zoom in/out |
| `Page Up` / `Page Down` | Move up/down one floor |
| `Ctrl + G` | Go to position |

### File Operations
| Shortcut | Action |
|----------|--------|
| `Ctrl + S` | Save map |
| `Ctrl + Shift + S` | Save map as |
| `Ctrl + Z` | Undo |
| `Ctrl + Y` | Redo |

### View
| Shortcut | Action |
|----------|--------|
| `G` | Toggle grid overlay |
| `L` | Toggle light overlay |

---

## Supported Formats

### Input
| Format | Extension | Description |
|--------|-----------|-------------|
| OTBM | `.otbm` | Open Tibia Binary Map (versions 1–3) |
| CIP Sprite Sheet | `.cip` | LZMA-compressed BMP sprite sheets |
| Appearances | `appearances.dat` | Protobuf-encoded appearance definitions |
| Catalog | `catalog-content.json` | Sprite sheet index and metadata |
| Legacy Sprites | `Tibia.spr` | Classic sprite format (8.60–10.98) |
| Legacy Data | `Tibia.dat` | Classic item/outfit/effect definitions |
| PNG | `.png` | Import sprites from PNG images |
| Spawn XML | `spawn.xml` | Creature spawn definitions |

### Output
| Format | Extension | Description |
|--------|-----------|-------------|
| OTBM | `.otbm` | Saves maps in the same format |
| CIP Sprite Sheet | `.cip` | Saves modified sprite sheets |
| Appearances | `appearances.dat` | Saves modified appearances |
| Legacy Sprites | `Tibia.spr` | Saves modified legacy sprites |
| Legacy Data | `Tibia.dat` | Saves modified legacy data |
| PNG | `.png` | Export individual sprites |
| Minimap PNG | `.png` | Export map floor as minimap image |

---

## Auto-Updater

PTE includes a built-in auto-updater that checks GitHub releases on startup. When a new version is available, you'll see a notification in the status bar with an option to download and install the update.

The updater compares your current version against the latest GitHub release tag and offers a one-click update that downloads the new binary and replaces the current one.

---

## Development

### Running Tests
```bash
cargo test --workspace
```

### Checking for Issues
```bash
# Type checking
cargo check --workspace

# Linting
cargo clippy --workspace -- -W clippy::all

# Formatting
cargo fmt --check
```

### Branch Model
- **`staging`** — integration branch; all feature/fix PRs target this branch
- **`master`** — stable release branch; auto-promoted from staging via release PRs
- Feature branches: `feat/description`, `fix/description`, `refactor/description`

---

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines on:
- Branch naming conventions
- Commit message format (conventional commits)
- PR workflow
- Code style

---

## Roadmap

See [ROADMAP.md](ROADMAP.md) for the full feature gap analysis against RME (Remere's Map Editor) and planned improvements.

Key upcoming features:
- **Clipboard operations** — cut, copy, paste, paste-and-drag preview
- **House system** — house brush, house editor, house palette
- **Minimap panel** — persistent bird's-eye view with click-to-navigate
- **Zone system** — protection zones, PvP zones, no-logout zones
- **Find & replace** — search items by ID, replace across the map
- **Linux & macOS builds**

---

## License

MIT — see [LICENSE](LICENSE) for details.

---

## Credits

Built by the [Pix-Elated](https://github.com/Pix-Elated) team.

PTE is not affiliated with or endorsed by CipSoft GmbH. Tibia is a registered trademark of CipSoft GmbH.
