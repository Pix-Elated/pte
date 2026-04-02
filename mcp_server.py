#!/usr/bin/env python3
"""
Pixelated's Tibia Editor — MCP Server

Bridges between MCP clients (Claude, VS Code, etc.) and the native map editor
via its embedded HTTP API on localhost:9982.

Usage:
    pip install mcp httpx
    python mcp_server.py

Or add to your MCP config:
    {
        "mcpServers": {
            "tibia-editor": {
                "command": "python",
                "args": ["path/to/mcp_server.py"]
            }
        }
    }
"""

import json
from typing import Any

try:
    import httpx
except ImportError:
    httpx = None

try:
    from mcp.server import Server
    from mcp.server.stdio import stdio_server
    from mcp.types import Tool, TextContent
except ImportError:
    Server = None
    stdio_server = None
    Tool = None
    TextContent = None

EDITOR_URL = "http://localhost:9982"


async def call_editor(method: str, params: dict[str, Any] | None = None) -> dict[str, Any]:
    """Send a JSON request to the editor's embedded HTTP API."""
    if httpx is None:
        raise RuntimeError("httpx not installed. Run: pip install httpx")

    async with httpx.AsyncClient(timeout=12.0) as client:
        resp = await client.post(
            EDITOR_URL,
            json={"method": method, "params": params or {}},
        )
        resp.raise_for_status()
        return resp.json()


# ─── MCP Tool Definitions ──────────────────────────────────────────

TOOLS = [
    {
        "name": "editor_status",
        "description": "Get editor status: mode, camera position, tool, loaded map info, undo state.",
        "inputSchema": {"type": "object", "properties": {}, "required": []},
    },
    {
        "name": "get_tile",
        "description": "Get tile data at (x, y, z). Returns ground ID, items, flags, house_id.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "x": {"type": "integer", "description": "X coordinate (0-65535)"},
                "y": {"type": "integer", "description": "Y coordinate (0-65535)"},
                "z": {"type": "integer", "description": "Z level (0-15, 7 = ground)"},
            },
            "required": ["x", "y", "z"],
        },
    },
    {
        "name": "set_tile",
        "description": "Set a tile at (x, y, z). Optionally set ground, items, and flags.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "x": {"type": "integer"},
                "y": {"type": "integer"},
                "z": {"type": "integer"},
                "ground_id": {"type": "integer", "description": "Ground item ID"},
                "items": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "id": {"type": "integer"},
                            "action_id": {"type": "integer"},
                            "unique_id": {"type": "integer"},
                        },
                        "required": ["id"],
                    },
                },
                "flags": {"type": "integer", "description": "Tile flags bitfield"},
            },
            "required": ["x", "y", "z"],
        },
    },
    {
        "name": "remove_tile",
        "description": "Remove a tile completely at (x, y, z).",
        "inputSchema": {
            "type": "object",
            "properties": {
                "x": {"type": "integer"},
                "y": {"type": "integer"},
                "z": {"type": "integer"},
            },
            "required": ["x", "y", "z"],
        },
    },
    {
        "name": "add_item",
        "description": "Add an item to an existing tile's item stack.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "x": {"type": "integer"},
                "y": {"type": "integer"},
                "z": {"type": "integer"},
                "item_id": {"type": "integer", "description": "Item ID to place"},
                "action_id": {"type": "integer"},
                "unique_id": {"type": "integer"},
            },
            "required": ["x", "y", "z", "item_id"],
        },
    },
    {
        "name": "fill_area",
        "description": "Fill a rectangular area with a ground item. Max 10000 tiles.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "x1": {"type": "integer"},
                "y1": {"type": "integer"},
                "x2": {"type": "integer"},
                "y2": {"type": "integer"},
                "z": {"type": "integer"},
                "item_id": {"type": "integer", "description": "Ground item ID"},
            },
            "required": ["x1", "y1", "x2", "y2", "z", "item_id"],
        },
    },
    {
        "name": "get_tiles_in_area",
        "description": "Get all tiles in a rectangular area at a z-level.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "x1": {"type": "integer"},
                "y1": {"type": "integer"},
                "x2": {"type": "integer"},
                "y2": {"type": "integer"},
                "z": {"type": "integer"},
            },
            "required": ["x1", "y1", "x2", "y2", "z"],
        },
    },
    {
        "name": "replace_item",
        "description": "Find and replace all instances of an item ID. Optionally restrict to a z-level.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "find_id": {"type": "integer"},
                "replace_id": {"type": "integer"},
                "z": {"type": "integer", "description": "Optional: restrict to z-level"},
            },
            "required": ["find_id", "replace_id"],
        },
    },
    {
        "name": "search_appearances",
        "description": "Search item appearances by name or ID. Returns matching items.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "query": {"type": "string", "description": "Search text or item ID"},
                "limit": {"type": "integer", "description": "Max results (default 50)"},
            },
            "required": ["query"],
        },
    },
    {
        "name": "select_item",
        "description": "Select an item ID as the active brush in the editor.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "id": {"type": "integer"},
            },
            "required": ["id"],
        },
    },
    {
        "name": "move_camera",
        "description": "Move the editor camera to a world position.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "x": {"type": "number"},
                "y": {"type": "number"},
                "z": {"type": "integer"},
            },
            "required": ["x", "y"],
        },
    },
    {
        "name": "get_metadata",
        "description": "Get map metadata: dimensions, description, files, tile/town/waypoint counts.",
        "inputSchema": {"type": "object", "properties": {}, "required": []},
    },
    {
        "name": "map_stats",
        "description": "Get map statistics: tile count, chunk count, z-level distribution.",
        "inputSchema": {"type": "object", "properties": {}, "required": []},
    },
    {
        "name": "undo",
        "description": "Undo the last edit action.",
        "inputSchema": {"type": "object", "properties": {}, "required": []},
    },
    {
        "name": "redo",
        "description": "Redo the last undone action.",
        "inputSchema": {"type": "object", "properties": {}, "required": []},
    },
]


async def run_server() -> None:
    """Run the MCP server."""
    if Server is None:
        print("Error: mcp package not installed. Run: pip install mcp")
        return

    server = Server("tibia-map-editor")

    @server.list_tools()
    async def list_tools() -> list:
        return [
            Tool(
                name=t["name"],
                description=t["description"],
                inputSchema=t["inputSchema"],
            )
            for t in TOOLS
        ]

    @server.call_tool()
    async def call_tool(name: str, arguments: dict) -> list:
        try:
            result = await call_editor(name, arguments)
            return [TextContent(type="text", text=json.dumps(result, indent=2))]
        except httpx.ConnectError:
            return [TextContent(
                type="text",
                text="Error: Cannot connect to editor. Is it running? (expected at localhost:9982)",
            )]
        except Exception as e:
            return [TextContent(type="text", text=f"Error: {e}")]

    async with stdio_server() as (read_stream, write_stream):
        await server.run(read_stream, write_stream, server.create_initialization_options())


if __name__ == "__main__":
    import asyncio
    asyncio.run(run_server())
