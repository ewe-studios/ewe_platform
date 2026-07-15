# Feature 03 — SWIM Gossip & Membership

**Status:** ✅ Complete (implemented + tested 2026-07-13)
**Depends on:** 01
**Unblocks:** 04
**Decisions:** [05](../../decisions/05-swim-full-membership-gossip.md), [06](../../decisions/06-hybrid-transport-tls-psk.md)

## Implementation notes (2026-07-13)

`foundation_wireguard::shared::membership` — fully sans-I/O, simulated-clock testable:

- `record` — `PeerId`, `Capabilities`, `MemberState`, `PeerRecord` with the CRDT
  `supersedes` rule (higher incarnation wins; `Dead`>`Suspect`>`Alive` at equal
  incarnation; higher heartbeat = LWW on endpoints/caps).
- `mod`/`Membership` — the CRDT set + `merge_record` returning an observable delta.
- `message` — `SwimMessage` (Ping/Ack/PingReq/Gossip/SyncDigest/SyncResponse), `SwimOutbound`,
  and a compact version-prefixed `bincode` codec (`encode`/`decode`).
- `swim::Swim` — the state machine: `tick(now)` (probe → indirect ping-req → suspect →
  dead → tombstone → GC + anti-entropy), `on_message` (dispatch + direct-contact liveness),
  `merge`/`snapshot` (join/PullMembership). Seeded xorshift for reproducible peer choice.
  Epidemic re-gossip of *changed* records only (terminates on convergence); one-shot
  incarnation refutation; a `reaped` guard stops anti-entropy from resurrecting GC'd
  tombstones while still allowing higher-incarnation rejoins.

Tests (`--profile uat`, in-memory lossy network + simulated clock): codec round-trip,
CRDT order-independence/idempotency/refutation, N-node convergence with **and under 30%
loss**, dead-node detection with **no false positives**, and tombstone GC. All green,
zero warnings.

## WHY

Masterless discovery: the sans-I/O SWIM full-membership state machine that spreads peer records
epidemically, detects failures, and reconciles by anti-entropy — so the network self-heals and
survives the seed's death.

## WHAT

`shared::membership` — a pure, transport-agnostic, simulated-clock-testable state machine:

1. `PeerRecord` + `Membership` (the CRDT-like set).
2. `Swim` state machine: probe/ping-req, suspicion, tombstones, incarnation refutation, anti-entropy.
3. `SwimEvent` outputs (membership changes) + `SwimMessage` I/O (fed by the mesh/transport).

## HOW

### Types ([decision 05](../../decisions/05-swim-full-membership-gossip.md))

```rust
struct PeerRecord { identity_pubkey:[u8;32], tunnel_ip:IpAddr, endpoints:Vec<SocketAddr>,
                    caps:Capabilities, incarnation:u64, state:MemberState, heartbeat:u64 }
enum MemberState { Alive, Suspect, Dead /*tombstone+ttl*/ }

trait SwimClock { fn now(&self) -> Instant; }  // real or simulated
struct Swim { me: PeerRecord, members: Membership, cfg: SwimConfig }
impl Swim {
    fn tick(&mut self, now: Instant) -> (Vec<SwimMessage>, Vec<SwimEvent>);   // periodic
    fn on_message(&mut self, from:&PeerId, msg: SwimMessage) -> (Vec<SwimMessage>, Vec<SwimEvent>);
    fn merge(&mut self, records: &[PeerRecord]) -> Vec<SwimEvent>;            // anti-entropy / join
}
```

- **Merge rule (commutative/idempotent):** per `identity_pubkey`, keep the higher
  `(incarnation, heartbeat)`; `Dead` beats `Alive` at equal incarnation unless refuted by a higher
  incarnation; tombstones GC after `tombstone_ttl`.
- **Failure detection:** `tick` schedules a direct `Ping` to a random member; on timeout, `PingReq`
  via *k* random members; unrefuted → `Suspect` → (after suspicion timeout) `Dead`.
- **Refutation:** hearing self `Suspect`/`Dead` → bump own `incarnation`, broadcast `Alive`.
- **Anti-entropy:** `tick` periodically emits a `SyncDigest` to a random peer; `merge` on the reply.

### Driving it

- The mesh layer (feature 04) owns the transport: during **join** it feeds `merge` from
  `PullMembership` (feature 02); in **steady state** it routes `SwimMessage`s over the **in-tunnel**
  UDP channel ([decision 06](../../decisions/06-hybrid-transport-tls-psk.md)) and calls `tick` on the
  driver's timer.
- `SwimEvent::{PeerUp, PeerEndpointChanged, PeerDown, CapsChanged}` are consumed by feature 04 to add/
  update/remove `WgTunnel`s and by feature 05 for relay selection.

## Task list

1. `PeerRecord`/`Membership` + merge semantics + property tests (commutativity, idempotency,
   convergence).
2. `Swim` state machine (probe, ping-req, suspicion, tombstone, refutation, anti-entropy) with a
   `SwimClock` abstraction.
3. `SwimMessage` codec (compact, versioned) for the in-tunnel channel.
4. Simulation harness: N virtual nodes over an in-memory lossy network + simulated clock; assert
   convergence, failure detection, seed-death survival.
5. Tests: convergence under message loss; false-positive avoidance via ping-req; tombstone GC.

## Test plan / success

- N-node simulation converges to identical membership after churn (joins/leaves/failures).
- A killed node is detected `Dead` without false-positiving healthy nodes.
- Membership merge is order-independent (CRDT property).
