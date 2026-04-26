# Voice Agent Server -- REST API

Base URL: `http://localhost:3000`

All request and response bodies are JSON.

## Assistants

### List All Assistants

```
GET /assistants
```

Returns all assistants stored in the local database. Does not call Vapi.

**Response:**

```json
{
  "assistants": [
    {
      "id": "uuid-here",
      "name": "Alice",
      "firstMessage": "Hey! How can I help?",
      "systemPrompt": "You are Alice...",
      "voiceProvider": "11labs",
      "voiceId": "DwwuoY7Uz8AP8zrY5TAo",
      "endCallMessage": "Thank you for calling. Goodbye!",
      "maxDurationSeconds": 300,
      "phoneNumberId": "optional-phone-id",
      "vapiAssistantId": "vapi-xxx",
      "createdAt": "2025-01-15T10:30:00.000Z"
    }
  ]
}
```

### Get Assistant by ID

```
GET /assistants/:id
```

Fetches the local record and enriches it with live data from Vapi.

**Response:** Local assistant record merged with the full Vapi assistant object.

### Create Assistant

```
POST /assistants
```

**Request body:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | Yes | Assistant display name |
| `firstMessage` | string | Yes | Opening greeting spoken to callers |
| `systemPrompt` | string | No | Custom system prompt (uses [default](./00-overview.md#default-assistant-personality) if omitted) |

The server creates the assistant on Vapi with these fixed defaults:

- **Model:** OpenAI `chatgpt-4o-latest`
- **Voice:** 11Labs, voice ID `DwwuoY7Uz8AP8zrY5TAo`
- **Max duration:** 300 seconds (5 minutes)
- **End call message:** "Thank you for calling. Goodbye!"

Then stores the record locally with a UUID and timestamp.

**Response:** The created assistant record.

### Update Assistant

```
PATCH /assistants
```

**Request body:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | string | Yes | Local assistant ID to update |
| `name` | string | No | New name |
| `firstMessage` | string | No | New greeting |
| `systemPrompt` | string | No | New system prompt (replaces AI model config in Vapi) |
| `voiceProvider` | string | No | New voice provider (e.g. "11labs") |
| `voiceId` | string | No | New voice ID |
| `endCallMessage` | string | No | New end-of-call message |
| `maxDurationSeconds` | number | No | New max call duration |
| `phoneNumberId` | string | No | Link a phone number to this assistant (also updates Vapi phone routing) |

Only provided fields are updated. The server sends a partial update to Vapi and patches the local record.

When `phoneNumberId` is included, the server also updates the Vapi phone number record to route incoming calls to this assistant.

**Response:** The updated assistant record.

### Delete Assistant

```
DELETE /assistants
```

**Request body:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | string | Yes | Local assistant ID to delete |

Deletes from Vapi first, then removes the local record.

**Response:**

```json
{ "message": "Assistant deleted successfully" }
```

---

## Phone Numbers

### List All Phone Numbers

```
GET /phone-numbers
```

Returns all phone numbers from the local database. Does not call Vapi.

**Response:**

```json
{
  "phoneNumbers": [
    {
      "id": "uuid-here",
      "name": "Main Line",
      "number": "+12075551234",
      "areaCode": "207",
      "assistantId": "optional-assistant-id",
      "vapiPhoneNumberId": "vapi-phone-xxx",
      "createdAt": "2025-01-15T10:30:00.000Z"
    }
  ]
}
```

### Get Phone Number by ID

```
GET /phone-numbers/:id
```

Fetches the local record and enriches with live Vapi data (if available).

**Response:** Local phone number record merged with the Vapi phone number object.

### Create Phone Number

```
POST /phone-numbers
```

**Request body:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | Yes | Display name for the phone number |
| `assistantId` | string | No | Local assistant ID to link (must exist in local DB) |

Provisions a new phone number on Vapi with area code **207** (Maine, USA). If `assistantId` is provided, the number is linked to that assistant so incoming calls route to it.

**Response:** The created phone number record.

### Update Phone Number

```
PATCH /phone-numbers
```

**Request body:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | string | Yes | Local phone number ID to update |
| `assistantId` | string | No | New assistant to link (or null to unlink) |

Updates the Vapi phone number routing and the local record.

**Response:** The updated phone number record.

### Delete Phone Number

```
DELETE /phone-numbers
```

**Request body:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | string | Yes | Local phone number ID to delete |

Deletes from Vapi first, then removes the local record.

**Response:**

```json
{ "success": true, "message": "Phone number deleted" }
```

---

## Health Check

```
GET /
```

Returns `Hello World`. Simple liveness probe.

## Error Responses

All endpoints return errors with a consistent shape:

```json
{ "error": "Description of what went wrong" }
```

HTTP status codes:

| Code | Meaning |
|------|---------|
| 400 | Missing required fields or invalid reference (e.g., assistant not found) |
| 404 | Resource not found in local database |
| 500 | Vapi API error or internal failure (Vapi error details included when available) |
