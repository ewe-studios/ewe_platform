# Learnings

## Spec Implementation Integrity

**Never change specs to match implementation.**

When there is a disconnect between what a specification says and what was implemented:

1. **Fix the implementation** to match the spec, OR
2. **Ask the user first** before changing the approach

**Why this matters:** Changed the Cloudflare/GCP/AWS provider specs from "API-first" to "CLI-only" to match what was implemented, when the correct approach was to implement the API-first approach as specified, or ask before deviating.

**Going forward:** The spec is the source of truth. If the spec says API-first, implement API-first. If there's a reason to do CLI-first instead, ask the user and only change the spec after explicit approval.
