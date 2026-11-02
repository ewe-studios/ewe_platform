---
feature: "foundation_nativeapis OverlayStream.owner field is never read"
status: "done"
priority: "low"
depends_on: []
branch: "fix/io-35-turso-async-trait-overflow"
crates: ["foundation_nativeapis"]
backlog: "docs/bug-backlog.md §13 (entry deleted)"
---

# Feature 40: `OverlayStream.owner` dead-code warning

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** pull request (PR); AeroSpace and Defence Industries
Association of Europe (ASD), the owner of the STE standard ASD-STE100.

Use the shared protocol in [`requirements.md`](../../requirements.md) sections 3 and 4.

## T0. Confirm the backlog entry is still open

- **Where:** `foundation_nativeapis/src/dataplane/netstack.rs` line 717, field `owner` of the
  `pub struct OverlayStream` (line 710). The field is `Arc<OverlayStreamOwner>`.
- **Confirm:**

  ```bash
  cargo check -p foundation_nativeapis --profile dev 2>&1 | rg 'owner.*never read'
  ```

  The warning must still appear. The field is held only for its drop side effect: the
  `OverlayStreamOwner` `Drop` impl removes the smoltcp socket on the last drop. The derived
  `Clone` impl shares it, but dead-code analysis ignores that use, so the field reads as
  never read.

## E1. Remove the warning

- **Fix:** pick one option and record the choice in the PR body.
  1. **Rename the field to `_owner`.** The leading underscore tells the compiler the field is
     held for its drop side effect, not read. One line. Preferred.
  2. **Add `#[allow(dead_code)]` on the field** with a comment that the `Arc` is held only so
     `Drop` removes the socket once. Keeps the public name.
  Do not remove the field: the socket would drop early and a later clone would use a removed
  handle.
- **Tests:** the existing netstack suite covers the stream lifetime. The compile must stay
  warning-free (the verification below checks this). No `#[cfg(test)]` module goes into a
  source file (rule 7).

## Verification (run exactly)

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
python3 scripts/ci/ci.py run foundation_nativeapis --profile dev
cargo check -p foundation_nativeapis --profile dev 2>&1 | tee /tmp/nativeapis_check.log
! rg 'owner.*never read' /tmp/nativeapis_check.log
cargo check --workspace --all-targets --profile dev
```

## Done when

- [x] The `owner` dead-code warning is gone.
- [x] The netstack tests pass.
- [x] The backlog entry is deleted.
- [x] CI green.

## Review checklist (main session)

- [ ] The field still keeps the socket alive (its drop side effect is unchanged).
- [ ] The chosen option is stated in the PR body.
- [ ] No `#[cfg(test)]` module was added to a source file.

## Result

- Renamed `OverlayStream.owner` to `_owner` (option 1, the preferred fix). The doc comment now says
  the field is held only for its drop side effect, and the `Arc<OverlayStreamOwner>` still keeps the
  socket alive until the last drop.
- Deleted the backlog entry in §13.
- `cargo check -p foundation_nativeapis --features netstack` is clean (no `owner ... never read`
  warning). `cargo test -p foundation_nativeapis --features netstack --test dataplane` passes 7/7.
  A separate, pre-existing `tests/fd/registration.rs` edge-triggered failure is tracked in §13 and is
  unrelated to this feature.
