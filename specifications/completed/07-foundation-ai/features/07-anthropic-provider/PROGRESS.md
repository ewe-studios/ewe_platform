# Progress - Anthropic Messages API Provider

_Last updated: 2026-06-15_

**Status:** ✅ Complete — 44 / 44 tasks (100%)

`AnthropicMessagesProvider` connecting to Anthropic's `/v1/messages` endpoint.
`AnthropicConfig` with builder, native auth (`x-api-key` + `anthropic-version`),
full request/response types (`MessagesRequest`, `AnthropicContentBlock`,
`AnthropicToolChoice`), SSE streaming (`StreamEvent`, `AnthropicDelta`),
`AnthropicModel` implementing `Model` trait, `AnthropicStream` iterator,
stop reason mapping, error handling with retry/backoff, 5 mock integration
tests + 5 llama-server integration tests (gated).
