---
title: "Pi Extensions -- pi-discord"
---

# pi-discord

**Discord bot integration for Pi with persistent sessions and tool access.**

pi-discord brings Pi into your Discord server. Mention the bot or use slash commands to run Pi with full tool access, persistent sessions, and optional project extensions.

## How It Works

A detached daemon listens for Discord mentions, DMs, and slash commands. Each channel gets its own persistent Pi session, so follow-up questions remember the earlier conversation. When a message comes in, the daemon calls `session.prompt()`, subscribes to the response stream, and live-updates the Discord reply as text streams back.

## Commands

### Discord Side

```
# Mention the bot
@your-bot Summarize the latest PR

# Slash commands
/pi ask text:"What's the status of the CI pipeline?"
```

### Pi Side (Operator)

```
/discord start    -- Start the Discord bot
/discord stop     -- Stop the bot
/discord status   -- Check bot status
/discord setup    -- Initial configuration wizard
```

## Setup

Requires a bot token and application ID from the Discord Developer Portal. Run `/discord setup` in Pi to configure.

## Session Persistence

Each Discord channel gets its own Pi session. The session persists across restarts, so the bot remembers context within a channel thread.

## Package Details

| Property | Value |
|----------|-------|
| Install | `pi install npm:pi-discord` |
| Requires | Discord bot token, application ID |
| Sessions | Per-channel persistent sessions |
| Trigger | Mentions, slash commands, DMs |
