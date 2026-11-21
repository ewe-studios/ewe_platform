# LEARNINGS — Proxy Registration and Registrar

## One spec, two halves (2026-11-14)

The proxy registration authority and the registrar sidecar are two ends of one contract. Put
them in one specification, or the proto, the failure table and the task order drift apart.

They are still two features with two owners:

- Feature 00 extends `foundation_proxy`. It owns the wire contract, the registry, leases, the
  rotation rule and the router swap.
- Feature 01 is a new crate, `foundation_registrar`. It owns discovery, probes, metrics and the
  reconcile loop.

## Read the existing proxy first

`foundation_proxy` already has a fixed router, health probes, a Unix control socket, a persisted
state store and TLS. The registration work extends them. A second probe loop or a second control
channel is a defect.

## The two auth links

- The sidecar authenticates only to the proxy registration authority: enrollment token one time,
  then mTLS, or a Unix socket with file permissions.
- The proxy ends client TLS. It never sees the app's client credential.
- The sidecar never takes part in client TLS.

## Health has Kubernetes semantics

A startup probe gates liveness and readiness. A liveness failure is a failure state. A readiness
failure removes the backend from rotation but keeps it alive. A missed report lets the lease
expire; a later report brings the backend back. No operator action is needed except for an
expired or revoked certificate.

## Rotation must not drop live connections

A backend that leaves gets no new requests. Its open connections finish, up to
`connection_drain_timeout`. The router swaps whole immutable snapshots with a generation number;
no request sees a half-built table.
