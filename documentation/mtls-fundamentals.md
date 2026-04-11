# Mutual TLS (mTLS) Fundamentals and Rust Implementation

## Table of Contents

1. [Overview](#overview)
2. [TLS vs mTLS](#tls-vs-mtls)
3. [Certificate Chain of Trust](#certificate-chain-of-trust)
4. [The mTLS Handshake](#the-mtls-handshake)
5. [Certificate Management](#certificate-management)
6. [Rust Implementation](#rust-implementation)
7. [Certificate Generation (OpenSSL)](#certificate-generation-openssl)
8. [Common Pitfalls](#common-pitfalls)
9. [Production Considerations](#production-considerations)

---

## Overview

Mutual TLS (mTLS) is an extension of TLS where **both** the client and server authenticate each other using X.509 certificates. In standard TLS, only the server presents a certificate; in mTLS, the client must also present a valid certificate that the server trusts.

### When to Use mTLS

- **Service-to-service communication** in microservice architectures
- **Zero-trust networks** where identity must be cryptographically proven
- **API authentication** replacing or supplementing API keys/tokens
- **IoT device authentication** where devices need strong identity
- **Internal infrastructure** (databases, message brokers, control planes)

### Key Properties

| Property | Standard TLS | mTLS |
|---|---|---|
| Server authenticated | Yes | Yes |
| Client authenticated | No (app-layer) | Yes (transport-layer) |
| Client certificate required | No | Yes |
| Man-in-the-middle protection | Yes | Yes |
| Client identity at transport layer | No | Yes |

---

## TLS vs mTLS

### Standard TLS (One-Way)

```
Client                          Server
  |                                |
  |  --- ClientHello ----------->  |
  |  <-- ServerHello -----------  |
  |  <-- Server Certificate ----  |  Server proves identity
  |  <-- ServerHelloDone -------  |
  |  --- ClientKeyExchange ----->  |
  |  --- ChangeCipherSpec ------>  |
  |  --- Finished -------------->  |
  |  <-- ChangeCipherSpec ------  |
  |  <-- Finished -------------  |
  |                                |
  |  ===== Encrypted Channel ====  |
```

The client verifies the server's certificate against its trust store, but the server has no cryptographic proof of the client's identity. Authentication happens at the application layer (cookies, tokens, API keys).

### mTLS (Two-Way)

```
Client                          Server
  |                                |
  |  --- ClientHello ----------->  |
  |  <-- ServerHello -----------  |
  |  <-- Server Certificate ----  |  Server proves identity
  |  <-- CertificateRequest ----  |  Server demands client cert
  |  <-- ServerHelloDone -------  |
  |  --- Client Certificate ---->  |  Client proves identity
  |  --- ClientKeyExchange ----->  |
  |  --- CertificateVerify ----->  |  Client proves key ownership
  |  --- ChangeCipherSpec ------>  |
  |  --- Finished -------------->  |
  |  <-- ChangeCipherSpec ------  |
  |  <-- Finished -------------  |
  |                                |
  |  ===== Encrypted Channel ====  |
```

The critical additions are:
1. **CertificateRequest** - Server tells the client it must present a certificate
2. **Client Certificate** - Client sends its X.509 certificate
3. **CertificateVerify** - Client signs a hash of handshake messages with its private key, proving it owns the certificate

---

## Certificate Chain of Trust

### PKI Hierarchy

```
Root CA (self-signed, offline, long-lived)
  |
  +-- Intermediate CA (signed by Root, medium-lived)
  |     |
  |     +-- Server Certificate (signed by Intermediate, short-lived)
  |     +-- Client Certificate (signed by Intermediate, short-lived)
  |
  +-- Intermediate CA 2 (optional: separate CA for clients vs servers)
        |
        +-- Client Certificate (signed by Intermediate 2)
```

### Certificate Fields That Matter for mTLS

```
Certificate:
    Version: 3
    Serial Number: <unique per cert>
    Issuer: CN=My Intermediate CA, O=My Org          # Who signed this cert
    Validity:
        Not Before: Apr 11 00:00:00 2026 GMT
        Not After:  Apr 11 00:00:00 2027 GMT
    Subject: CN=my-service, O=My Org                  # Identity of this cert
    Subject Public Key Info:
        Public Key Algorithm: id-ecPublicKey
        EC Public Key: (P-256 curve)
    X509v3 Extensions:
        X509v3 Key Usage: critical
            Digital Signature, Key Encipherment
        X509v3 Extended Key Usage:
            TLS Web Server Authentication              # For server certs
            TLS Web Client Authentication              # For client certs
        X509v3 Subject Alternative Name:
            DNS:my-service.internal                    # SAN: hostname matching
        X509v3 Basic Constraints: critical
            CA:FALSE                                   # Not a CA certificate
```

### Important Extension Details

- **Key Usage**: `Digital Signature` is required for ECDSA/EdDSA key exchange; `Key Encipherment` for RSA
- **Extended Key Usage (EKU)**:
  - Server certs need `TLS Web Server Authentication` (OID 1.3.6.1.5.5.7.3.1)
  - Client certs need `TLS Web Client Authentication` (OID 1.3.6.1.5.5.7.3.2)
  - A cert can have both if it acts as both client and server
- **Subject Alternative Name (SAN)**: Modern TLS implementations match hostnames against SAN, not CN. Always set SAN.
- **Basic Constraints**: `CA:FALSE` prevents leaf certificates from being used to sign other certificates

---

## The mTLS Handshake

### TLS 1.3 mTLS Handshake (Modern)

TLS 1.3 simplifies and secures the handshake:

```
Client                                    Server

ClientHello
  + key_share
  + supported_versions
  + signature_algorithms       -------->
                                          ServerHello
                                            + key_share
                                            + supported_versions
                                          {EncryptedExtensions}
                                          {CertificateRequest}
                                          {Certificate}
                                          {CertificateVerify}
                               <--------  {Finished}

  {Certificate}
  {CertificateVerify}
  {Finished}                   -------->

  [Application Data]          <------->   [Application Data]
```

`{}` = encrypted with handshake keys
`[]` = encrypted with application keys

Key differences from TLS 1.2:
- Only 1 round trip (1-RTT) instead of 2
- Handshake messages after ServerHello are encrypted
- Removed vulnerable cipher suites, only AEAD ciphers remain
- Forward secrecy is mandatory (ephemeral key exchange only)

### Verification Steps (Both Sides)

**Server verifying client certificate:**
1. Is the certificate well-formed and not expired?
2. Is the certificate signed by a trusted CA (or chain leads to one)?
3. Is the certificate revoked? (CRL or OCSP check)
4. Does the certificate have the `TLS Web Client Authentication` EKU?
5. Does the `CertificateVerify` signature validate against the certificate's public key?
6. (Optional) Does the CN/SAN match an expected identity?

**Client verifying server certificate:**
1. Same checks as above, but with `TLS Web Server Authentication` EKU
2. Does the SAN match the hostname being connected to?

---

## Certificate Management

### Certificate Lifecycle

```
 Generate Key Pair
       |
       v
 Create CSR (Certificate Signing Request)
       |
       v
 CA Signs CSR -> Certificate Issued
       |
       v
 Deploy Certificate + Key
       |
       v
 Monitor Expiration
       |
       v
 Rotate (generate new key + CSR before expiry)
       |
       v
 Revoke Old Certificate (CRL / OCSP)
```

### Rotation Strategies

1. **Overlap rotation**: Issue new cert before old one expires. Run both temporarily.
2. **Short-lived certificates**: Issue certs with 24h-72h validity. No revocation needed - they expire naturally. (Used by SPIFFE/SPIRE, Istio, Vault PKI)
3. **Certificate hot-reloading**: Watch the filesystem or a signal to reload certs without restarting the process.

---

## Rust Implementation

### Crate Ecosystem

| Crate | Role | Notes |
|---|---|---|
| `rustls` | Pure-Rust TLS implementation | No OpenSSL dependency, memory-safe |
| `tokio-rustls` | Async TLS streams for Tokio | Wraps `rustls` for async I/O |
| `rcgen` | Certificate generation | Useful for tests and dev environments |
| `x509-parser` | Parse and inspect X.509 certs | Read cert fields programmatically |
| `webpki` | Certificate verification | Used internally by `rustls` |
| `rustls-pemfile` | PEM file parsing | Load certs/keys from PEM files |

### Dependencies (Cargo.toml)

```toml
[dependencies]
rustls = { version = "0.23", features = ["ring"] }   # or aws-lc-rs backend
tokio-rustls = "0.26"
tokio = { version = "1", features = ["full"] }
rustls-pemfile = "2"
rcgen = "0.13"                                         # For cert generation in tests

# For HTTP-level mTLS
hyper = { version = "1", features = ["http1", "http2", "server", "client"] }
hyper-rustls = "0.27"
hyper-util = { version = "0.1", features = ["full"] }
```

### Loading Certificates and Keys

```rust
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

/// Load a certificate chain from a PEM file.
/// The file may contain multiple certificates (leaf + intermediates).
fn load_certs(path: &Path) -> Vec<CertificateDer<'static>> {
    let file = File::open(path)
        .unwrap_or_else(|e| panic!("failed to open cert file {}: {e}", path.display()));
    let mut reader = BufReader::new(file);

    rustls_pemfile::certs(&mut reader)
        .collect::<Result<Vec<_>, _>>()
        .unwrap_or_else(|e| panic!("failed to parse certs from {}: {e}", path.display()))
}

/// Load a private key from a PEM file.
/// Supports PKCS#8, RSA, and EC key formats.
fn load_private_key(path: &Path) -> PrivateKeyDer<'static> {
    let file = File::open(path)
        .unwrap_or_else(|e| panic!("failed to open key file {}: {e}", path.display()));
    let mut reader = BufReader::new(file);

    rustls_pemfile::private_key(&mut reader)
        .expect("failed to parse private key")
        .unwrap_or_else(|| panic!("no private key found in {}", path.display()))
}

/// Load a CA certificate bundle for use as a trust anchor.
fn load_ca_certs(path: &Path) -> rustls::RootCertStore {
    let certs = load_certs(path);
    let mut root_store = rustls::RootCertStore::empty();
    for cert in certs {
        root_store.add(cert).expect("failed to add CA certificate");
    }
    root_store
}
```

### mTLS Server

```rust
use rustls::server::WebPkiClientVerifier;
use rustls::ServerConfig;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;

async fn run_mtls_server() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Load the CA certificate(s) that signed client certificates.
    //    Only clients presenting a cert signed by this CA will be accepted.
    let client_ca_store = load_ca_certs(Path::new("certs/ca.pem"));

    // 2. Build a client certificate verifier.
    //    This enforces that clients MUST present a valid certificate.
    let client_verifier = WebPkiClientVerifier::builder(Arc::new(client_ca_store))
        .build()
        .expect("failed to build client verifier");

    // 3. Load the server's own certificate chain and private key.
    let server_certs = load_certs(Path::new("certs/server.pem"));
    let server_key = load_private_key(Path::new("certs/server-key.pem"));

    // 4. Build the TLS server configuration.
    let tls_config = ServerConfig::builder()
        .with_client_cert_verifier(client_verifier)    // Require client certs
        .with_single_cert(server_certs, server_key)
        .expect("failed to build server TLS config");

    let acceptor = TlsAcceptor::from(Arc::new(tls_config));

    // 5. Bind and accept connections.
    let listener = TcpListener::bind("0.0.0.0:8443").await?;
    println!("mTLS server listening on :8443");

    loop {
        let (tcp_stream, peer_addr) = listener.accept().await?;
        let acceptor = acceptor.clone();

        tokio::spawn(async move {
            match acceptor.accept(tcp_stream).await {
                Ok(tls_stream) => {
                    // Connection established with verified client.
                    // Extract client certificate info:
                    let (_, server_conn) = tls_stream.get_ref();
                    if let Some(certs) = server_conn.peer_certificates() {
                        println!(
                            "Client {} presented {} certificate(s)",
                            peer_addr,
                            certs.len()
                        );
                    }

                    // Handle the connection (read/write on tls_stream)...
                    handle_connection(tls_stream).await;
                }
                Err(e) => {
                    // Client failed to present a valid certificate,
                    // or TLS handshake failed for another reason.
                    eprintln!("TLS handshake failed from {}: {}", peer_addr, e);
                }
            }
        });
    }
}
```

### mTLS Client

```rust
use rustls::ClientConfig;
use rustls::pki_types::ServerName;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;

async fn connect_mtls_client() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Load the CA certificate(s) that signed the server's certificate.
    let server_ca_store = load_ca_certs(Path::new("certs/ca.pem"));

    // 2. Load the client's own certificate and private key.
    let client_certs = load_certs(Path::new("certs/client.pem"));
    let client_key = load_private_key(Path::new("certs/client-key.pem"));

    // 3. Build the TLS client configuration with client certificate.
    let tls_config = ClientConfig::builder()
        .with_root_certificates(server_ca_store)          // Trust server's CA
        .with_client_auth_cert(client_certs, client_key)  // Present client cert
        .expect("failed to build client TLS config");

    let connector = TlsConnector::from(Arc::new(tls_config));

    // 4. Connect to the server.
    let tcp_stream = TcpStream::connect("127.0.0.1:8443").await?;
    let server_name = ServerName::try_from("my-service.internal")?;

    let tls_stream = connector.connect(server_name, tcp_stream).await?;

    println!("mTLS connection established");

    // 5. Use tls_stream for application-level I/O...
    // tls_stream implements AsyncRead + AsyncWrite.

    Ok(())
}
```

### Extracting Client Identity from the Certificate

```rust
use x509_parser::prelude::*;

/// Extract the Common Name (CN) from a client's certificate.
fn extract_client_cn(cert_der: &[u8]) -> Option<String> {
    let (_, cert) = X509Certificate::from_der(cert_der).ok()?;

    cert.subject()
        .iter_common_name()
        .next()
        .and_then(|cn| cn.as_str().ok())
        .map(|s| s.to_string())
}

/// Extract all Subject Alternative Names from a certificate.
fn extract_sans(cert_der: &[u8]) -> Vec<String> {
    let (_, cert) = match X509Certificate::from_der(cert_der) {
        Ok(parsed) => parsed,
        Err(_) => return vec![],
    };

    let mut sans = Vec::new();
    if let Ok(Some(san_ext)) = cert.subject_alternative_name() {
        for name in &san_ext.value.general_names {
            match name {
                GeneralName::DNSName(dns) => sans.push(dns.to_string()),
                GeneralName::RFC822Name(email) => sans.push(email.to_string()),
                GeneralName::URI(uri) => sans.push(uri.to_string()),
                _ => {}
            }
        }
    }
    sans
}
```

### Custom Client Certificate Verifier

For cases where you need policy beyond standard PKI validation (e.g., checking specific SANs, enforcing naming conventions, or integrating with an external authorization system):

```rust
use rustls::server::danger::{ClientCertVerified, ClientCertVerifier};
use rustls::pki_types::{CertificateDer, UnixTime};
use rustls::{DigitallySignedStruct, DistinguishedName, Error, SignatureScheme};

#[derive(Debug)]
struct PolicyCheckingVerifier {
    /// The inner verifier handles cryptographic validation.
    inner: Arc<dyn ClientCertVerifier>,
    /// Allowed CN patterns.
    allowed_cn_patterns: Vec<String>,
}

impl ClientCertVerifier for PolicyCheckingVerifier {
    fn root_hint_subjects(&self) -> &[DistinguishedName] {
        self.inner.root_hint_subjects()
    }

    fn verify_client_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        now: UnixTime,
    ) -> Result<ClientCertVerified, Error> {
        // Step 1: Delegate cryptographic verification to the inner verifier.
        self.inner.verify_client_cert(end_entity, intermediates, now)?;

        // Step 2: Apply custom policy checks.
        let cn = extract_client_cn(end_entity.as_ref())
            .ok_or_else(|| Error::General("client cert has no CN".into()))?;

        let allowed = self.allowed_cn_patterns.iter().any(|pattern| {
            cn == *pattern || cn.ends_with(pattern)
        });

        if !allowed {
            return Err(Error::General(
                format!("client CN '{}' is not in the allow list", cn),
            ));
        }

        Ok(ClientCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, Error> {
        self.inner.verify_tls12_signature(message, cert, dss)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, Error> {
        self.inner.verify_tls13_signature(message, cert, dss)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.inner.supported_verify_schemes()
    }
}
```

### Certificate Hot-Reloading

For long-running servers that need to rotate certificates without restart:

```rust
use rustls::server::{ClientHello, ResolvesServerCert};
use rustls::sign::CertifiedKey;
use std::sync::RwLock;

/// A cert resolver that can be updated at runtime.
struct ReloadableResolver {
    certified_key: RwLock<Arc<CertifiedKey>>,
}

impl ReloadableResolver {
    fn new(certs: Vec<CertificateDer<'static>>, key: PrivateKeyDer<'static>) -> Self {
        let signing_key = rustls::crypto::ring::sign::any_supported_type(&key)
            .expect("failed to create signing key");
        let certified_key = Arc::new(CertifiedKey::new(certs, signing_key));
        Self {
            certified_key: RwLock::new(certified_key),
        }
    }

    /// Call this when certificates have been rotated on disk.
    fn reload(&self, certs: Vec<CertificateDer<'static>>, key: PrivateKeyDer<'static>) {
        let signing_key = rustls::crypto::ring::sign::any_supported_type(&key)
            .expect("failed to create signing key");
        let new_key = Arc::new(CertifiedKey::new(certs, signing_key));
        *self.certified_key.write().unwrap() = new_key;
    }
}

impl ResolvesServerCert for ReloadableResolver {
    fn resolve(&self, _client_hello: ClientHello<'_>) -> Option<Arc<CertifiedKey>> {
        Some(self.certified_key.read().unwrap().clone())
    }
}

// Usage in server config:
// let resolver = Arc::new(ReloadableResolver::new(server_certs, server_key));
//
// let tls_config = ServerConfig::builder()
//     .with_client_cert_verifier(client_verifier)
//     .with_cert_resolver(resolver.clone());
//
// // In a background task, watch for file changes:
// tokio::spawn(async move {
//     let mut watcher = notify::recommended_watcher(move |_| {
//         let certs = load_certs(Path::new("certs/server.pem"));
//         let key = load_private_key(Path::new("certs/server-key.pem"));
//         resolver.reload(certs, key);
//     }).unwrap();
//     watcher.watch(Path::new("certs/"), notify::RecursiveMode::NonRecursive).unwrap();
// });
```

### Test Helpers: Generating Certificates with rcgen

```rust
#[cfg(test)]
mod tests {
    use rcgen::{
        BasicConstraints, CertificateParams, DnType, ExtendedKeyUsagePurpose, IsCa,
        KeyPair, KeyUsagePurpose,
    };
    use std::time::Duration;

    struct CertBundle {
        ca_cert_pem: String,
        server_cert_pem: String,
        server_key_pem: String,
        client_cert_pem: String,
        client_key_pem: String,
    }

    fn generate_test_certs() -> CertBundle {
        // --- Root CA ---
        let mut ca_params = CertificateParams::new(vec![]).unwrap();
        ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        ca_params.distinguished_name.push(DnType::CommonName, "Test CA");
        ca_params.distinguished_name.push(DnType::OrganizationName, "Test Org");
        ca_params.key_usages.push(KeyUsagePurpose::KeyCertSign);
        ca_params.key_usages.push(KeyUsagePurpose::CrlSign);
        ca_params.not_after = rcgen::date_time_ymd(2030, 1, 1);

        let ca_key = KeyPair::generate().unwrap();
        let ca_cert = ca_params.self_signed(&ca_key).unwrap();

        // --- Server Certificate ---
        let mut server_params = CertificateParams::new(
            vec!["localhost".to_string(), "127.0.0.1".to_string()]
        ).unwrap();
        server_params.distinguished_name.push(DnType::CommonName, "test-server");
        server_params.extended_key_usages.push(ExtendedKeyUsagePurpose::ServerAuth);
        server_params.key_usages.push(KeyUsagePurpose::DigitalSignature);
        server_params.not_after = rcgen::date_time_ymd(2028, 1, 1);

        let server_key = KeyPair::generate().unwrap();
        let server_cert = server_params.signed_by(&server_key, &ca_cert, &ca_key).unwrap();

        // --- Client Certificate ---
        let mut client_params = CertificateParams::new(vec![]).unwrap();
        client_params.distinguished_name.push(DnType::CommonName, "test-client");
        client_params.extended_key_usages.push(ExtendedKeyUsagePurpose::ClientAuth);
        client_params.key_usages.push(KeyUsagePurpose::DigitalSignature);
        client_params.not_after = rcgen::date_time_ymd(2028, 1, 1);

        let client_key = KeyPair::generate().unwrap();
        let client_cert = client_params.signed_by(&client_key, &ca_cert, &ca_key).unwrap();

        CertBundle {
            ca_cert_pem: ca_cert.pem(),
            server_cert_pem: server_cert.pem(),
            server_key_pem: server_key.serialize_pem(),
            client_cert_pem: client_cert.pem(),
            client_key_pem: client_key.serialize_pem(),
        }
    }

    #[tokio::test]
    async fn test_mtls_handshake() {
        let certs = generate_test_certs();

        // Parse certs from PEM strings into rustls types...
        // Build server config, build client config, connect, assert success.
        // (Full working test would follow the server/client patterns above,
        //  loading from the PEM strings instead of files.)
    }
}
```

---

## Certificate Generation (OpenSSL)

Reference commands for generating a full mTLS PKI for development and testing.

### 1. Generate Root CA

```bash
# Generate CA private key (EC P-256, no passphrase for dev)
openssl ecparam -genkey -name prime256v1 -noout -out ca-key.pem

# Generate self-signed CA certificate (10 year validity)
openssl req -new -x509 -key ca-key.pem -out ca.pem -days 3650 \
  -subj "/CN=Dev Root CA/O=My Org"
```

### 2. Generate Server Certificate

```bash
# Generate server key
openssl ecparam -genkey -name prime256v1 -noout -out server-key.pem

# Create CSR
openssl req -new -key server-key.pem -out server.csr \
  -subj "/CN=my-service.internal/O=My Org"

# Create extension file for SAN
cat > server-ext.cnf << 'EOF'
[v3_ext]
basicConstraints = critical, CA:FALSE
keyUsage = critical, digitalSignature
extendedKeyUsage = serverAuth
subjectAltName = DNS:my-service.internal, DNS:localhost, IP:127.0.0.1
EOF

# Sign with CA
openssl x509 -req -in server.csr -CA ca.pem -CAkey ca-key.pem \
  -CAcreateserial -out server.pem -days 365 \
  -extfile server-ext.cnf -extensions v3_ext
```

### 3. Generate Client Certificate

```bash
# Generate client key
openssl ecparam -genkey -name prime256v1 -noout -out client-key.pem

# Create CSR
openssl req -new -key client-key.pem -out client.csr \
  -subj "/CN=my-client-service/O=My Org"

# Create extension file
cat > client-ext.cnf << 'EOF'
[v3_ext]
basicConstraints = critical, CA:FALSE
keyUsage = critical, digitalSignature
extendedKeyUsage = clientAuth
EOF

# Sign with CA
openssl x509 -req -in client.csr -CA ca.pem -CAkey ca-key.pem \
  -CAcreateserial -out client.pem -days 365 \
  -extfile client-ext.cnf -extensions v3_ext
```

### 4. Verify Certificates

```bash
# Verify server cert against CA
openssl verify -CAfile ca.pem server.pem

# Verify client cert against CA
openssl verify -CAfile ca.pem client.pem

# Inspect a certificate
openssl x509 -in server.pem -text -noout

# Test mTLS connection (requires a running server)
openssl s_client -connect localhost:8443 \
  -cert client.pem -key client-key.pem -CAfile ca.pem
```

---

## Common Pitfalls

### 1. Certificate Chain Ordering

PEM files must contain certificates in order: **leaf first, then intermediates, root last** (or root omitted). A reversed chain causes handshake failures with opaque errors.

```
# Correct order in server.pem:
-----BEGIN CERTIFICATE-----
<server leaf certificate>
-----END CERTIFICATE-----
-----BEGIN CERTIFICATE-----
<intermediate CA certificate>
-----END CERTIFICATE-----
```

### 2. Missing Extended Key Usage

If a client cert lacks `clientAuth` EKU, or a server cert lacks `serverAuth` EKU, rustls will reject it. The error message may not clearly indicate this is the problem. Always set EKU explicitly.

### 3. SAN vs CN Matching

Modern TLS libraries (including rustls/webpki) match hostnames against Subject Alternative Name (SAN), **not** Common Name (CN). A certificate with only a CN and no SAN will fail hostname verification. Always include a SAN.

### 4. Clock Skew

Certificate validity is checked against the system clock. If a server or client has clock skew exceeding the certificate's `Not Before` / `Not After` window, the handshake fails. Use NTP in production.

### 5. Private Key Permissions

Private keys must be readable only by the process owner. On Unix:

```bash
chmod 600 server-key.pem client-key.pem
```

### 6. CA Certificate vs Leaf Certificate Confusion

Using a CA certificate as a server/client leaf cert (or vice versa) will fail. Check `Basic Constraints: CA:TRUE/FALSE` to verify you have the right cert type.

### 7. rustls Does Not Support All Cipher Suites

`rustls` deliberately excludes legacy/weak cipher suites (CBC, RC4, 3DES, static RSA key exchange). If the peer only supports these, the handshake will fail. This is generally a good thing - update the peer instead.

---

## Production Considerations

### Certificate Rotation

- Automate certificate issuance and renewal (Vault PKI, cert-manager, ACME)
- Use short-lived certificates (hours to days) to minimize blast radius of key compromise
- Implement hot-reloading (see above) to avoid downtime during rotation
- Monitor certificate expiration with alerting (Prometheus `x509_cert_not_after` metric)

### Revocation

- **CRL (Certificate Revocation List)**: Periodically published list of revoked serial numbers. Can become large.
- **OCSP (Online Certificate Status Protocol)**: Real-time check per certificate. Adds latency and a dependency.
- **OCSP Stapling**: Server fetches its own OCSP response and staples it to the handshake. Reduces client-side latency.
- **Short-lived certs**: The pragmatic alternative - if certs expire in 24h, revocation is less critical.

Note: `rustls` supports CRL checking. OCSP stapling support is available but requires additional configuration.

### Performance

- **Session resumption**: TLS 1.3 supports 0-RTT resumption, reducing handshake overhead for repeat connections. `rustls` supports this via `ServerConfig::session_storage`.
- **Connection pooling**: Reuse TLS connections where possible. Each handshake involves public key operations.
- **Key algorithm choice**: ECDSA P-256 keys are faster to verify than RSA-2048/4096. Ed25519 is fastest but has less ecosystem support.
- **Crypto backend**: `rustls` supports `ring` (default) and `aws-lc-rs`. The latter uses assembly-optimized code and can be faster for high-throughput servers.

### Observability

- Log TLS handshake failures with peer address and error reason
- Track certificate expiration as a metric
- Record the client certificate CN/SAN in request metadata for audit trails
- Alert on unexpected certificate verification failures (may indicate misconfiguration or attack)

### Defense in Depth

mTLS provides transport-layer authentication but should be combined with:

- **Authorization**: mTLS proves identity, not permissions. Use the client's CN/SAN to look up their allowed operations.
- **Network segmentation**: mTLS doesn't replace firewalls. Limit which networks can reach the TLS listener.
- **Encryption at rest**: mTLS protects data in transit. Sensitive data at rest needs separate encryption.
- **Audit logging**: Record which client identity performed which operations.
