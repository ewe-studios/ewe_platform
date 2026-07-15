---
feature: "TLS transport for DockerClient (tcp:// + DOCKER_CERT_PATH mutual TLS)"
description: "DockerClient::connect_tls + https base_url + DOCKER_HOST/DOCKER_TLS_VERIFY/DOCKER_CERT_PATH; a uniform mTLS client constructor on foundation_netio's SSLConnector. Small — the TLS stack (mTLS, custom CAs) already exists in foundation_netio."
status: "complete"
priority: "medium"
phase: 3
depends_on: ["02-unix-socket-transport"]
estimated_effort: "small"
created: 2026-07-15
---
# Feature 07: TLS transport (remote Docker over `tcp://` with client certs)

## Why

Docker daemons exposed over TCP are normally protected with **mutual TLS**: the
client presents a cert (`cert.pem` + `key.pem`) signed by a CA the daemon trusts,
and verifies the daemon against a CA (`ca.pem`). This is the standard
`DOCKER_HOST=tcp://host:2376` + `DOCKER_TLS_VERIFY=1` + `DOCKER_CERT_PATH=~/.docker`
setup. Today `connect_tcp` only does plaintext, and `connect_with_defaults`
rejects any non-`unix://` `DOCKER_HOST`.

## What already exists (so this is small)

`foundation_netio` already has a full client+server TLS stack — the only new code
is a convenience constructor and the DockerClient wiring:

- `HttpClientBuilder::with_tls_connector(SSLConnector)` and the connection layer
  route on `url.scheme().is_https()` (`connect_with_tls_config`,
  `connect_https_host_port`).
- `SSLConnector` (feature-aliased; default `ssl-rustls-ring`, **already enabled**
  by this crate) exposes `RustlsConnector::with_config(Arc<ClientConfig>)` and
  re-exports `rustls::{ClientConfig, ServerConfig}` — so a full mTLS client config
  (`with_root_certificates(custom_ca).with_client_auth_cert(chain, key)`) is
  reachable. openssl (`set_certificate`/`set_private_key`/`set_ca_file`) and
  native-tls (`Identity`, `from_der`) have the same capability.
- `tls_verification.rs` already builds `ClientConfig`s with custom root stores and
  custom cert verifiers — covering private CAs and `DOCKER_TLS_VERIFY=0` (insecure).

## Scope

| Crate | Change |
|-------|--------|
| `foundation_netio` | A uniform mTLS client constructor on `SSLConnector` (per active backend): `from_client_mutual_pem(ca_pem, cert_pem, key_pem, verify_server)` — builds the connector from PEM bytes. Reachable today via `with_config`; this makes it turnkey and backend-agnostic. |
| `foundation_deployment_docker` | `DockerClient::connect_tls(host, TlsConfig)`; `base_url()` emits `https://` when TLS is active; `connect_with_defaults` handles `tcp://` + `DOCKER_TLS_VERIFY`/`DOCKER_CERT_PATH`. |

## Design

```rust
pub struct DockerTls {
    pub ca_pem:   Option<Vec<u8>>, // ca.pem — verify the daemon (None = system roots)
    pub cert_pem: Vec<u8>,         // cert.pem — client identity
    pub key_pem:  Vec<u8>,         // key.pem
    pub verify:   bool,            // DOCKER_TLS_VERIFY (false = accept any server cert)
}

impl DockerClient {
    pub fn connect_tls(host: &str, tls: DockerTls) -> Result<Self, DockerError> {
        let connector = SSLConnector::from_client_mutual_pem(
            tls.ca_pem.as_deref(), &tls.cert_pem, &tls.key_pem, tls.verify)?;
        let http = HttpClientBuilder::new()
            .with_tls_connector(connector)
            .read_timeout(Duration::from_secs(120))
            .build();
        Ok(Self { http, /* tls: true, */ remote_host: Some(host.into()), .. })
    }
}
```

- `base_url()`: `https://{host}/v{version}` when TLS, else `http://…` as today. A
  `tls: bool` (or `scheme: &str`) field on `DockerClient` records which.
- `connect_with_defaults`: `DOCKER_HOST=tcp://…` + `DOCKER_TLS_VERIFY` set →
  load `DOCKER_CERT_PATH`/{ca,cert,key}.pem and `connect_tls`; `tcp://` without
  TLS env → plaintext `connect_tcp`; `unix://` unchanged.

Docker's TCP+TLS API is HTTP/1.1 over TLS — the existing `connect_with_tls_config`
path handles it; no HTTP/2 needed.

## Verification

- `DockerClient::connect_tls` compiles; `base_url` emits `https://`.
- Integration test (gated on a TLS daemon): start dockerd with
  `--tlsverify --tlscacert ca.pem --tlscert server-cert.pem --tlskey server-key.pem
  -H tcp://0.0.0.0:2376`, generate client certs from the same CA, then `info()` /
  a container round-trip over `connect_tls`. Skips cleanly when the TLS daemon /
  certs are absent.

## Acceptance criteria

1. `SSLConnector::from_client_mutual_pem` builds an mTLS client connector from PEM.
2. `DockerClient::connect_tls` speaks to a `--tlsverify` daemon; `base_url` is `https`.
3. `connect_with_defaults` honors `tcp://` + `DOCKER_TLS_VERIFY` + `DOCKER_CERT_PATH`.
4. `verify=false` accepts a self-signed daemon cert (`DOCKER_TLS_VERIFY=0`).

## Outcome (complete 2026-07-15)

All acceptance criteria met. Implementation:

- `foundation_netio`: `SSLConnector::from_client_mutual_pem(ca_pem, cert_pem, key_pem, verify)`
  (rustls); `NoServerCertVerify` for the `verify=false` path (validates handshake
  signatures, skips chain/hostname). `HttpClientBuilder::with_tls_connector` threads
  a custom connector into `NativeHttpClient`.
- `foundation_deployment_docker`: `DockerTls { from_cert_dir }`, `DockerClient::connect_tls`,
  `base_url()` emits `https://` under TLS, and `connect_with_defaults` parses
  `unix://` / `tcp://` (+ `DOCKER_TLS_VERIFY` / `DOCKER_CERT_PATH`) / `ssh://`.

Verified against docker-in-docker (`DOCKER_TLS_CERTDIR`) — `tests/tls_transport_tests.rs`
passes 3/3: full-mTLS `info`, mTLS container round-trip, and insecure-TLS `info`.
