# Spec 92 Progress: closing specs 77 and 83

_Last updated: 2026-11-27._

## Status at a glance

| Feature | State |
|---|---|
| 00 Live proof on the real services | not started |
| 01 DST fidelity: the real registry | not started |
| 02 Sandbox follow-ups | not started |
| 03 GCP KMS over gRPC | not started |
| 04 Live cloud resources with OpenTofu | not started |

## What is already true

Specs 77 and 83 are merged on `master` (`348620340`, then `39cd005f1` and `c9a913dc0`). The
combined pull request #1009 and the dependency pull request #1040 are merged.

The owner rejected spec 77 feature 16 (Docker Sandboxes, `sbx`) on 2026-11-27; decision D18.

The owner confirms that `CLOUDFLARE_ACCOUNT_ID` and `CLOUDFLARE_WORKERS_AI_API_TOKEN` are in
the shell environment and in `~/.userenv`, which unblocks spec 83 feature 00 task T7.

## Next

1. Feature 00, task T2 first: make `sandbox-cloudflare` green. The red run's log names the
   step and now prints the failing value.
2. Feature 00, task T1: one live run against the owner's account, recorded here.
3. Feature 00, tasks T3 to T6: the KMS and WebCrypto tests.
