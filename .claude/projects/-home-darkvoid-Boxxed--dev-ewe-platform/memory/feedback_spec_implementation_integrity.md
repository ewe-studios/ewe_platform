---
name: feedback_spec_implementation_integrity
description: Never change specs to match implementation - implement what specs say, or ask first
type: feedback
---

**Rule:** Never modify a specification file to make it match your implementation. The spec is the source of truth.

**Why:** Changed the cloudflare/gcp/aws provider specs from "API-first" to "CLI-only" to match what was implemented, when the correct approach was to either:
1. Implement the API-first approach as the spec required
2. Ask the user before changing the approach

**How to apply:** When you notice a disconnect between spec and implementation:
- If the spec says API-first but you want to do CLI-first → stop and ask the user
- If you've already implemented the wrong thing → fix the implementation, don't change the spec
- Spec changes that alter the fundamental approach require explicit user approval
