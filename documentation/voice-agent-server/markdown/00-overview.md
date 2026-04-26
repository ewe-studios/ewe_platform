# Voice Agent Server -- Overview

## What It Is

Voice Agent Server is a small Express.js REST API server that manages voice AI assistants and their associated phone numbers. It sits between a local JSON file database and the Vapi AI platform, providing CRUD operations that keep both in sync.

Instead of calling Vapi's API directly, applications talk to this server. The server handles:

- Creating voice assistants (with configurable personality, voice, and AI model)
- Provisioning phone numbers (area code 207 by default)
- Linking phone numbers to assistants
- Persisting all records locally in a `data.json` file

## Purpose

Vapi's API is powerful but stateless. This server adds a persistence and management layer so you can:

1. **Track created assistants and phone numbers** in a local database with metadata
2. **Update assistants** (personality, voice, model config) through a single endpoint
3. **Manage phone number-to-assistant routing** -- link any phone number to any assistant
4. **Keep a local record** that survives Vapi API downtime or account changes

It's intentionally simple. No auth layer, no complex routing, no caching. Just CRUD with Vapi sync.

## Architecture

```
┌──────────────┐     HTTP      ┌─────────────────────┐     API      ┌──────────────┐
│   Client     │ ────────────> │  Voice Agent Server │ ──────────>  │   Vapi API   │
│  (any app)   │ <──────────── │  (Express.js :3000) │ <──────────  │  (Cloud)     │
└──────────────┘               │                     │              └──────────────┘
                               │  ┌───────────────┐  │
                               │  │  data.json    │  │
                               │  │  (local DB)   │  │
                               │  └───────────────┘  │
                               └─────────────────────┘
```

### Two Resource Types

**Assistants** represent a voice AI configuration:

- Name and greeting message
- System prompt (defines personality)
- AI model (OpenAI `chatgpt-4o-latest`)
- Voice provider and voice ID (11Labs)
- Max call duration, end call message
- Link to a phone number

**Phone Numbers** represent provisioned telephone lines:

- Name and actual phone number
- Area code (207 -- Maine, USA)
- Link to an assistant (routes incoming calls)

### How Writes Work

Every POST/PATCH/DELETE follows the same pattern:

1. Perform the operation on Vapi's API first
2. If Vapi succeeds, update the local `data.json`
3. Return the local record (with Vapi IDs attached)

This means the local DB is the source of truth for IDs and metadata, while Vapi holds the actual voice assistant and phone number configuration.

### How Reads Work

- **List** (`GET /assistants`, `GET /phone-numbers`): Returns only local records (fast, no Vapi calls)
- **Get by ID** (`GET /assistants/:id`, `GET /phone-numbers/:id`): Fetches the local record, then enriches it with live data from Vapi

## Default Assistant Personality

When creating an assistant without a custom system prompt, the server uses this default:

> You are {name}, a real human personal assistant. Never call yourself an AI, bot, or virtual assistant. Speak casually, warmly, and with a sense of humor. Be curious and conversational -- ask natural follow-up questions and react like a friend who genuinely cares. Keep answers practical and concise, but not terse; add a light joke or playful remark when it fits. Avoid canned phrases and corporate tone. If you do not know something, say so and think it through out loud. Your goal: help, collaborate, and make the conversation feel like it is with a real person.

## Project Structure

```
voice-agent-server/
├── src/
│   ├── index.ts          # Express app, all route handlers
│   └── lib/
│       ├── db.ts         # JSON file database (read/write)
│       └── vapi.ts       # Vapi SDK client initialization
├── data.json             # Local database (created on first write)
├── package.json          # Dependencies and scripts
├── tsconfig.json         # TypeScript config
├── Dockerfile            # Multi-stage Docker build
└── fly.toml              # Fly.io deployment config
```

Only three source files. The entire server is ~350 lines of TypeScript.
