# Voice Agent Server -- Development

## Prerequisites

- Node.js 22.x or later
- npm (comes with Node.js)
- A Vapi API key

## Setup

```bash
# Clone or navigate to the project
cd voice-agent-server

# Install dependencies
npm install

# Create .env file with your Vapi API key
echo 'VAPI_API_KEY=your-key-here' > .env
```

## Development Workflow

### Build

```bash
npm run build
```

Compiles TypeScript to `dist/`. Run this before `npm start`.

### Start (Production)

```bash
npm start
```

Runs the compiled JavaScript from `dist/index.js`. Requires a prior build.

### Development Mode

```bash
npm run dev
```

Runs TypeScript in watch mode (`tsc -w`) and Node with `--watch` so the server restarts on file changes. This is the recommended workflow during development.

## Project Layout

```
src/
├── index.ts          # Main entry point. Express app, all route handlers (~350 lines)
└── lib/
    ├── db.ts         # JSON file database with CRUD for assistants and phone numbers
    └── vapi.ts       # Vapi SDK client (single line: creates client with API key)
```

That's it. Three files, ~500 lines total.

## Testing the API

Once the server is running (`npm run dev`), you can test with curl:

### Create an assistant

```bash
curl -X POST http://localhost:3000/assistants \
  -H 'Content-Type: application/json' \
  -d '{
    "name": "TestBot",
    "firstMessage": "Hey there! What can I help with?"
  }'
```

### List assistants

```bash
curl http://localhost:3000/assistants
```

### Create a phone number

```bash
curl -X POST http://localhost:3000/phone-numbers \
  -H 'Content-Type: application/json' \
  -d '{
    "name": "Test Line"
  }'
```

### Link a phone number to an assistant

```bash
curl -X PATCH http://localhost:3000/phone-numbers \
  -H 'Content-Type: application/json' \
  -d '{
    "id": "<phone-number-id>",
    "assistantId": "<assistant-id>"
  }'
```

## Database File

The local database lives at `data.json` in the project root. It is created automatically on the first write operation. You can inspect and edit it directly -- the server reads it fresh on every request.

```json
{
  "assistants": [],
  "phoneNumbers": []
}
```

To reset the database, delete `data.json`. The server will recreate it on the next write.

## Notes

- **No tests:** The project has no test suite. Testing is done manually via API calls.
- **No linting:** No ESLint or Prettier config. The TypeScript compiler's strict mode catches most issues.
- **No auth:** The API has no authentication. Deploy it behind a trusted network or add your own auth layer.
