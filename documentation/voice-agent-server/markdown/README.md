# Voice Agent Server -- Documentation Index

**Voice Agent Server** is a lightweight Express.js REST API that manages voice AI assistants and phone numbers backed by the Vapi AI platform. It provides a simple management layer between a local JSON database and Vapi's voice infrastructure.

## Documents

| Document | What It Covers |
|----------|---------------|
| [00-overview.md](./00-overview.md) | What it is, purpose, architecture at a glance |
| [01-api.md](./01-api.md) | REST API endpoints: assistants and phone numbers (CRUD) |
| [02-stack.md](./02-stack.md) | Tech stack: Express.js, Vapi SDK, 11Labs, JSON DB |
| [03-deployment.md](./03-deployment.md) | Docker, Fly.io deployment, environment variables |
| [04-development.md](./04-development.md) | Local setup, development workflow |

## Quick Orientation

```
Client (HTTP)
    |
    v
Express.js (port 3000)
    |
    +-- /assistants  -->  Vapi API  +  data.json
    |
    +-- /phone-numbers --> Vapi API +  data.json
```

The server acts as a thin management layer: every write operation syncs to both Vapi's cloud API and a local `data.json` file. Reads merge local records with live Vapi data.

## Source

`/home/darkvoid/Boxxed/@formulas/src.rust/src.llamacpp/src.AIApps/voice-agent-server/`
