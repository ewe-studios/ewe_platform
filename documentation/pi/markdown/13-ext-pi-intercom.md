---
title: "Pi Extensions -- pi-intercom"
---

# pi-intercom

**Direct 1:1 messaging between Pi sessions on the same machine.**

pi-intercom enables communication between running Pi sessions. Send context, findings, or requests from one session to another -- whether you're driving the conversation or letting agents coordinate autonomously.

## Usage

```
User flow: press Alt+M or run /intercom to pick a session and send a message
```

## Why It Exists

Sometimes you're running multiple Pi sessions -- one researching, one executing, one reviewing. pi-intercom enables:

### User-Driven Orchestration

Send context or findings from your research session to your execution session without manual copy-paste.

### Agent Collaboration

An agent can reach out to another session when it needs help or wants to share results.

### Session Awareness

See what other Pi sessions are running and their current status.

## How It Works

Pi-intercom uses inter-process communication on the local machine. Each Pi session registers itself with a local message broker. Sessions can discover each other, send messages, and receive responses.

## Package Details

| Property | Value |
|----------|-------|
| Install | `pi install npm:pi-intercom` |
| Trigger | Alt+M or `/intercom` |
| Scope | Local machine sessions only |
| Pattern | 1:1 messaging between sessions |
