# Checkpointing Behavior

## hermes-agent CheckpointManager

### Trigger Points

Checkpoints are created before destructive operations:
- `write_file` -- before overwriting file contents
- `patch` -- before applying file patches
- Destructive terminal commands -- before commands that modify files

### Snapshot Contents

The `CheckpointManager` snapshots the working directory:
- File contents of all tracked files
- Current git state
- Timestamp and description

### Restore Flow

1. User requests checkpoint restore
2. CheckpointManager loads the snapshot
3. Files are restored to their checkpointed state
4. Working directory matches the checkpoint state

### Checkpoint Storage

Checkpoints are stored locally (typically in a `.hermes/` directory or temp directory).

## pi-mono

pi-mono does not have explicit checkpointing. It relies on git for file version tracking and does not snapshot the working directory before operations.

## Key Patterns for foundation_ai

1. **Pre-destructive snapshots** protect against unintended file modifications
2. **Trigger on write/patch/destructive commands** covers the main risk areas
3. **Restore flow** allows undoing agent actions
4. **Git as implicit checkpoint** -- pi-mono relies on git version control
5. **Local storage** for checkpoints -- not cloud-synced
