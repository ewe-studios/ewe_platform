# Progress - Anthropic Messages API Provider

## Overview

Implement `AnthropicMessagesProvider` connecting to Anthropic's Messages API
(`/v1/messages`) — the primary interface for all Claude models.

## Task Status

| # | Task | Status |
|---|------|--------|
| 1 | AnthropicConfig with builder | Pending |
| 2 | AnthropicRole, AnthropicMessage, AnthropicContentBlock types | Pending |
| 3 | AnthropicImageSource struct | Pending |
| 4 | AnthropicTool, AnthropicToolChoice types | Pending |
| 5 | AnthropicThinkingConfig | Pending |
| 6 | MessagesRequest struct | Pending |
| 7 | MessagesResponse, AnthropicUsage | Pending |
| 8 | StreamEvent, AnthropicDelta, MessageDeltaDelta | Pending |
| 9 | AnthropicMessagesProvider struct + Default | Pending |
| 10 | AuthProvider impl on AnthropicConfig | Pending |
| 11 | ModelProvider trait impl | Pending |
| 12 | Model listing via /v1/models | Pending |
| 13 | AnthropicModel struct | Pending |
| 14 | AnthropicModel::generate() | Pending |
| 15 | AnthropicModel::stream() | Pending |
| 16 | AnthropicStream iterator | Pending |
| 17 | build_anthropic_request() helper | Pending |
| 18 | Message mapping (User/Assistant → AnthropicMessage) | Pending |
| 19 | Image mapping to AnthropicImageSource | Pending |
| 20 | Tool mapping to AnthropicTool | Pending |
| 21 | Tool choice mapping | Pending |
| 22 | parse_response() helper | Pending |
| 23 | Stop reason mapping | Pending |
| 24 | HTTP status → GenerationError mapping | Pending |
| 25 | Anthropic error response parsing | Pending |
| 26 | Retry with exponential backoff | Pending |
| 27 | Mock tests: generate, streaming, tool calls | Pending |
| 28 | Mock test: extended thinking | Pending |
| 29 | Mock test: multimodal | Pending |
| 30 | Integration tests: generate, streaming, tool use | Pending |
| 31 | Integration test: extended thinking | Pending |
| 32 | Register module in backends/mod.rs | Pending |

**Totals:** 0 / 32 tasks complete (0%)

## What's Done

Nothing yet — feature spec just created.

## Notes

- Auth: `x-api-key` + `anthropic-version` headers (not Bearer token)
- Default timeout: 120s (reasoning models are slow)
- Streaming uses named SSE events (`message_start`, `content_block_delta`, etc.)
- Content is always an array of blocks, never a plain string
- Extended thinking (Claude 3.7+) produces separate thinking content blocks
