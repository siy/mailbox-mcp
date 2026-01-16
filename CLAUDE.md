# mailbox-mcp

A minimalistic MCP (Model Context Protocol) server for agent-to-agent communication. Enables multiple AI agents to coordinate via message queues and shared context, with SQLite persistence.

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

Simple MCP server for agent-to-agent communication:
- SQLite database stored in user's data directory
- HTTP server bound to 127.0.0.1 only
- **Context tools**: Global and project-scoped key-value storage (`context_set`, `context_get`, `context_delete`, `context_list`)
- **Message tools**: Per-agent queues with optional request/response linking (`send_message`, `receive_messages`, `peek_messages`, `delete_message`)
- Project addressing uses GitHub-style IDs (e.g., `owner/repo`)

## Code Style

- Minimize dependencies
- Prefer SQLite built-in functions over Rust libraries where possible
- Keep it simple - this is a local utility, not a production service
