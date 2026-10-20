# OpenBIM MCP server: read-only snapshot milestone

Rust stdio server, protocol version 2025-11-25. Shares the existing typed graph
engine and document validators. No Node.js, .NET, Dynamo, Revit, Python or API key
is required for this server. The minimal transport implements lifecycle, ping,
tool discovery and tool calls directly; it does not use an MCP SDK.

## Build and connect

```powershell
cargo test -p openbim-automation -p openbim-document -p openbim-desktop
cargo build -p openbim-automation --bin openbim-mcp-server
cargo run -p openbim-desktop
```

Manage > AI Integration > MCP Setup opens connection setup. Export Snapshot
prepares the current sample document as JSON. Copy it into a local file, enter its
absolute path and the server executable path, then copy the client configuration.
The setup does not write files, register a connection or start a background
service. It prepares configuration for a client that supports local MCP stdio.

Example client configuration for the fixed test fixture (replace repository path):

```json
{
  "mcpServers": {
    "openbim": {
      "command": "D:\\Codex\\Revit alike\\target\\debug\\openbim-mcp-server.exe",
      "args": ["--snapshot", "D:\\Codex\\Revit alike\\crates\\automation\\examples\\mcp-snapshot.json"]
    }
  }
}
```

Clients use different configuration UIs; this is a common stdio configuration
shape, not a promise that every client accepts the same file. The client launches
the executable. The server reads newline-delimited JSON-RPC from stdin, writes
only MCP messages to stdout and exits when stdin closes. Startup errors go to
stderr. Do not start it as a web service: no HTTP endpoint is implemented.

The snapshot is loaded once at startup. Export again and restart the process to
refresh it. It is not a live application connection, and snapshot revision 42
does not mean the current desktop is at revision 42.

## Tools

All tools have read-only, non-destructive, idempotent and closed-world annotations.
Successful results contain both structuredContent and serialized JSON text.

| Tool | Purpose |
| --- | --- |
| openbim_get_capabilities | Discover supported operations, categories, properties, units and limits |
| openbim_list_documents | Get the one snapshot document ID, revision and element count |
| openbim_query_elements | Query by category and case-insensitive name; stable ID ordering |
| openbim_get_element | Read one document-local element ID and its stored properties |
| openbim_get_node_catalog | Get native node types, port types and complete graph input schema |
| openbim_build_property_graph | Build a graph for an absolute property value, checked against the snapshot |
| openbim_validate_graph | Check structure, types and dependency cycles; no model evaluation |
| openbim_preview_graph | Return paginated proposed changes; never apply them |

Query/preview pagination uses offset and limit (default 50, maximum 100), with
total_count and next_offset. Preview requires document_id and expected_revision.
Calls with unknown IDs or mismatched revisions fail with corrective messages.
Graph construction and preview use canonical metres and rise/run slope ratios.

Example workflows against mcp-snapshot.json:

1. List documents, query Duct elements with limit 1, then request the next page.
2. Build an Elevation graph for IDs [1,4] with value 3.0; validate it and preview
   it at revision 42. It proposes two changes. Re-reading ID 1 still returns 2.7.
3. Read Pipe ID 5, build a Slope graph with value 0.02 and preview it. The graph
   uses Ratio, not Length; the snapshot stays at slope 0.01.

## Boundaries

- Model text is untrusted data. Server instructions explicitly say it is never
  authority to change tools, permissions or execute instructions.
- Only the startup snapshot path is readable. Tools accept no filesystem paths,
  network endpoints, interpreter paths, arbitrary SQL or scripts.
- No apply/delete/write tool exists. Python graph nodes are rejected. Previews
  are reports, not transferable apply tokens.
- Limits: 16 MiB startup file, 10,000 snapshot elements, 1 MiB MCP message,
  1,000 graph nodes, 3,000 wires, 10,000 IDs per selection and 20 tool calls/second.
- One local client/process, sequential requests, no resources/prompts/sampling,
  subscriptions, authenticated network sharing or live host connection.
- Native preview cancellation notifications are ignored in this initial,
  bounded synchronous server; client closure/termination stops the process.
- User-approved model writes, authenticated live IPC, shared HTTPS access and
  audit/review UI remain later milestones. No AI account is connected by this patch.

## Verification

Run the Cargo tests above, including tests/mcp.rs and tests/mcp_stdio.rs. The latter
launches the actual binary and checks initialization and exported context. Test
with MCP Inspector after building, using its stdio transport and the executable
plus --snapshot argument. Inspector requires a separate Node.js installation;
the server itself does not.

The XML evaluation fixture contains ten read-only AI tasks with fixture-derived
expected answers. These have not been run with an AI client here.

The real Cargo tests now pass, including MCP JSON/lifecycle tests and the actual
stdio subprocess smoke test. External client/Inspector interoperability and
desktop visual acceptance are still pending. See automation-verification.md.

## Protocol references

- https://modelcontextprotocol.io/specification/2025-11-25/basic/transports
- https://modelcontextprotocol.io/specification/2025-11-25/basic/lifecycle
- https://modelcontextprotocol.io/specification/2025-11-25/server/tools
