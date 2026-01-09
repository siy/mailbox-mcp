# mailbox-mcp

A minimalistic MCP (Model Context Protocol) server for agent-to-agent communication, written in Rust.

## Features

- **Message Queues**: Per-agent message queues with request/response linking via reference IDs
- **Shared Context**: Global and project-scoped key-value stores
- **SQLite Persistence**: Messages and context survive server restarts
- **HTTP Transport**: Single server supports multiple concurrent agents
- **Project Addressing**: Uses GitHub-style project IDs (e.g., `owner/repo`)

## Installation

### Quick Install (Recommended)

**Linux/macOS:**
```bash
curl -fsSL https://raw.githubusercontent.com/siy/mailbox-mcp/master/scripts/install.sh | sh
```

**Windows (PowerShell):**
```powershell
iwr -useb https://raw.githubusercontent.com/siy/mailbox-mcp/master/scripts/install.ps1 | iex
```

### From GitHub Releases

Download the appropriate binary for your platform from the [Releases](https://github.com/siy/mailbox-mcp/releases) page.

### From Source

```bash
git clone https://github.com/siy/mailbox-mcp.git
cd mailbox-mcp
cargo build --release
```

The binary will be at `./target/release/mailbox-mcp`.

## Usage

```bash
# Start with default settings (port 3000, localhost only)
mailbox-mcp

# Custom port
mailbox-mcp --port 8080

# Show version
mailbox-mcp --version
```

> **Note:** The server is intentionally hardcoded to bind to `127.0.0.1` (localhost) only. This is a local-only service and should never be exposed to the network.

## MCP Tools

### Context Operations

| Tool | Parameters | Description |
|------|------------|-------------|
| `context_set` | `key`, `value`, `project_id?` | Set a value (omit project_id for global) |
| `context_get` | `key`, `project_id?` | Get a value |
| `context_delete` | `key`, `project_id?` | Delete a value |
| `context_list` | `project_id?` | List all keys |

### Message Operations

| Tool | Parameters | Description |
|------|------------|-------------|
| `send_message` | `project_id`, `to_agent`, `content`, `from_agent?` (default: "anonymous"), `reference_id?` | Send message, returns `message_id` |
| `receive_messages` | `project_id`, `agent_id`, `limit?` | Get and consume messages |
| `peek_messages` | `project_id`, `agent_id`, `limit?` | View without consuming |
| `delete_message` | `message_id` | Delete specific message |

### Message Structure

```json
{
  "id": "123",
  "reference_id": "122",
  "from_agent": "sender",
  "content": "message body",
  "created_at": "2025-01-08T12:00:00Z"
}
```

> **Note:** Message IDs are auto-incrementing integers (as strings). Reference IDs link responses to original requests.

## Configuration

### Claude Code

Add to your Claude Code MCP settings (`~/.claude/settings.json`):

```json
{
  "mcpServers": {
    "mailbox": {
      "type": "url",
      "url": "http://localhost:3000/mcp"
    }
  }
}
```

### Data Storage

- **Linux:** `~/.local/share/mailbox-mcp/mailbox.db`
- **macOS:** `~/Library/Application Support/mailbox-mcp/mailbox.db`
- **Windows:** `%APPDATA%\mailbox-mcp\mailbox.db`

## Example: Agent Communication

**Agent A** sends a request:
```
send_message(
  project_id: "owner/shared-lib",
  to_agent: "library-maintainer",
  from_agent: "feature-developer",
  content: "Need Option::tap() method added to support fluent API"
)
# Returns: {"message_id": "1"}
```

**Agent B** receives and responds:
```
receive_messages(project_id: "owner/shared-lib", agent_id: "library-maintainer")
# Returns messages, including the request above

send_message(
  project_id: "owner/shared-lib",
  to_agent: "feature-developer",
  from_agent: "library-maintainer",
  content: "Added Option::tap() in v1.2.0",
  reference_id: "1"  # Links to original request
)
```

## License

Apache License 2.0 - see [LICENSE](LICENSE)
