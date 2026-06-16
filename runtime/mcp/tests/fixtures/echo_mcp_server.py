#!/usr/bin/env python3
"""
Minimal MCP stdio server for integration testing.

Implements the MCP protocol over stdin/stdout with:
- initialize / notifications/initialized
- tools/list (returns a single "echo" tool)
- tools/call (echoes back the input message)

Usage:
    python3 echo_mcp_server.py

The server reads JSON-RPC messages from stdin (newline-delimited)
and writes responses to stdout.
"""

import json
import sys


def send_response(response):
    """Send a JSON-RPC response to stdout."""
    sys.stdout.write(json.dumps(response) + "\n")
    sys.stdout.flush()


def send_notification(method, params=None):
    """Send a JSON-RPC notification to stdout."""
    notification = {"jsonrpc": "2.0", "method": method}
    if params:
        notification["params"] = params
    sys.stdout.write(json.dumps(notification) + "\n")
    sys.stdout.flush()


def handle_initialize(request):
    """Handle initialize request."""
    send_response({
        "jsonrpc": "2.0",
        "id": request.get("id"),
        "result": {
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {"listChanged": True}
            },
            "serverInfo": {
                "name": "echo-mcp-server",
                "version": "1.0.0"
            }
        }
    })


def handle_initialized(request):
    """Handle notifications/initialized notification."""
    # No response needed for notifications
    pass


def handle_tools_list(request):
    """Handle tools/list request."""
    send_response({
        "jsonrpc": "2.0",
        "id": request.get("id"),
        "result": {
            "tools": [
                {
                    "name": "echo",
                    "description": "Echoes back the input message",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "message": {
                                "type": "string",
                                "description": "The message to echo"
                            }
                        },
                        "required": ["message"]
                    }
                }
            ]
        }
    })


def handle_tools_call(request):
    """Handle tools/call request."""
    params = request.get("params", {})
    tool_name = params.get("name", "")
    arguments = params.get("arguments", {})

    if tool_name == "echo":
        message = arguments.get("message", "no message")
        send_response({
            "jsonrpc": "2.0",
            "id": request.get("id"),
            "result": {
                "content": [
                    {
                        "type": "text",
                        "text": f"Echo: {message}"
                    }
                ],
                "isError": False
            }
        })
    else:
        send_response({
            "jsonrpc": "2.0",
            "id": request.get("id"),
            "error": {
                "code": -32601,
                "message": f"Method not found: {tool_name}"
            }
        })


def main():
    """Main event loop - read JSON-RPC messages from stdin."""
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue

        try:
            request = json.loads(line)
        except json.JSONDecodeError:
            send_response({
                "jsonrpc": "2.0",
                "id": None,
                "error": {
                    "code": -32700,
                    "message": "Parse error"
                }
            })
            continue

        method = request.get("method", "")

        if method == "initialize":
            handle_initialize(request)
        elif method == "notifications/initialized":
            handle_initialized(request)
        elif method == "tools/list":
            handle_tools_list(request)
        elif method == "tools/call":
            handle_tools_call(request)
        else:
            send_response({
                "jsonrpc": "2.0",
                "id": request.get("id"),
                "error": {
                    "code": -32601,
                    "message": f"Method not found: {method}"
                }
            })


if __name__ == "__main__":
    main()
