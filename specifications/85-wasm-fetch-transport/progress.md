# Spec 85 Progress: Wasm Fetch Transport

_Last updated: 2026-11-15._

## Status at a glance

| Task | State |
|---|---|
| T1 — `Fetcher`, `GlobalFetcher`, `FetchTransport`, the pump and builders | ⬜ Pending |
| T2 — `FetchServe`, `DispatchBody`, the channel sink, the `foundation_netio` helpers | ⬜ Pending |
| T3 — `StubFetcher`, `DoStubTransport` | ⬜ Pending |
| T4 — `SendWrapper` stream impl and the injected deadline | ⬜ Pending |
| T5 — The Node test runner and the CI entry | ⬜ Pending |

## No product code exists

This specification is a plan in a pull request for the owner's review. No file under
`backends/` changes until the owner approves it.

## Design corrections (2026-11-15)

- `FetchTransport` held `fetcher: Rc<F>`, and `Transport` requires `Send + Sync + 'static`
  (`backends/foundation_connectrpc/src/shared/transport/base.rs:161`), so the struct could not
  implement the trait. The field is now `SendWrapper<Rc<F>>`. The same holds for every JS value
  the transport returns: `HeadStream` and `BodyStream` are `Send` aliases, so `body_stream` and
  the pump wrap their JS values too.
- `fetch.md` said `SendWrapper` panics on a drop from another thread. The real type has no
  `Drop` check and no run-time check at all. The paragraph now names the compile-time gate
  instead.
- Section 5 has a new rule 9: a JavaScript value crosses a `Send` slot only inside
  `SendWrapper`, never with a second wrapper and never with `unsafe impl Send`.

Issue #556 records these three corrections, with the audit that found them. It is closed.

## Where this specification came from

The design lived in spec 83 feature 05 (`worker-and-web.md` sections 5.4, 5.5 and 6.2) as
tasks T19 and T21. Spec 77 feature 07 also planned its own body pump (about 80 lines) and
said it did not wait for spec 83.

On 2026-11-15 the owner gave the direction: one specification owns the plumbing, and both
consumers build on it. This specification is that owner.

| Before | After |
|---|---|
| Spec 83 feature 05 T19 (`FetchServe`, `FetchTransport`) | Spec 85 T1 and T2 |
| Spec 83 feature 05 T21, transport part (`DoStubTransport`) | Spec 85 T3 |
| Spec 77 feature 07 own body pump | `response_stream` of spec 85 T1 |
| Spec 77 feature 08 own Worker entry details | `FetchServe` of spec 85 T2 |

## Next steps

1. T4 first. It is small and it unblocks the deadline read of `FetchServe`.
2. T1 next. The transport has a test with a stub `Fetcher` and needs no Cloudflare binding.
3. T2 after T1, because it shares the channel and the body types.
4. T3 after T1, behind the `cloudflare` feature.
5. T5 last, because the Node runner needs the tests of T1 to T3.

## Open questions

None. Section 12 of `requirements.md` records the five decisions.
