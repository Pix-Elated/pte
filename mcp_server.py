#!/usr/bin/env python3
"""
Pixelated's Tibia Editor — MCP Server

Enterprise-grade MCP bridge between AI agents and PTE's native map editor.
Communicates with the editor's embedded HTTP API (localhost:9982) via JSON.

Capabilities:
  - Tools:     Full map editing (tiles, items, areas, batch ops) with progress
  - Resources: Live map metadata, stats, editor status
  - Prompts:   Guided workflows for common editing tasks
  - Progress:  Per-step reporting on bulk/batch operations
  - Logging:   Structured notifications to the harness (info/warn/error)

Install:
    pip install "mcp[cli]" httpx

Run:
    python mcp_server.py

Or add to MCP config (VS Code / Claude Desktop):
    {
        "mcpServers": {
            "pte": {
                "command": "python",
                "args": ["path/to/pte/mcp_server.py"]
            }
        }
    }
"""

from __future__ import annotations

import json
from collections.abc import AsyncIterator
from contextlib import asynccontextmanager
from dataclasses import dataclass
from typing import Any

import httpx
from pydantic import BaseModel, Field

from mcp.server.fastmcp import FastMCP, Context
from mcp.server.session import ServerSession

# ── Constants ───────────────────────────────────────────────────────

EDITOR_URL = "http://localhost:9982"
SERVER_VERSION = "2.0.0"
BATCH_MAX = 500
AREA_TILE_LIMIT = 10_000


# ── Pydantic models for structured output ───────────────────────────

class EditorStatus(BaseModel):
    """Current editor state snapshot."""
    mode: str = Field(description="Editor mode (Welcome, MapEditor)")
    assets_ready: bool = Field(description="Whether game assets are loaded")
    map_loaded: bool = Field(description="Whether a map is currently open")
    tile_count: int = Field(description="Total tiles in the loaded map")
    camera_x: float = Field(description="Camera center X world coordinate")
    camera_y: float = Field(description="Camera center Y world coordinate")
    camera_z: int = Field(description="Active Z-level (0=sky, 7=ground, 15=deep)")
    zoom: float = Field(description="Current zoom level")
    active_tool: str = Field(description="Name of the active tool/brush")
    selected_item_id: int | None = Field(description="Item ID of active brush (null if none)")
    can_undo: bool
    can_redo: bool


class TileData(BaseModel):
    """Tile at a specific map position."""
    x: int
    y: int
    z: int
    ground: int | None = Field(description="Ground item ID (null if no ground)")
    items: list[dict[str, Any]] = Field(description="Stacked items [{id, action_id?, unique_id?, text?, tele_dest?}]")
    flags: int = Field(description="Tile flags bitfield")
    house_id: int | None = Field(description="House ID if tile belongs to a house")


class MapMetadata(BaseModel):
    """Map file metadata."""
    width: int
    height: int
    description: str
    spawn_file: str | None = Field(alias="spawnFile", default=None)
    house_file: str | None = Field(alias="houseFile", default=None)
    tile_count: int = Field(alias="tileCount")
    town_count: int = Field(alias="townCount")
    waypoint_count: int = Field(alias="waypointCount")


class MapStats(BaseModel):
    """Map statistics."""
    tile_count: int = Field(alias="tileCount")
    chunk_count: int = Field(alias="chunkCount")
    z_levels: list[int] = Field(alias="zLevels", description="Occupied Z-levels")
    z_range: dict[str, int] | None = Field(alias="zRange", description="{min, max} or null")
    town_count: int = Field(alias="townCount")
    waypoint_count: int = Field(alias="waypointCount")


class BatchResult(BaseModel):
    """Result of a batch operation."""
    total: int = Field(description="Total operations submitted")
    succeeded: int = Field(description="Operations that succeeded")
    failed: int = Field(description="Operations that failed")
    results: list[dict[str, Any]] = Field(description="Per-operation results in order")


# ── Lifespan (persistent HTTP client) ──────────────────────────────

@dataclass
class AppContext:
    """Shared state across the server lifetime."""
    client: httpx.AsyncClient


@asynccontextmanager
async def lifespan(_server: FastMCP) -> AsyncIterator[AppContext]:
    """Manage a persistent HTTP client for editor communication."""
    async with httpx.AsyncClient(
        base_url=EDITOR_URL,
        timeout=30.0,
        limits=httpx.Limits(max_connections=4, max_keepalive_connections=2),
    ) as client:
        yield AppContext(client=client)


# ── Server ──────────────────────────────────────────────────────────

mcp = FastMCP(
    "Pixelated's Tibia Editor",
    instructions=(
        "You are connected to Pixelated's Tibia Editor (PTE), a GPU-accelerated map "
        "editor for Open Tibia servers. The editor works with OTBM maps on a 2D grid "
        "where each position (x, y, z) can hold one ground tile and a stack of items.\n\n"
        "Coordinate system:\n"
        "  - X/Y: 0–65535 (world coordinates, most maps use a subset)\n"
        "  - Z: 0–15 where 7 is ground level, 0–6 are sky floors, 8–15 are underground\n\n"
        "Items are identified by numeric IDs. Use search_appearances to find items by name.\n"
        "All mutations create undo entries — use undo/redo freely.\n"
        "For large edits, prefer batch_operations for atomicity and progress reporting.\n"
        "Check editor_status first to confirm a map is loaded before editing."
    ),
    lifespan=lifespan,
)


# ── Core helpers ────────────────────────────────────────────────────

class EditorError(Exception):
    """Raised when the editor API returns an error response."""


async def _call(ctx: Context[ServerSession, AppContext], method: str, params: dict[str, Any] | None = None) -> Any:
    """Send a command to the editor and return the data payload.

    Raises EditorError on application-level errors, httpx exceptions on transport errors.
    """
    client = ctx.request_context.lifespan_context.client
    try:
        resp = await client.post("/", json={"method": method, "params": params or {}})
        resp.raise_for_status()
    except httpx.ConnectError:
        raise EditorError(
            "Cannot connect to editor at localhost:9982. "
            "Make sure PTE is running with a map open."
        )
    except httpx.TimeoutException:
        raise EditorError("Editor did not respond within 30 seconds.")

    body = resp.json()
    if not body.get("ok"):
        raise EditorError(body.get("error", "Unknown editor error"))
    return body.get("data")


async def _safe_call(ctx: Context[ServerSession, AppContext], method: str, params: dict[str, Any] | None = None) -> tuple[bool, Any]:
    """Like _call but returns (success, data_or_error) instead of raising."""
    try:
        data = await _call(ctx, method, params)
        return True, data
    except (EditorError, httpx.HTTPError) as exc:
        return False, str(exc)


# ── Resources ───────────────────────────────────────────────────────

@mcp.resource("editor://status")
async def resource_editor_status(ctx: Context[ServerSession, AppContext]) -> str:
    """Live editor status: mode, camera, tool, map state."""
    try:
        data = await _call(ctx, "status")
        return json.dumps(data, indent=2)
    except EditorError as exc:
        return json.dumps({"error": str(exc)})


@mcp.resource("map://metadata")
async def resource_map_metadata(ctx: Context[ServerSession, AppContext]) -> str:
    """Map metadata: dimensions, description, file references, counts."""
    try:
        data = await _call(ctx, "get_metadata")
        return json.dumps(data, indent=2)
    except EditorError as exc:
        return json.dumps({"error": str(exc)})


@mcp.resource("map://stats")
async def resource_map_stats(ctx: Context[ServerSession, AppContext]) -> str:
    """Map statistics: tile/chunk counts, Z-level distribution."""
    try:
        data = await _call(ctx, "map_stats")
        return json.dumps(data, indent=2)
    except EditorError as exc:
        return json.dumps({"error": str(exc)})


# ── Prompts ─────────────────────────────────────────────────────────

@mcp.prompt()
def build_structure(
    x: int,
    y: int,
    z: int = 7,
    width: int = 10,
    height: int = 10,
    description: str = "a simple house",
) -> str:
    """Guide for building a structure at the given coordinates."""
    return (
        f"Build {description} at position ({x}, {y}, z={z}) with dimensions {width}x{height}.\n\n"
        "Steps:\n"
        "1. Use editor_status to confirm a map is loaded\n"
        "2. Use search_appearances to find appropriate ground, wall, and floor item IDs\n"
        "3. Use fill_area to lay the floor\n"
        "4. Use batch_operations to place walls around the perimeter\n"
        "5. Use add_item to place doors, windows, and furniture\n"
        "6. Use move_camera to center the view on the result\n\n"
        "Tips:\n"
        "- Common ground IDs: grass=4526, stone=459, wood=405\n"
        "- Always check search_appearances for exact IDs in this asset set\n"
        "- Place walls as items on top of ground tiles\n"
        "- Use undo if anything looks wrong"
    )


@mcp.prompt()
def analyze_area(
    x1: int,
    y1: int,
    x2: int,
    y2: int,
    z: int = 7,
) -> str:
    """Guide for analyzing the contents of a map area."""
    return (
        f"Analyze the area from ({x1}, {y1}) to ({x2}, {y2}) at z={z}.\n\n"
        "Steps:\n"
        "1. Use get_tiles_in_area to retrieve all tiles in the region\n"
        "2. Summarize what ground types are present (group by ground ID)\n"
        "3. List notable items (doors, chests, teleports, signs)\n"
        "4. Note any tiles with house_id set\n"
        "5. Report empty positions (gaps in coverage)\n"
        "6. Use search_appearances to look up names of unknown item IDs"
    )


@mcp.prompt()
def replace_terrain(
    find_name: str,
    replace_name: str,
    z: int = 7,
) -> str:
    """Guide for finding and replacing terrain/items across the map."""
    return (
        f"Replace all '{find_name}' with '{replace_name}' on z={z}.\n\n"
        "Steps:\n"
        f"1. Use search_appearances with query='{find_name}' to find the source item ID\n"
        f"2. Use search_appearances with query='{replace_name}' to find the target item ID\n"
        "3. Use replace_item with the found IDs, restricted to z-level\n"
        "4. Report the number of replacements made\n"
        "5. Remind the user they can undo if the result is wrong"
    )


# ── Tools — Read-only ───────────────────────────────────────────────

@mcp.tool()
async def editor_status(ctx: Context[ServerSession, AppContext]) -> EditorStatus:
    """Get current editor state: mode, camera position, active tool, map info, undo availability.

    Call this first to confirm the editor is running and a map is loaded.
    """
    await ctx.info("Querying editor status...")
    data = await _call(ctx, "status")
    return EditorStatus(
        mode=data["mode"],
        assets_ready=data["assetsReady"],
        map_loaded=data["mapLoaded"],
        tile_count=data["tileCount"],
        camera_x=data["cameraX"],
        camera_y=data["cameraY"],
        camera_z=data["cameraZ"],
        zoom=data["zoom"],
        active_tool=data["activeTool"],
        selected_item_id=data["selectedItemId"],
        can_undo=data["canUndo"],
        can_redo=data["canRedo"],
    )


@mcp.tool()
async def get_tile(
    ctx: Context[ServerSession, AppContext],
    x: int = Field(description="X coordinate (0–65535)"),
    y: int = Field(description="Y coordinate (0–65535)"),
    z: int = Field(description="Z level (0–15, 7=ground)"),
) -> TileData | None:
    """Get tile data at a specific position. Returns null if no tile exists there."""
    data = await _call(ctx, "get_tile", {"x": x, "y": y, "z": z})
    if data is None:
        return None
    return TileData(**data)


@mcp.tool()
async def get_tiles_in_area(
    ctx: Context[ServerSession, AppContext],
    x1: int = Field(description="Start X"),
    y1: int = Field(description="Start Y"),
    x2: int = Field(description="End X"),
    y2: int = Field(description="End Y"),
    z: int = Field(description="Z level"),
) -> dict[str, Any]:
    """Get all tiles in a rectangular area. Returns {tiles: [...], count: N}."""
    width = abs(x2 - x1) + 1
    height = abs(y2 - y1) + 1
    area = width * height
    if area > AREA_TILE_LIMIT:
        raise EditorError(f"Area too large ({area} tiles, max {AREA_TILE_LIMIT})")

    await ctx.info(f"Reading {width}x{height} area ({area} positions)...")
    await ctx.report_progress(progress=0, total=1.0, message="Fetching tiles...")

    data = await _call(ctx, "get_tiles_in_area", {
        "x1": x1, "y1": y1, "x2": x2, "y2": y2, "z": z,
    })

    count = data.get("count", 0)
    await ctx.report_progress(progress=1.0, total=1.0, message=f"Retrieved {count} tiles")
    return data


@mcp.tool()
async def get_metadata(ctx: Context[ServerSession, AppContext]) -> MapMetadata:
    """Get map metadata: dimensions, description, spawn/house files, tile/town/waypoint counts."""
    data = await _call(ctx, "get_metadata")
    return MapMetadata(**data)


@mcp.tool()
async def map_stats(ctx: Context[ServerSession, AppContext]) -> MapStats:
    """Get map statistics: tile count, chunk count, Z-level distribution."""
    data = await _call(ctx, "map_stats")
    return MapStats(**data)


@mcp.tool()
async def search_appearances(
    ctx: Context[ServerSession, AppContext],
    query: str = Field(description="Item name (partial match) or exact numeric item ID"),
    limit: int = Field(default=50, description="Max results (1–200)"),
) -> dict[str, Any]:
    """Search the item catalog by name or ID. Returns {results: [{id, name}], total: N}."""
    limit = max(1, min(limit, 200))
    await ctx.info(f"Searching appearances for '{query}'...")
    data = await _call(ctx, "search_appearances", {"query": query, "limit": limit})
    return data


# ── Tools — Mutations ───────────────────────────────────────────────

@mcp.tool()
async def set_tile(
    ctx: Context[ServerSession, AppContext],
    x: int = Field(description="X coordinate"),
    y: int = Field(description="Y coordinate"),
    z: int = Field(description="Z level"),
    ground_id: int | None = Field(default=None, description="Ground item ID"),
    items: list[dict[str, Any]] | None = Field(default=None, description="Items [{id, action_id?, unique_id?}]"),
    flags: int | None = Field(default=None, description="Tile flags bitfield"),
) -> dict[str, bool]:
    """Set or overwrite a tile at (x, y, z). Creates undo entry."""
    params: dict[str, Any] = {"x": x, "y": y, "z": z}
    if ground_id is not None:
        params["ground_id"] = ground_id
    if items is not None:
        params["items"] = items
    if flags is not None:
        params["flags"] = flags

    await ctx.info(f"Setting tile at ({x}, {y}, {z})")
    data = await _call(ctx, "set_tile", params)
    return data


@mcp.tool()
async def remove_tile(
    ctx: Context[ServerSession, AppContext],
    x: int = Field(description="X coordinate"),
    y: int = Field(description="Y coordinate"),
    z: int = Field(description="Z level"),
) -> dict[str, bool]:
    """Remove a tile completely at (x, y, z). Creates undo entry."""
    await ctx.info(f"Removing tile at ({x}, {y}, {z})")
    data = await _call(ctx, "remove_tile", {"x": x, "y": y, "z": z})
    return data


@mcp.tool()
async def add_item(
    ctx: Context[ServerSession, AppContext],
    x: int = Field(description="X coordinate"),
    y: int = Field(description="Y coordinate"),
    z: int = Field(description="Z level"),
    item_id: int = Field(description="Item ID to place"),
    action_id: int | None = Field(default=None, description="Action ID (for scripts)"),
    unique_id: int | None = Field(default=None, description="Unique ID"),
) -> dict[str, bool]:
    """Add an item to an existing tile's item stack. Creates undo entry."""
    params: dict[str, Any] = {"x": x, "y": y, "z": z, "item_id": item_id}
    if action_id is not None:
        params["action_id"] = action_id
    if unique_id is not None:
        params["unique_id"] = unique_id

    await ctx.info(f"Adding item {item_id} at ({x}, {y}, {z})")
    data = await _call(ctx, "add_item", params)
    return data


@mcp.tool()
async def fill_area(
    ctx: Context[ServerSession, AppContext],
    x1: int = Field(description="Start X"),
    y1: int = Field(description="Start Y"),
    x2: int = Field(description="End X"),
    y2: int = Field(description="End Y"),
    z: int = Field(description="Z level"),
    item_id: int = Field(description="Ground item ID to fill with"),
) -> dict[str, int]:
    """Fill a rectangular area with a ground item. Max 10,000 tiles. Creates undo entry."""
    width = abs(x2 - x1) + 1
    height = abs(y2 - y1) + 1
    area = width * height

    if area > AREA_TILE_LIMIT:
        raise EditorError(f"Area too large ({area} tiles, max {AREA_TILE_LIMIT})")

    await ctx.info(f"Filling {width}x{height} area ({area} tiles) with item {item_id}")
    await ctx.report_progress(progress=0, total=1.0, message=f"Filling {area} tiles...")

    data = await _call(ctx, "fill_area", {
        "x1": x1, "y1": y1, "x2": x2, "y2": y2, "z": z, "item_id": item_id,
    })

    count = data.get("count", 0)
    await ctx.report_progress(progress=1.0, total=1.0, message=f"Filled {count} tiles")
    return data


@mcp.tool()
async def replace_item(
    ctx: Context[ServerSession, AppContext],
    find_id: int = Field(description="Item ID to find"),
    replace_id: int = Field(description="Item ID to replace with"),
    z: int | None = Field(default=None, description="Restrict to this Z-level (null = all levels)"),
) -> dict[str, int]:
    """Find and replace all instances of an item ID across the map. Creates undo entry."""
    params: dict[str, Any] = {"find_id": find_id, "replace_id": replace_id}
    if z is not None:
        params["z"] = z

    scope = f"z={z}" if z is not None else "all Z-levels"
    await ctx.info(f"Replacing item {find_id} → {replace_id} on {scope}")
    await ctx.report_progress(progress=0, total=1.0, message="Scanning map...")

    data = await _call(ctx, "replace_item", params)

    count = data.get("count", 0)
    await ctx.report_progress(progress=1.0, total=1.0, message=f"Replaced {count} instances")
    await ctx.info(f"Replaced {count} instances of item {find_id}")
    return data


@mcp.tool()
async def select_item(
    ctx: Context[ServerSession, AppContext],
    id: int = Field(description="Item ID to select as active brush"),
) -> dict[str, int]:
    """Set an item ID as the active brush in the editor UI."""
    data = await _call(ctx, "select_item", {"id": id})
    return data


@mcp.tool()
async def move_camera(
    ctx: Context[ServerSession, AppContext],
    x: float = Field(description="Target X world coordinate"),
    y: float = Field(description="Target Y world coordinate"),
    z: int | None = Field(default=None, description="Target Z-level (null = keep current)"),
) -> dict[str, bool]:
    """Move the editor camera to a world position."""
    params: dict[str, Any] = {"x": x, "y": y}
    if z is not None:
        params["z"] = z
    data = await _call(ctx, "move_camera", params)
    return data


@mcp.tool()
async def undo(ctx: Context[ServerSession, AppContext]) -> dict[str, bool]:
    """Undo the last edit action."""
    data = await _call(ctx, "undo")
    await ctx.info("Undo applied")
    return data


@mcp.tool()
async def redo(ctx: Context[ServerSession, AppContext]) -> dict[str, bool]:
    """Redo the last undone action."""
    data = await _call(ctx, "redo")
    await ctx.info("Redo applied")
    return data


# ── Tools — Batch operations ────────────────────────────────────────

@mcp.tool()
async def batch_operations(
    ctx: Context[ServerSession, AppContext],
    operations: list[dict[str, Any]] = Field(
        description=(
            "Array of operations to execute sequentially. Each operation is: "
            '{"method": "<api_method>", "params": {<method_params>}}. '
            "Supported methods: set_tile, remove_tile, add_item, fill_area, "
            "replace_item, select_item, move_camera."
        ),
    ),
) -> BatchResult:
    """Execute multiple map-editing operations in sequence with per-step progress.

    Each operation creates its own undo entry. If an operation fails, subsequent
    operations still execute (partial failure). Use undo to roll back.

    Example:
        operations = [
            {"method": "set_tile", "params": {"x": 100, "y": 200, "z": 7, "ground_id": 4526}},
            {"method": "add_item", "params": {"x": 100, "y": 200, "z": 7, "item_id": 2160}},
        ]
    """
    ALLOWED_METHODS = {
        "set_tile", "remove_tile", "add_item", "fill_area",
        "replace_item", "select_item", "move_camera",
    }

    if len(operations) > BATCH_MAX:
        raise EditorError(f"Too many operations ({len(operations)}, max {BATCH_MAX})")

    total = len(operations)
    succeeded = 0
    failed = 0
    results: list[dict[str, Any]] = []

    await ctx.info(f"Starting batch of {total} operations")

    for i, op in enumerate(operations):
        method = op.get("method", "")
        params = op.get("params", {})

        if method not in ALLOWED_METHODS:
            results.append({"ok": False, "error": f"Disallowed method: {method}"})
            failed += 1
            continue

        await ctx.report_progress(
            progress=i,
            total=total,
            message=f"[{i + 1}/{total}] {method}",
        )

        ok, data = await _safe_call(ctx, method, params)
        if ok:
            results.append({"ok": True, "data": data})
            succeeded += 1
        else:
            results.append({"ok": False, "error": data})
            failed += 1
            await ctx.warning(f"Operation {i + 1} ({method}) failed: {data}")

    await ctx.report_progress(
        progress=total,
        total=total,
        message=f"Complete: {succeeded} succeeded, {failed} failed",
    )

    await ctx.info(f"Batch complete: {succeeded}/{total} succeeded")

    return BatchResult(
        total=total,
        succeeded=succeeded,
        failed=failed,
        results=results,
    )


# ── Entrypoint ──────────────────────────────────────────────────────

if __name__ == "__main__":
    mcp.run()
