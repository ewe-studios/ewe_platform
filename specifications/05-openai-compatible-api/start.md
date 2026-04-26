---
purpose: "Entry point for agents implementing the OpenAI-compatible API specification"
version: "1.0"
created: 2026-03-08
retired: 2026-04-26
---

# ⚠️ This specification has been RETIRED

The OpenAI-compatible API implementation has been **moved to** `specifications/07-foundation-ai/`.

## Where to go

| You want to... | → Go to |
|---|---|
| Understand the current OpenAI provider | `specifications/07-foundation-ai/features/00c-openai-provider/feature.md` |
| Implement Responses API, JSON mode, etc. | `specifications/07-foundation-ai/features/00g-openai-provider-enhancements/feature.md` |
| See progress on all AI features | `specifications/07-foundation-ai/PROGRESS.md` |

## Why

This spec was created before OpenAI provider work was absorbed into the
`07-foundation-ai` specification. The Chat Completions implementation
(`openai_provider.rs`) is complete. Remaining gaps are tracked under
`00g-openai-provider-enhancements` in spec 07.

**No work should be done from this spec's feature list.**

---

_Originally created: 2026-03-08_
_Retired: 2026-04-26_
