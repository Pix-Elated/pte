# Pixelated's Tibia Editor — Feature Roadmap

Comprehensive gap analysis vs. RME (Remere's Map Editor) and planned improvements.

## ✅ Currently Implemented

### Core
- [x] OTBM map loading/saving (via pte-otbm crate)
- [x] Protobuf appearances loading (appearances.dat)
- [x] Sprite sheet loading + rendering
- [x] Camera pan (Ctrl+drag, middle-mouse) + zoom (scroll wheel)
- [x] Z-level navigation (layers panel)
- [x] Animated sprite rendering in viewport

### Tools
- [x] Brush tool (paint ground + items)
- [x] Eraser tool
- [x] Fill tool (flood fill)
- [x] Select tool (rectangle selection)
- [x] Eyedropper tool (pick item from map)
- [x] Door brush (normal/locked/magic/quest/hatch/window variants)
- [x] Creature brush
- [x] Spawn brush
- [x] Waypoint brush

### Brushes
- [x] Ground brush (with auto-bordering)
- [x] Wall brush (auto-joins)
- [x] Carpet brush
- [x] Table brush
- [x] Doodad brush
- [x] Raw brush (direct item placement)
- [x] Border processing (auto-border generation)
- [x] Brush palette with category filters
- [x] Brush shapes (square, circle)

### Sprite Management
- [x] Sprite picker grid with search + pagination
- [x] Sprite detail side panel (frame groups, flags, properties)       ← NEW
- [x] Pixel art sprite editor (pencil, eraser, fill, line, rect, eyedropper)
- [x] Sprite CRUD (new blank, duplicate, delete)
- [x] PNG import/export for sprites
- [x] Save sprites back to sprite sheets on disk

### UI
- [x] Welcome/launcher screen
- [x] Dark theme throughout
- [x] Toolbar with tool selection
- [x] Status bar (tile coords, tool info)
- [x] Undo/redo (100 action stack)
- [x] Stroke batching (drag = single undo action)
- [x] LOD rendering for zoom-out performance                           ← NEW
  - LOD 0: tiles < 1px → skip
  - LOD 1: tiles < 4px → minimap colors (automap or hash)
  - LOD 2: tiles < 8px → ground-only sprites
  - LOD 3: tiles ≥ 8px → full detail + grid + items

---

## 🔲 Missing Features — RME Parity

### Priority 1 — Critical for Mapping Workflow

#### Selection Operations
- [ ] Cut / Copy / Paste tiles (clipboard buffer)
- [ ] Paste-and-drag preview (ghost stamp before commit)
- [ ] Selection move (drag selected area to new position)
- [ ] Delete selection
- [ ] Randomize selection (swap matching items for random variants)
- [ ] Borderize selection (recalculate borders for selected tiles)

#### Minimap Window
- [ ] Persistent minimap panel (bird's-eye view of entire map)
- [ ] Click-to-navigate on minimap
- [ ] Viewport rectangle indicator on minimap
- [ ] Minimap export to PNG (selectable sizes like RME)

#### Multiple Map Tabs
- [ ] Open multiple maps simultaneously (tabbed editors)
- [ ] Drag between tabs (copy tiles across maps)
- [ ] Import map (merge one map into another at offset)

#### Search & Find
- [ ] Find item by ID dialog (jump to tile containing item)
- [ ] Search: unique items, action items, containers, writeables
- [ ] Search: duplicated items, walls-upon-walls
- [ ] Replace items dialog (mass-replace one item ID with another)
- [ ] Remove items from selection by type
- [ ] Search results panel with clickable entries

#### House System
- [ ] House brush — paint tiles as belonging to a house
- [ ] House exit brush — mark house exit tile
- [ ] House palette — list/create/edit/delete houses
- [ ] House properties: ID, name, rent, beds, size, town
- [ ] Import/export house files
- [ ] Clear invalid house references
- [ ] Go-to-house navigation

#### Town Management
- [ ] Town editor dialog (create, edit, delete towns)
- [ ] Town temple position setting
- [ ] Town-to-house association

### Priority 2 — Important Quality of Life

#### Zone System
- [ ] Zone brush — paint named zones on tiles
- [ ] Zone palette — create/edit/delete zones
- [ ] Zone import/export
- [ ] Protection Zone flag brush (PZ)
- [ ] No-PvP flag brush
- [ ] PvP Zone flag brush
- [ ] No-Logout flag brush

#### Waypoint System (extended)
- [ ] Waypoint palette — list all waypoints with go-to navigation
- [ ] Waypoint name editing
- [ ] Waypoint visualization on map

#### Monster/NPC Spawn System (extended)
- [ ] Monster palette — browse all registered monsters
- [ ] NPC palette — browse all registered NPCs
- [ ] Load monsters/NPCs from Canary Lua files
- [ ] Spawn radius visualization (circle overlay)
- [ ] Edit spawn time from selection
- [ ] Monster/NPC count statistics
- [ ] Remove empty spawn areas

#### Map Operations
- [ ] Map statistics dialog (tile counts, item counts, entity counts)
- [ ] Map properties dialog (description, dimensions, spawn/house files)
- [ ] Map cleanup (remove redundant data)
- [ ] Map remove: corpses, unreachable tiles, empty spawns
- [ ] Generate blank map with dimensions
- [ ] Go-to-position dialog (jump to x,y,z)
- [ ] Go-to-previous-position (nav history)

#### Brush Features
- [ ] Brush size toolbar (visual slider 1-10+)
- [ ] Brush variation (auto-select random variant)
- [ ] Brush thickness modifier
- [ ] Optional border brush
- [ ] Eraser flags mode (erase only zone/house markers)

### Priority 3 — Power Features

#### Live Collaboration
- [ ] Live server hosting (others can connect and co-edit)
- [ ] Live client joining
- [ ] Real-time cursor/selection visibility
- [ ] Action syncing via network packets

#### Tilesets Manager
- [ ] Create/edit custom tilesets (groupings of related items)
- [ ] Add items to/from tilesets
- [ ] Tileset categories in palette
- [ ] Tileset export/import

#### Doodad Editor
- [ ] Doodad palette — composite multi-tile decorations
- [ ] Doodad preview buffer
- [ ] Paint doodads as atomic unit
- [ ] Create new doodads from selection

#### Lighting System
- [ ] Light source visualization (glow radius on map)
- [ ] Light strength overlay toggle
- [ ] Edit light properties per item

#### View Options
- [ ] Show/hide: monsters, NPCs, spawns, items, ground
- [ ] Ghost items (semi-transparent items on other floors)
- [ ] Ghost higher floors (see-through when editing underground)
- [ ] Show ingame box (client viewport rectangle)
- [ ] Show as minimap (automap colors for entire view)
- [ ] Show only modified tiles
- [ ] Show pickupable/moveable/avoidable item highlights
- [ ] Show wall hooks overlay
- [ ] Shade effect (darken non-selected areas)
- [ ] Show tooltips for items on hover

#### Hotkey System
- [ ] 10 configurable hotkeys (each binds a position or brush)
- [ ] Jump to hotkey position
- [ ] Switch to hotkey brush
- [ ] Hotkey editor in preferences

#### Item Properties
- [ ] Tile browse dialog (click to view all items on a tile, reorder stack)
- [ ] Item properties editor (action IDs, unique IDs, text, destination)
- [ ] Container contents editor
- [ ] Item overlay order customization

### Priority 4 — Modern Extras (Beyond RME)

#### Lua Scripting
- [ ] Embedded Lua script runner (automate repetitive map edits)
- [ ] Script manager UI (RME recently added this)
- [ ] Script API for tile/item/brush manipulation

#### Performance & Rendering
- [ ] Tile chunk caching (pre-render chunks to texture atlas at low zoom)
- [ ] Background thread for sprite decompression
- [ ] GPU-accelerated tile rendering (instanced draw calls)
- [ ] Progressive loading for huge maps (stream chunks on demand)

#### Import/Export
- [ ] Import from older OTBM formats (v76, v81, v854)
- [ ] Export minimap image (full map render)
- [ ] Export tileset XML
- [ ] Import/export monster spawn XML
- [ ] Import/export NPC spawn XML

#### Collaboration & Versioning
- [ ] Autosave with configurable interval
- [ ] Crash recovery (save state periodically)
- [ ] Map diff viewer (compare two maps)

#### Sprite Editor Enhancements
- [ ] Animation frame editor (reorder/add/remove frames)
- [ ] Outfit colorizer (apply head/body/legs/feet colors)
- [ ] Batch sprite operations (resize all, recolor all)
- [ ] Sprite atlas viewer (see full sheet layout)

#### UX Polish
- [ ] Preferences dialog (paths, defaults, keybinds)
- [ ] Recent files list
- [ ] Drag-and-drop .otbm files to open
- [ ] Fullscreen toggle
- [ ] Performance monitor (FPS, tile count, memory)
- [ ] Welcome dialog with quick actions + recent maps
- [ ] Context menu on right-click (tile operations)
- [ ] Keyboard shortcuts reference panel (? key)

---

## Implementation Priority Queue

**Next sprint (immediate):**
1. Cut/Copy/Paste with clipboard buffer
2. Go-to-position dialog
3. Minimap panel
4. Find/Replace items
5. Map statistics

**Following sprint:**
6. House system (brush, palette, properties)
7. Zone system (brush, palette, flags)
8. Tile browse/properties dialog
9. View toggles (show/hide entity types)
10. Brush size slider + variation

**Medium term:**
11. Monster/NPC palettes with Lua loader
12. Waypoint palette
13. Town editor
14. Tilesets manager
15. Doodad editor

**Long term:**
16. Live collaboration
17. Lua scripting engine
18. GPU chunk caching
19. Hotkey system
20. Map diff viewer
