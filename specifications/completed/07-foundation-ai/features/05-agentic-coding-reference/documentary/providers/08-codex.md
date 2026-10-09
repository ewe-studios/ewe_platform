# OpenAI Codex Provider -- Deep Dive

## Files Referenced

- hermes-agent: `_CodexCompletionsAdapter` in `run_agent.py`
- pi-mono: `packages/ai/src/providers/openai-codex-responses.ts`

## API Mode

Codex uses the OpenAI Responses API mode (`codex_responses`), accessed via `chatgpt.com/backend-api/codex`.

## Codex Adapter Pattern

hermes-agent implements `_CodexCompletionsAdapter` that:
1. Intercepts `chat.completions.create()` calls
2. Translates them to Responses API format internally
3. Returns responses in Chat Completions format for the caller

This allows the rest of the code to use the familiar Chat Completions interface while the adapter handles the Responses API translation.

## Authentication

Codex uses OAuth authentication rather than a simple API key. The OAuth token is obtained through a browser-based auth flow.

## Response Format

The Responses API returns:
- `response.output` array of items (text, function_call, etc.)
- Function calls as separate items in the output array
- Different event types for streaming

See the [OpenAI Responses API](03-openai-responses.md) document for the detailed format.

## Key Patterns for foundation_ai

1. **Adapter pattern** -- translate Chat Completions calls to Responses API internally
2. **OAuth authentication** -- browser-based auth flow, not API key
3. **Chatgpt.com routing** -- specific endpoint `chatgpt.com/backend-api/codex`
4. **Same Responses API format** -- see openai-responses document for details
