# 25 — SSL redirect (HTTP→HTTPS)

**Date:** 2026-07-10
**Status:** Resolved

## Decision

When TLS is enabled, `foundation_proxy` serves a plain-HTTP listener on port 80
that redirects every request to `https://{host}{path}` with a `301 Moved
Permanently`. This is bundled with TLS termination (Decision 18) — it's a small,
self-contained addition to the TLS stage.

## Why

kamal-proxy does this automatically when `--tls` is set. Without it, users who
type `http://app.example.com` get a connection refused or the proxy's raw HTTP
front-end, neither of which is correct.

## Design

```rust
// In ProxyServer::start(), after TLS listener is up:
if config.ssl.provider != SslProvider::None {
    let http_listener = TcpListener::bind("0.0.0.0:80")?;
    let redirect_shutdown = shutdown.clone();
    std::thread::spawn(move || {
        redirect_loop(http_listener, &redirect_shutdown);
    });
}

fn redirect_loop(listener: TcpListener, shutdown: &OnSignal) {
    for stream in listener.incoming() {
        if shutdown.probe() { break; }
        if let Ok(mut stream) = stream {
            stream.set_read_timeout(Some(Duration::from_secs(2))).ok();
            let mut buf = [0u8; 4096];
            if let Ok(n) = stream.read(&mut buf) {
                if let Some(host) = extract_host(&buf[..n]) {
                    let path = extract_path(&buf[..n]).unwrap_or("/");
                    let redirect = format!(
                        "HTTP/1.1 301 Moved Permanently\r\nLocation: https://{host}{path}\r\nConnection: close\r\n\r\n"
                    );
                    let _ = stream.write_all(redirect.as_bytes());
                }
            }
        }
    }
}
```

Per-service override: `ServiceConfig::ssl_redirect(bool)` — defaults to `true`.
Set to `false` for services that intentionally serve plain HTTP (internal
health endpoints, Prometheus metrics, local dev).

## Verification

1. Proxy with `SslProvider::Static`, curl `http://app.local` → `301` to `https://app.local`.
2. Service with `ssl_redirect: false` → plain HTTP served, no redirect.
3. TLS disabled → no port 80 listener, no redirect.
