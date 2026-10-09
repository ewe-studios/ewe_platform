# Session Persistence Behavior

## pi-mono: JSONL Sessions

### Format

Sessions are stored as JSONL files where each line is a JSON object:
- User prompts
- Assistant messages
- Tool execution events
- System events (model changes, compaction summaries)

### Operations

| Operation | Behavior |
|-----------|----------|
| Create | New session with unique ID |
| Continue | Load last session and append new messages |
| Resume | Load specific session by ID |
| Fork | Create new session branching from specific message |

### Session Directory

Sessions stored in a configurable directory:
- Default: `~/.pi/agent/sessions/`
- Override: `--session-dir` flag

### Export

Sessions can be exported to HTML with tool execution rendering and syntax highlighting.

## hermes-agent: JSON + SQLite

### Dual Storage

hermes-agent uses both JSON and SQLite for session persistence:
- **JSON**: Full session data for export and debugging
- **SQLite**: Structured queries for session search, history, and analytics

### Session Database

SQLite stores:
- Session metadata (ID, model, provider, timestamps)
- Message count and token usage
- Tool execution history
- Compaction summaries

### Background Persistence

After each turn:
1. Background memory/skill review spawned in thread
2. Session persisted to JSON + SQLite
3. Context compression check for next turn

## Comparison

| Aspect | pi-mono | hermes-agent |
|--------|---------|-------------|
| Format | JSONL | JSON + SQLite |
| Query capability | Linear scan | SQL queries |
| Session search | File-based | SQLite-powered |
| Export | HTML | JSON |
| Compaction storage | In session file | Compaction entries |
| Background persistence | No | Yes (thread) |

## Key Patterns for foundation_ai

1. **JSONL** is simple, append-only, easy to resume/fork
2. **SQLite** enables structured queries and analytics
3. **Hybrid approach** -- JSONL for simplicity + SQLite for queries
4. **Session operations** (create, continue, resume, fork) are universal
5. **Background persistence** avoids blocking the agent loop
6. **HTML export** for human-readable session review
