# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] - 2026-01-16

### Changed
- **BREAKING**: Replaced message queue with pub-sub model
  - Removed: `send_message`, `receive_messages`, `peek_messages`, `delete_message`
  - Added: `publish`, `receive`, `peek`, `list_topics`
- Messages now addressed by topic instead of `to_agent`
- Per-consumer read tracking via `(topic, message_id, consumer)` tuples
- At-least-once delivery: messages persist until TTL-based cleanup

### Added
- `publish` - Publish message to any topic
- `receive` - Get unread messages, mark as read for this consumer
- `peek` - View messages without marking read
- `list_topics` - List all topics with messages

## [0.1.1] - 2026-01-15

### Changed
- Removed `--upgrade` flag (use install scripts to upgrade)
- Removed `directories` crate dependency, using hardcoded platform paths
- Simplified signal handling with graceful error handling (no panics)
- Consolidated database modules into single `db.rs`
- Simplified JSON response helpers using `json!` macro

### Fixed
- Documentation: `created_at` field now correctly shows ISO 8601 format
- Documentation: `from_agent` parameter default ("anonymous") now documented
- Whitespace handling: keys and `from_agent` are now trimmed consistently
- Tool descriptions now mention that limit values above 500 are silently capped

## [0.1.0] - 2025-01-08

### Added
- Initial release
- Context operations: `context_set`, `context_get`, `context_delete`, `context_list`
- Message operations: `send_message`, `receive_messages`, `peek_messages`, `delete_message`
- SQLite persistence for messages and context
- HTTP transport via Streamable HTTP MCP
- Installation scripts for Linux, macOS, and Windows
