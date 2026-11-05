---
feature: "foundation_netio and the Docker client handle a 1xx interim response the client did not ask for"
status: "done"
lane: "A"
model: "haiku"
branch: "fix/io-41-50-lane-a"
crates: ["foundation_netio", "foundation_deployment_docker"]
catalogue_rows: [59]
---

# Feature 69: unsolicited interim responses

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they are. Follow [requirements.md](../../requirements.md) sections 3, 4 and 8._

## Why (owner question after feature 43)

Feature 43 turned off `Expect: 100-continue` for the Docker client. RFC 9110 §15.2 says a client
must accept a 1xx interim response even when it did not ask for one (a server or proxy may send
`100 Continue` or `103 Early Hints` anyway). The code skips interim responses in `ReadingHead`
(`backends/foundation_netio/src/http/tasks/request_redirect.rs`, the `(100..200).contains(&code)
&& code != 101` check), but no test proves it end to end with the handshake off.

## Tests to add (copy the in-file test server from
`backends/foundation_netio/tests/simple_http/expect_continue_timeout_tests.rs`)

In `backends/foundation_netio/tests/simple_http/unsolicited_interim_tests.rs` (register it in
`tests/simple_http/mod.rs`), all with `HttpClientBuilder::new().expect_continue(false)`:

1. `unsolicited_100_before_final_is_skipped_tcp`: POST with a body; the server reads the whole
   request, then sends `HTTP/1.1 100 Continue\r\n\r\n` and then `200` with a body. Assert the
   client returns 200 and the body, and the request had no `expect` header.
2. Same over a Unix socket: `unsolicited_100_before_final_is_skipped_unix`.
3. `early_hints_103_with_headers_is_skipped`: the server sends
   `HTTP/1.1 103 Early Hints\r\nLink: </a.css>; rel=preload\r\n\r\n` then `200`. Assert 200, and
   that the `Link` header is not on the final response.
4. `several_interim_responses_are_skipped`: `100`, then `102 Processing`, then `200`.
5. `unsolicited_100_on_get_is_skipped`: a GET (no body) gets `100` then `200`.

In `backends/foundation_deployment_docker/tests/expect_continue_tests.rs` (needs
`--features docker`): `docker_client_skips_unsolicited_100` — the mock daemon sends `100
Continue` and then the normal JSON reply to `containers/create`; the typed call succeeds.

If any test fails, the client has a bug: fix it in netio (lane A) with the failing test as the
regression test, and say what was wrong.

## Verify

```bash
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0
cargo test -p foundation_netio --profile dev --test foundation_netio_tests unsolicited_interim
cargo test -p foundation_deployment_docker --profile dev --features docker --test expect_continue_tests
python3 scripts/ci/ci.py run foundation_netio --profile dev
```
Flake check: 20 runs of each new test.

## Done when

- [x] Six tests pass 20/20 (or each failure is fixed with its test). CI green.
