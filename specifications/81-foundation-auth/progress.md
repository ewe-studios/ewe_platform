# Spec 81 Progress: Foundation Auth

_Last updated: 2026-11-14._

## Status at a glance

| Feature | State | Source |
|---|---|---|
| 00 — foundation auth | ⏳ 0/6, design complete | Moved from spec 83 feature 11 |

## What this spec owns

The library, not its consumers:

| Module | Tasks |
|---|---|
| Signed secrets and key files | T5, T7 |
| CA and mTLS verifiers | T8, T10 |
| Trust store | T9 |
| `ClientAuth` providers and interceptor | T11 |

## What stays out

Key protection at rest (keystore, KEK, WebCrypto, remote KMS, rotation) stays in spec 83
feature 08. This library accepts operator key files for the proof of concept.

## Next steps

1. Finish the move: add the nine-section story wrapper to the feature.
2. Run the baseline against spec 80 (Env entropy and IDs, transport facts, migration groups).
3. Implement T5 and T7 first; the consumers need signed secrets.

## Open questions

See `features/00-foundation-auth/feature.md` and `auth.md`.
