# mailbox-mcp

A minimalistic MCP (Model Context Protocol) server for agent-to-agent communication via pub-sub. Enables multiple AI agents to coordinate via topic-based messaging and shared context, with SQLite persistence.

## Critical Context

**This application is STRICTLY LOCAL-ONLY.** It:
- Runs ONLY on localhost (127.0.0.1)
- Is NEVER exposed to the internet
- Is NEVER accessible from other machines
- Has no authentication because it doesn't need any - only local processes can connect

When reviewing or modifying this code:
- Do NOT add authentication, rate limiting, or other network security features - they are unnecessary overhead
- Do NOT flag "security issues" related to network exposure - the app cannot be exposed
- DO focus on correctness, simplicity, and local resource management (disk, memory)

## Architecture

### Design Principles

1. **Topic-based pub-sub** - Messages addressed to topics, not agents. Anyone can read any topic.
2. **Per-consumer read tracking** - Each consumer tracks their own read position independently.
3. **At-least-once delivery** - Messages persist until TTL-based cleanup. Consumers may see messages multiple times if they don't track reads.
4. **No subscription management** - No need to subscribe. Just start reading.

### Data Model

```
┌─────────────────────────────────────────────────────────────┐
│                        messages                              │
├─────────────────────────────────────────────────────────────┤
│ id          │ INTEGER PRIMARY KEY                           │
│ topic       │ TEXT NOT NULL (e.g., "releases/pragmatica")   │
│ from_agent  │ TEXT NOT NULL (sender identifier)             │
│ reference_id│ TEXT (optional, for threading)                │
│ content     │ TEXT NOT NULL (max 1MB)                       │
│ created_at  │ TEXT (ISO 8601)                               │
└─────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│                      read_markers                            │
├─────────────────────────────────────────────────────────────┤
│ topic       │ TEXT NOT NULL                                 │
│ message_id  │ INTEGER NOT NULL                              │
│ consumer    │ TEXT NOT NULL                                 │
│ read_at     │ TEXT (ISO 8601)                               │
│ PRIMARY KEY (topic, message_id, consumer)                   │
└─────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────┐
│                        context                               │
├─────────────────────────────────────────────────────────────┤
│ project_id  │ TEXT (NULL for global)                        │
│ key         │ TEXT NOT NULL                                 │
│ value       │ TEXT NOT NULL (max 64KB)                      │
│ PRIMARY KEY (project_id, key)                               │
└─────────────────────────────────────────────────────────────┘
```

### MCP Tools

**Context operations** (unchanged):
- `context_set`, `context_get`, `context_delete`, `context_list`

**Pub-sub operations**:
- `publish` - Send message to topic
- `receive` - Get unread messages for consumer, mark as read
- `peek` - View messages without marking read
- `list_topics` - List all topics with messages

### Typical Flow

```
Agent A: publish(topic="releases/lib", content="v1.0.0 released", from_agent="build-bot")
         → returns message_id

Agent B: receive(topic="releases/lib", consumer="agent-b")
         → returns message, marks read for agent-b

Agent C: receive(topic="releases/lib", consumer="agent-c")
         → returns same message, marks read for agent-c

Agent B: receive(topic="releases/lib", consumer="agent-b")
         → returns empty (already read)
```

## Code Style

- Minimize dependencies
- Prefer SQLite built-in functions over Rust libraries where possible
- Keep it simple - this is a local utility, not a production service
- All pub-sub operations are atomic within SQLite transactions
