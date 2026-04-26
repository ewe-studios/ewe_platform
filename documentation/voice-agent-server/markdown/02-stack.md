# Voice Agent Server -- Technology Stack

## Languages and Runtimes

| Concern | Choice | Version |
|---------|--------|---------|
| Language | TypeScript (strict mode) | 5.6+ |
| Runtime | Node.js | 22.x |
| Module system | CommonJS | -- |

## Dependencies

### Runtime

| Package | Purpose | Version |
|---------|---------|---------|
| `express` | HTTP server and routing | 5.1.0 |
| `@vapi-ai/server-sdk` | Vapi AI platform client | 0.11.0 |
| `dotenv` | Environment variable loading | 17.2.3 |

### Development

| Package | Purpose | Version |
|---------|---------|---------|
| `typescript` | TypeScript compiler | 5.6.0 |
| `@types/express` | Express type definitions | 5.0.0 |
| `@types/node` | Node.js type definitions | 22.0.0 |
| `@flydotio/dockerfile` | Dockerfile generator | 0.7.10 |

## External Services

### Vapi AI Platform

The core dependency. Vapi provides:

- **Voice assistant creation and management** -- LLM + voice + telephony in one API
- **Phone number provisioning** -- US phone numbers with configurable area codes
- **Call handling** -- STT, LLM inference, TTS, all managed by Vapi

This server is essentially a persistence and convenience layer on top of Vapi. Without Vapi, the server has nothing to manage.

### OpenAI

The AI backend for all voice assistants:

- **Model:** `chatgpt-4o-latest`
- **Provider:** OpenAI (configured through Vapi)
- Managed entirely by Vapi; this server only passes the model name in assistant creation requests

### ElevenLabs (11Labs)

Voice synthesis for all assistants:

- **Voice ID:** `DwwuoY7Uz8AP8zrY5TAo`
- Configured through Vapi during assistant creation
- Can be changed per-assistant via the update endpoint

## Local Database

**Format:** Single JSON file (`data.json`)

**Location:** Project root (same directory as `package.json`)

**Structure:**

```json
{
  "assistants": [],
  "phoneNumbers": []
}
```

**Implementation:** `src/lib/db.ts` -- ~130 lines. Uses Node.js `fs.readFileSync` / `fs.writeFileSync` for every operation. No caching, no indexing, no migrations. Each read re-reads the file; each write overwrites it entirely.

This is intentional: the dataset is expected to be small (dozens of records at most), and simplicity is prioritized over performance.

**Backwards compatibility:** If `data.json` exists but lacks a `phoneNumbers` array (from before phone numbers were added), the database layer adds it automatically on read.

## Build and Deployment

| Step | Tool |
|------|------|
| Transpile | `tsc` (TypeScript compiler) |
| Container | Docker (multi-stage build, Node 22 slim) |
| Hosting | Fly.io (shared CPU, 1 GB RAM, auto-stop) |
| Region | YYZ (Toronto) |

## What This Server Does NOT Use

No database server (Postgres, MongoDB, etc.). No caching layer (Redis). No authentication or authorization. No rate limiting. No logging framework. No test framework. No CI/CD pipeline.

This is by design. The server is a thin management layer meant to be simple and easy to understand. If you need any of those features, they would be additions to the baseline.
