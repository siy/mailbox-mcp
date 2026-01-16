# mailbox-mcp

A minimalistic MCP server for agent-to-agent communication via pub-sub, written in Rust.

## Features

- **Pub-sub Messaging**: Topic-based messaging with per-consumer read tracking
- **Shared Context**: Global and project-scoped key-value stores
- **SQLite Persistence**: Messages and context survive server restarts
- **At-least-once Delivery**: Messages persist until TTL, consumers track their own read position

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

### From Source

```bash
git clone https://github.com/siy/mailbox-mcp.git
cd mailbox-mcp
cargo build --release
```

## Usage

```bash
mailbox-mcp              # Default port 3000
mailbox-mcp --port 8080  # Custom port
```

## MCP Tools

### Context Operations

| Tool | Parameters | Description |
|------|------------|-------------|
| `context_set` | `key`, `value`, `project_id?` | Set a value (omit project_id for global) |
| `context_get` | `key`, `project_id?` | Get a value |
| `context_delete` | `key`, `project_id?` | Delete a value |
| `context_list` | `project_id?` | List all keys |

### Pub-sub Operations

| Tool | Parameters | Description |
|------|------------|-------------|
| `publish` | `topic`, `content`, `from_agent?`, `reference_id?` | Publish message to topic |
| `receive` | `topic`, `consumer`, `limit?` | Get unread messages, mark as read |
| `peek` | `topic`, `limit?` | View messages without marking read |
| `list_topics` | | List all topics with messages |

### Message Structure

```json
{
  "id": "123",
  "topic": "releases/my-project",
  "from_agent": "build-agent",
  "reference_id": null,
  "content": "Version 1.0.0 released",
  "created_at": "2026-01-15T12:00:00Z"
}
```

## Topic Conventions

Topics are free-form strings. Suggested conventions:

- `mailbox/{agent}` - Direct messaging to specific agent
- `releases/{project}` - Release notifications
- `deps/{project}` - Dependency update notifications
- `rfc/{rfc-id}` - RFC discussion threads

## Example: Release Notification

**Build agent publishes:**
```
publish(
  topic: "releases/pragmatica-lite",
  content: "0.9.12 published to Maven Central",
  from_agent: "build-agent"
)
```

**Dependent project agents receive:**
```
receive(
  topic: "releases/pragmatica-lite",
  consumer: "jbct-cli-agent"
)
# Returns the message, marks it read for this consumer

receive(
  topic: "releases/pragmatica-lite",
  consumer: "aether-agent"
)
# Same message, independent read tracking
```

## Configuration

### Claude Code

Add to `~/.claude/settings.json`:

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

## Design

### Why Pub-sub?

Traditional message queues (agent A → agent B) require knowing recipients upfront. Pub-sub decouples publishers from consumers:

- **Publishers** don't need to know who will read
- **Consumers** can start/stop reading anytime
- **Multiple consumers** can independently track their read position
- **Topics** provide natural namespacing (e.g., `releases/project-name`)

### At-least-once Delivery

Messages persist until cleanup. The `receive` operation:
1. Queries messages NOT in `read_markers` for this consumer
2. Returns messages
3. Inserts read markers atomically

If a consumer crashes after receiving but before processing, it will see the message again on restart (at-least-once semantics).

### Schema

```sql
-- Messages with topic-based addressing
CREATE TABLE messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    topic TEXT NOT NULL,
    from_agent TEXT NOT NULL,
    reference_id TEXT,
    content TEXT NOT NULL,
    created_at TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

-- Per-consumer read tracking
CREATE TABLE read_markers (
    topic TEXT NOT NULL,
    message_id INTEGER NOT NULL,
    consumer TEXT NOT NULL,
    read_at TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    PRIMARY KEY (topic, message_id, consumer)
);
```

### Limits

| Resource | Limit |
|----------|-------|
| Message content | 1 MB |
| Context value | 64 KB |
| Messages per query | 500 |

## License

Apache License 2.0
