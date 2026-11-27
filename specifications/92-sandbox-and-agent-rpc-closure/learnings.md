# Spec 92 Learnings

## Why this spec exists (2026-11-27)

Specs 77 and 83 merged with three kinds of debt:

1. **Proof debt.** The real Cloudflare container path was red, and no live run had used the
   owner's account. The real AWS and GCP KMS paths had never run.
2. **Fidelity debt.** The DST harness used a scripted registry, so the production registry
   code was not under simulation.
3. **Small correctness debt.** Eleven findings stayed open because each feature could land
   without them.

This spec exists to pay those debts. A feature of this spec is done when its issues are
closed, not when its code compiles.
