---
feature: "009-ssh-key-provisioning"
spec: "57-foundation-social-and-keychain"
depends: "008-keychain-vault-and-backends"
status: "complete"
---

# Feature 009: SSH Key Provisioning + App Registry

## Current state
✅ Complete. 548 LOC in `core/provisioning/` (apps, keygen, ssh_keys, store).
3 integration tests pass. Ed25519 + RSA 4096 key generation via `ssh-key`,
age-encrypted private keys at rest, Argon2id app secrets, X-App-Secret +
Bearer auth. Native-gated (native only — wasm deferred for future).
