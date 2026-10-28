# Spec 70 learnings

_What an agent learned that the next agent needs. Newest first._

- 2026-10-28 (from the PR #160 work): `H2Connection::flush_write` used `write_all`, so a
  `WouldBlock` part-way re-sent the already-written prefix and corrupted the HTTP/2 frame
  stream. Any non-blocking writer must track how much it wrote.
- 2026-10-28: the shared reactor's drain thread must request an executor resume after it latches
  events, or parked tasks wait for the idle back-off (up to 100 ms).
