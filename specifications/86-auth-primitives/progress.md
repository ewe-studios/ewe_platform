# Spec 86 Progress: Auth Primitives

_Last updated: 2026-11-15._

## Status at a glance

| Task | State |
|---|---|
| T1 — `Secret`, the `ConfidentialText` alias, redacted output, `ct_eq` | ⬜ Pending |
| T2 — One `constant_time_eq`; delete the four copies | ⬜ Pending |
| T3 — `extract_bearer_token` over `Secret::from_bearer`; `bearer_header` | ⬜ Pending |
| T4 — `AuthInfo` moves to `foundation_auth`; connectrpc re-exports it | ⬜ Pending |
| T5 — `RequestFacts`, `ClientAuth`, `AuthError` | ⬜ Pending |
| T6 — `StaticToken`; version bumps and consumers | ⬜ Pending |

## No product code exists

This specification is a plan in a pull request for the owner's review. No file under
`backends/` changes until the owner approves it.

## Where this specification came from

The owner reviewed six conflicts on 2026-11-15. Two of them are this specification:

- Spec 77 feature 00 defined `pub struct Secret(String)` and feature 02 defined `check_bearer`,
  while spec 81 owned client auth with no static-token provider. The workspace would get two
  secret types and two bearer parsers.
- Spec 81 owned `AuthInfo` and the providers, but a sandbox server cannot use them without the
  whole of spec 81.

Owner direction: "lets have another small spec own this and implement all the need bits into
foundation_auth, then others depend on it and refine it as needed."

## Next steps

1. T2 first: one comparison, and the four copies go. It has no new type and it is easy to test.
2. T1 next: `Secret` and the alias. It uses T2.
3. T3: the bearer parser over T1.
4. T4: the move of `AuthInfo`. It is a mechanical move plus one shim.
5. T5: the trait. It needs T4 for `AuthInfo`.
6. T6: `StaticToken` over T1, T3 and T5.

## Known issues to record

| Finding | Severity | State |
|---|---|---|
| Four private copies of `constant_time_eq` in `foundation_auth` | low | [#554](https://github.com/ewe-studios/ewe_platform/issues/554); T2 closes it |
| `ConfidentialText::Display` shows the first character and the length of the secret | low | [#555](https://github.com/ewe-studios/ewe_platform/issues/555); T1 closes it |

## Open questions

None. Section 10 of `requirements.md` records the five decisions.
