# Specification 91 learnings

## 2026-11-24

The Cargo fixture builds its temporary package offline. The Windows guest needs the fixture dependencies in its Cargo cache.
The platform condition on the atomic exchange test explains why macOS and Linux run one additional staging test.
Windows compilation checks leave native filesystem behavior unverified.
