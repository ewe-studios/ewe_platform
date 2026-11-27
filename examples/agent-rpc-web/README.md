# agent-rpc-web

The agent-rpc browser client, exported to JavaScript. The Rust code is
`BrowserClient` in `src/lib.rs`; the page in `index.html` calls it.

The client uses an operator-issued access payload (token purpose `Access`).
Enrollment with a WebCrypto client key is not in this example yet: the Worker
host does not serve `BootstrapService`.

## Build

```bash
cargo install wasm-bindgen-cli --version 0.2.130
cargo build --target wasm32-unknown-unknown --release
wasm-bindgen --target web --out-dir pkg \
  target/wasm32-unknown-unknown/release/agent_rpc_web.wasm
python3 -m http.server 8000
```

Open <http://localhost:8000>, paste an access payload, connect, create an
instance and chat. The page and the host must be on origins that the host names
in `AGENT_RPC_CORS_ORIGINS`.

## What the client does

| Call | Connect procedure |
|---|---|
| `capabilities()` | `GetCapabilities` |
| `createInstance(name, model)` | `CreateInstance` |
| `destroyInstance(name)` | `DestroyInstance` |
| `chat(instance, prompt)` | `ChatStream`, collected to text |

Every call carries `Authorization: Bearer <access secret>`. The payload rules
of spec 83 feature 05 section 6.4 apply: a pinned certificate and a host without
the signed-secret mode are refused before the first call.
