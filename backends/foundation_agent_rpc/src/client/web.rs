//! The browser client (spec 83 feature 05, section 6).
//!
//! WHY: a page is WebAssembly compiled by `wasm-bindgen`. It calls a Worker or
//! a native host over Connect with the browser `fetch`. It shows a bearer
//! secret on each call, because a page cannot choose a client certificate.
//!
//! WHAT: [`WebClient`], its [`WebCredentialStore`] ([`IndexedDbCredentials`]
//! by default), [`WebIdentity`] and [`WebKeyPair`].
//!
//! HOW: enrollment makes a non-extractable WebCrypto key, calls `Enroll` with
//! its public key, checks the server proof with the payload public key and
//! saves the identity. `resume` signs a nonce with the saved key and calls
//! `RefreshAccess`. A call refreshes an access secret before it expires, and
//! once more after an `UNAUTHENTICATED` reply. [`H1Transport::default`] gives
//! the browser `fetch` client.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use foundation_auth::secret::{DecodeLimits, SignedSecret};
use foundation_compact::SendWrapper;
use foundation_connectrpc::{
    ClientOptions, Code, ConnectError, H1Transport, Request, RpcCtx, ServerStream,
};
use foundation_context::Ctx;
use foundation_errstacks::ErrorTrace;
use js_sys::{Array, Object, Reflect, Uint8Array};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use zeroize::Zeroizing;

use crate::core::bootstrap::{decode, BootstrapAccess};
use crate::core::browser::check_browser_payload;
use crate::core::model::{Device, ModelKind};
use crate::core::registry::{InstanceInfo, InstanceSpec, TurnEventStream};
use crate::error::{AgentRpcError, AgentRpcResult};
use crate::proto::v1 as pb;
use crate::proto::v1::agent_service::AgentServiceClient;
use crate::proto::v1::bootstrap_service::BootstrapServiceClient;

/// The refresh domain of a client-key signature.
const REFRESH_DOMAIN: &[u8] = b"ewe.agent_rpc.refresh.v1\0";

fn config(what: &str, error: impl std::fmt::Debug) -> ErrorTrace<AgentRpcError> {
    ErrorTrace::new(AgentRpcError::Config(format!("{what}: {error:?}")))
}

fn map_error(error: &ErrorTrace<ConnectError>) -> ErrorTrace<AgentRpcError> {
    let wire = error.current_context();
    if wire.code() == Code::Unauthenticated {
        ErrorTrace::new(AgentRpcError::Auth(wire.message().to_string()))
    } else {
        ErrorTrace::new(AgentRpcError::Transport(wire.to_string()))
    }
}

/// A non-extractable WebCrypto key and its public half.
pub struct WebKeyPair {
    alg: String,
    public_key: Vec<u8>,
    private_key: web_sys::CryptoKey,
}

impl std::fmt::Debug for WebKeyPair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebKeyPair")
            .field("alg", &self.alg)
            .finish_non_exhaustive()
    }
}

impl WebKeyPair {
    /// Make a key: Ed25519, or P-256 where the browser has no Ed25519.
    ///
    /// The private key is imported again with `extractable = false`, so the
    /// page cannot read its bytes.
    ///
    /// # Errors
    /// [`AgentRpcError::Config`] when WebCrypto cannot make a key.
    pub async fn generate() -> AgentRpcResult<Self> {
        match generate_key("Ed25519", None).await {
            Ok(key) => Ok(key),
            Err(_) => generate_key("ECDSA", Some("P-256")).await,
        }
    }

    /// "ed25519" or "p256".
    #[must_use]
    pub fn alg(&self) -> &str {
        &self.alg
    }

    /// The raw public key: 32 bytes for Ed25519, 65 for P-256.
    #[must_use]
    pub fn public_key(&self) -> &[u8] {
        &self.public_key
    }

    /// The non-extractable private key, for a page that keeps it.
    #[must_use]
    pub fn private_key(&self) -> &web_sys::CryptoKey {
        &self.private_key
    }

    /// Sign `message` with the private key.
    ///
    /// # Errors
    /// [`AgentRpcError::Auth`] when the browser refuses the signature.
    pub async fn sign(&self, message: &[u8]) -> AgentRpcResult<Vec<u8>> {
        let algorithm = if self.alg == "ed25519" {
            algorithm_object("Ed25519", None)
        } else {
            ecdsa_algorithm()
        };
        let subtle = subtle()?;
        let promise = subtle
            .sign_with_object_and_u8_array(&algorithm, &self.private_key, message)
            .map_err(|error| config("the signature", error))?;
        let signature = SendWrapper::new(JsFuture::from(promise))
            .await
            .map_err(|error| config("the signature", error))?;
        Ok(Uint8Array::new(&signature).to_vec())
    }
}

async fn generate_key(name: &str, curve: Option<&str>) -> AgentRpcResult<WebKeyPair> {
    let subtle = subtle()?;
    let algorithm = if name == "Ed25519" {
        algorithm_object("Ed25519", None)
    } else {
        algorithm_object("ECDSA", curve)
    };
    let usages = Array::new();
    usages.push(&JsValue::from_str("sign"));
    let pair = SendWrapper::new(JsFuture::from(
        subtle
            .generate_key_with_object(&algorithm, true, &usages)
            .map_err(|error| config("the key", error))?,
    ))
    .await
    .map_err(|error| config("the key", error))?;
    let pair: web_sys::CryptoKeyPair = pair.unchecked_into();
    let raw = SendWrapper::new(JsFuture::from(
        subtle
            .export_key("raw", &pair.get_public_key())
            .map_err(|error| config("the public key", error))?,
    ))
    .await
    .map_err(|error| config("the public key", error))?;
    let public_key = Uint8Array::new(&raw).to_vec();
    let jwk = SendWrapper::new(JsFuture::from(
        subtle
            .export_key("jwk", &pair.get_private_key())
            .map_err(|error| config("the private key", error))?,
    ))
    .await
    .map_err(|error| config("the private key", error))?;
    let jwk: Object = jwk.unchecked_into();
    let private = SendWrapper::new(JsFuture::from(
        subtle
            .import_key_with_object("jwk", &jwk, &algorithm, false, &usages)
            .map_err(|error| config("the private key", error))?,
    ))
    .await
    .map_err(|error| config("the private key", error))?;
    Ok(WebKeyPair {
        alg: if name == "Ed25519" { "ed25519" } else { "p256" }.to_string(),
        public_key,
        private_key: private.unchecked_into(),
    })
}

fn subtle() -> AgentRpcResult<web_sys::SubtleCrypto> {
    let global = js_sys::global();
    let crypto = Reflect::get(&global, &JsValue::from_str("crypto"))
        .map_err(|error| config("the WebCrypto API", error))?;
    let crypto: web_sys::Crypto = crypto
        .dyn_into()
        .map_err(|_| config("the WebCrypto API", "this host has no crypto"))?;
    Ok(crypto.subtle())
}

fn algorithm_object(name: &str, curve: Option<&str>) -> Object {
    let object = Object::new();
    let _ = Reflect::set(&object, &"name".into(), &JsValue::from_str(name));
    if let Some(curve) = curve {
        let _ = Reflect::set(&object, &"namedCurve".into(), &JsValue::from_str(curve));
    }
    object
}

fn ecdsa_algorithm() -> Object {
    let object = algorithm_object("ECDSA", None);
    let _ = Reflect::set(&object, &"hash".into(), &JsValue::from_str("SHA-256"));
    object
}

fn random_nonce() -> AgentRpcResult<[u8; 32]> {
    let mut nonce = [0u8; 32];
    let global = js_sys::global();
    let crypto = Reflect::get(&global, &JsValue::from_str("crypto"))
        .map_err(|error| config("the entropy source", error))?;
    let crypto: web_sys::Crypto = crypto
        .dyn_into()
        .map_err(|_| config("the entropy source", "this host has no crypto"))?;
    crypto
        .get_random_values_with_u8_array(&mut nonce)
        .map_err(|error| config("the nonce", error))?;
    Ok(nonce)
}

/// The enrolled identity of one page at one host.
#[derive(Clone)]
pub struct WebIdentity {
    /// The `AgentService` base URL.
    pub url: String,
    /// The `BootstrapService` base URL, for a refresh.
    pub enroll_url: String,
    /// The client ID that the host assigned.
    pub client_id: String,
    /// "ed25519" or "p256".
    pub alg: String,
    /// The raw public key.
    pub public_key: Vec<u8>,
    /// The non-extractable private key.
    pub private_key: web_sys::CryptoKey,
}

impl std::fmt::Debug for WebIdentity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebIdentity")
            .field("url", &self.url)
            .field("client_id", &self.client_id)
            .field("alg", &self.alg)
            .finish_non_exhaustive()
    }
}

/// Where a page keeps its enrolled identity between visits.
#[async_trait(?Send)]
pub trait WebCredentialStore {
    /// The identity of `url`, when the page saved one.
    ///
    /// # Errors
    /// [`AgentRpcError::Storage`] when the store fails.
    async fn load(&self, url: &str) -> AgentRpcResult<Option<WebIdentity>>;

    /// Save the identity of `url`.
    ///
    /// # Errors
    /// [`AgentRpcError::Storage`] when the store fails.
    async fn save(&self, url: &str, identity: &WebIdentity) -> AgentRpcResult<()>;
}

/// The default store: IndexedDB, which keeps the non-extractable key.
pub struct IndexedDbCredentials;

/// The object store name.
const STORE: &str = "credential";

const DB_NAME: &str = "ewe-agent-rpc";

impl IndexedDbCredentials {
    async fn open(&self) -> AgentRpcResult<web_sys::IdbDatabase> {
        let window = web_sys::window().ok_or_else(|| {
            ErrorTrace::new(AgentRpcError::Storage(
                "IndexedDB needs a browser page".to_string(),
            ))
        })?;
        let factory = window
            .indexed_db()
            .map_err(|error| config("IndexedDB", error))?
            .ok_or_else(|| {
                ErrorTrace::new(AgentRpcError::Storage(
                    "this host has no IndexedDB".to_string(),
                ))
            })?;
        let request = factory
            .open(DB_NAME)
            .map_err(|error| config("IndexedDB", error))?;
        let upgrade = wasm_bindgen::closure::Closure::once_into_js(move |event: JsValue| {
            let target = Reflect::get(&event, &"target".into())
                .ok()
                .and_then(|value| value.dyn_into::<web_sys::IdbOpenDbRequest>().ok());
            if let Some(request) = target {
                if let Ok(result) = request.result() {
                    if let Ok(database) = result.dyn_into::<web_sys::IdbDatabase>() {
                        let _ = database.create_object_store(STORE);
                    }
                }
            }
        });
        request.set_onupgradeneeded(Some(upgrade.unchecked_ref()));
        let value = await_request(&request).await?;
        value
            .dyn_into::<web_sys::IdbDatabase>()
            .map_err(|_| config("IndexedDB", "the open request returned no database"))
    }

    async fn object_store(
        &self,
        mode: web_sys::IdbTransactionMode,
    ) -> AgentRpcResult<web_sys::IdbObjectStore> {
        let database = self.open().await?;
        let transaction = database
            .transaction_with_str_and_mode(STORE, mode)
            .map_err(|error| config("IndexedDB", error))?;
        transaction
            .object_store(STORE)
            .map_err(|error| config("IndexedDB", error))
    }
}

#[async_trait(?Send)]
impl WebCredentialStore for IndexedDbCredentials {
    async fn load(&self, url: &str) -> AgentRpcResult<Option<WebIdentity>> {
        let store = self
            .object_store(web_sys::IdbTransactionMode::Readonly)
            .await?;
        let request = store
            .get(&JsValue::from_str(url))
            .map_err(|error| config("IndexedDB", error))?;
        let value = await_request(&request).await?;
        if value.is_null() || value.is_undefined() {
            return Ok(None);
        }
        let object: Object = value.unchecked_into();
        let text = |name: &str| {
            Reflect::get(&object, &JsValue::from_str(name))
                .ok()
                .and_then(|value| value.as_string())
                .unwrap_or_default()
        };
        let public_key = Reflect::get(&object, &JsValue::from_str("public_key"))
            .ok()
            .map(|value| Uint8Array::new(&value).to_vec())
            .unwrap_or_default();
        let private_key = Reflect::get(&object, &JsValue::from_str("private_key"))
            .map_err(|error| config("IndexedDB", error))?
            .dyn_into::<web_sys::CryptoKey>()
            .map_err(|_| config("IndexedDB", "the saved key is not a CryptoKey"))?;
        Ok(Some(WebIdentity {
            url: text("url"),
            enroll_url: text("enroll_url"),
            client_id: text("client_id"),
            alg: text("alg"),
            public_key,
            private_key,
        }))
    }

    async fn save(&self, url: &str, identity: &WebIdentity) -> AgentRpcResult<()> {
        let store = self
            .object_store(web_sys::IdbTransactionMode::Readwrite)
            .await?;
        let object = Object::new();
        let set = |name: &str, value: &JsValue| {
            let _ = Reflect::set(&object, &JsValue::from_str(name), value);
        };
        set("url", &JsValue::from_str(url));
        set("enroll_url", &JsValue::from_str(&identity.enroll_url));
        set("client_id", &JsValue::from_str(&identity.client_id));
        set("alg", &JsValue::from_str(&identity.alg));
        set(
            "public_key",
            &Uint8Array::from(identity.public_key.as_slice()).into(),
        );
        set("private_key", &identity.private_key.clone().into());
        let value: JsValue = object.into();
        let request = store
            .put_with_key(&value, &JsValue::from_str(url))
            .map_err(|error| config("IndexedDB", error))?;
        await_request(&request).await.map(|_| ())
    }
}

/// An in-memory store, for a test and a page without IndexedDB.
#[derive(Default)]
pub struct MemoryCredentialStore {
    identity: Mutex<Option<WebIdentity>>,
}

#[async_trait(?Send)]
impl WebCredentialStore for MemoryCredentialStore {
    async fn load(&self, _url: &str) -> AgentRpcResult<Option<WebIdentity>> {
        Ok(self.identity.lock().ok().and_then(|slot| slot.clone()))
    }

    async fn save(&self, _url: &str, identity: &WebIdentity) -> AgentRpcResult<()> {
        if let Ok(mut slot) = self.identity.lock() {
            *slot = Some(identity.clone());
        }
        Ok(())
    }
}

/// Wait for one IndexedDB request and return its result.
async fn await_request(request: &web_sys::IdbRequest) -> AgentRpcResult<JsValue> {
    let (sender, receiver) = futures::channel::oneshot::channel();
    let sender = Arc::new(Mutex::new(Some(sender)));
    let on_success = {
        let sender = Arc::clone(&sender);
        let request = request.clone();
        wasm_bindgen::closure::Closure::once_into_js(move |_event: JsValue| {
            let value = request.result().unwrap_or(JsValue::NULL);
            if let Some(sender) = sender.lock().ok().and_then(|mut slot| slot.take()) {
                let _ = sender.send(value);
            }
        })
    };
    let on_error = {
        let sender = Arc::clone(&sender);
        wasm_bindgen::closure::Closure::once_into_js(move |_event: JsValue| {
            if let Some(sender) = sender.lock().ok().and_then(|mut slot| slot.take()) {
                let _ = sender.send(JsValue::UNDEFINED);
            }
        })
    };
    request.set_onsuccess(Some(on_success.unchecked_ref()));
    request.set_onerror(Some(on_error.unchecked_ref()));
    let value = receiver
        .await
        .map_err(|_| config("IndexedDB", "the request ended without a result"))?;
    if value.is_undefined() && !request.result().map(|v| v.is_undefined()).unwrap_or(true) {
        return Err(config("IndexedDB", "the request failed"));
    }
    Ok(value)
}

/// The access secret of the current window.
struct Access {
    secret: Zeroizing<String>,
    valid_from: u64,
    valid_until: u64,
}

/// A browser client over one host.
pub struct WebClient {
    url: String,
    enroll_url: Option<String>,
    ctx: Ctx,
    transport: Arc<H1Transport>,
    access: Mutex<Access>,
    identity: Mutex<Option<WebIdentity>>,
    bootstrap: Mutex<Option<Arc<BootstrapServiceClient>>>,
}

impl std::fmt::Debug for WebClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebClient")
            .field("url", &self.url)
            .finish_non_exhaustive()
    }
}

impl WebClient {
    /// Use an operator-issued payload whose token purpose is `Access`.
    ///
    /// # Errors
    /// `Codec` for a bad payload; `Config` for a payload that a browser cannot
    /// use; `Unauthenticated` for a token outside its window.
    pub fn from_access_payload(payload: &str) -> AgentRpcResult<Self> {
        let bootstrap = decode(payload)?;
        check_browser_payload(&bootstrap)?;
        let BootstrapAccess::Bearer { token } = bootstrap.access else {
            return Err(AgentRpcError::Config(
                "this payload enrolls a client; call enroll".to_string(),
            )
            .into());
        };
        let url = bootstrap.url.to_string();
        let transport = Arc::new(H1Transport::default());
        Ok(Self {
            access: Mutex::new(Access {
                secret: token.encode(),
                valid_from: token.valid_from(),
                valid_until: token.valid_until(),
            }),
            enroll_url: None,
            url,
            ctx: Ctx::host(),
            transport,
            identity: Mutex::new(None),
            bootstrap: Mutex::new(None),
        })
    }

    /// Enroll with a payload whose token purpose is `Enroll`.
    ///
    /// # Errors
    /// `Codec` for a bad payload; `Config` for a payload that a browser cannot
    /// use; `Unauthenticated` for a used, revoked or expired token, or a
    /// server proof that does not verify.
    pub async fn enroll(
        payload: &str,
        client_name: &str,
        store: &dyn WebCredentialStore,
    ) -> AgentRpcResult<Self> {
        let bootstrap = decode(payload)?;
        check_browser_payload(&bootstrap)?;
        let BootstrapAccess::Enroll { token, enroll_url } = bootstrap.access else {
            return Err(AgentRpcError::Config(
                "this payload is an access token; use from_access_payload".to_string(),
            )
            .into());
        };
        let key = WebKeyPair::generate().await?;
        let nonce = random_nonce()?;
        let url = bootstrap.url.to_string();
        let enroll_url = enroll_url.to_string();
        let transport = Arc::new(H1Transport::default());
        let client = Self::bootstrap_client(&transport, &enroll_url)?;
        let credential =
            pb::__buffa::oneof::enroll_request::Credential::ClientKey(Box::new(pb::ClientKey {
                alg: key.alg().to_string(),
                public_key: key.public_key().to_vec(),
                ..Default::default()
            }));
        let request = pb::EnrollRequest {
            token: token.encode().to_string(),
            client_name: client_name.to_string(),
            credential: Some(credential),
            nonce: nonce.to_vec(),
            ..Default::default()
        };
        let response =
            SendWrapper::new(client.enroll(RpcCtx::client(Ctx::host()), Request::new(request)))
                .await
                .map_err(|error| map_error(&error))?
                .msg;
        let proof_ok = token
            .verifying_key()
            .verify_enroll_proof(
                &token.id(),
                &nonce,
                key.public_key(),
                &response.client_id,
                &response.server_proof,
            )
            .map_err(|error| {
                ErrorTrace::new(AgentRpcError::Auth(format!(
                    "the server proof is unusable: {error}"
                )))
            })?;
        if !proof_ok {
            return Err(AgentRpcError::Auth("the server proof does not verify".to_string()).into());
        }
        let access = SignedSecret::decode(&response.access_secret, &DecodeLimits::default())
            .map_err(|error| {
                ErrorTrace::new(AgentRpcError::Auth(format!("the access secret: {error}")))
            })?;
        let identity = WebIdentity {
            url: url.clone(),
            enroll_url: enroll_url.clone(),
            client_id: response.client_id,
            alg: key.alg().to_string(),
            public_key: key.public_key().to_vec(),
            private_key: key.private_key,
        };
        store.save(&url, &identity).await?;
        Ok(Self {
            access: Mutex::new(Access {
                valid_from: access.valid_from(),
                valid_until: access.valid_until(),
                secret: access.encode(),
            }),
            enroll_url: Some(enroll_url),
            url,
            ctx: Ctx::host(),
            transport,
            identity: Mutex::new(Some(identity)),
            bootstrap: Mutex::new(None),
        })
    }

    /// Resume an identity that [`enroll`](Self::enroll) saved.
    ///
    /// Returns `None` when the store holds no identity for `url`.
    ///
    /// # Errors
    /// `Unauthenticated` for an unknown or revoked client, a bad signature or
    /// a replayed nonce; `Storage` for a store failure.
    pub async fn resume(url: &str, store: &dyn WebCredentialStore) -> AgentRpcResult<Option<Self>> {
        let Some(identity) = store.load(url).await? else {
            return Ok(None);
        };
        let transport = Arc::new(H1Transport::default());
        let client = Self {
            url: identity.url.clone(),
            enroll_url: Some(identity.enroll_url.clone()),
            ctx: Ctx::host(),
            transport,
            access: Mutex::new(Access {
                secret: Zeroizing::new(String::new()),
                valid_from: 0,
                valid_until: 0,
            }),
            identity: Mutex::new(Some(identity)),
            bootstrap: Mutex::new(None),
        };
        client.refresh().await?;
        Ok(Some(client))
    }

    /// The base URL of the host.
    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }

    /// The capability report of the host.
    ///
    /// # Errors
    /// The wire status of a failed call.
    pub async fn capabilities(&self) -> AgentRpcResult<pb::GetCapabilitiesResponse> {
        self.ensure_access().await?;
        let request = Request::new(pb::GetCapabilitiesRequest::default());
        let response = SendWrapper::new(
            self.agent()
                .get_capabilities(RpcCtx::client(self.ctx.clone()), request),
        )
        .await
        .map_err(|error| map_error(&error))?;
        Ok(response.msg)
    }

    /// Create one instance.
    ///
    /// # Errors
    /// The wire status of a failed call.
    pub async fn create_instance(&self, spec: InstanceSpec) -> AgentRpcResult<InstanceInfo> {
        self.ensure_access().await?;
        let request = Request::new(create_request(&spec));
        let response = SendWrapper::new(
            self.agent()
                .create_instance(RpcCtx::client(self.ctx.clone()), request),
        )
        .await
        .map_err(|error| map_error(&error))?;
        Ok(instance_info(response.msg))
    }

    /// Destroy one instance.
    ///
    /// # Errors
    /// The wire status of a failed call.
    pub async fn destroy_instance(&self, name: &str) -> AgentRpcResult<()> {
        self.ensure_access().await?;
        let request = Request::new(pb::DestroyInstanceRequest {
            name: name.to_string(),
            ..Default::default()
        });
        SendWrapper::new(
            self.agent()
                .destroy_instance(RpcCtx::client(self.ctx.clone()), request),
        )
        .await
        .map_err(|error| map_error(&error))?;
        Ok(())
    }

    /// One turn as a stream of events.
    ///
    /// Dropping the stream drops the body reader, which cancels the call.
    #[must_use]
    pub fn chat_stream(&self, instance: &str, prompt: &str) -> TurnEventStream {
        let agent = self.agent();
        let ctx = self.ctx.clone();
        let state = ChatState {
            agent,
            ctx,
            request: Some(pb::ChatRequest {
                instance: instance.to_string(),
                prompt: prompt.to_string(),
                ..Default::default()
            }),
            stream: None,
            done: false,
        };
        Box::pin(futures::stream::unfold(state, |mut state| async move {
            if state.done {
                return None;
            }
            if state.stream.is_none() {
                let request = state.request.take()?;
                match SendWrapper::new(
                    state
                        .agent
                        .chat_stream(RpcCtx::client(state.ctx.clone()), Request::new(request)),
                )
                .await
                {
                    Ok(stream) => state.stream = Some(SendWrapper::new(stream)),
                    Err(error) => {
                        state.done = true;
                        return Some((Err(map_error(&error)), state));
                    }
                }
            }
            let stream = state.stream.as_mut()?;
            match SendWrapper::new(stream.receive()).await {
                Ok(Some(event)) => Some((Ok(event), state)),
                Ok(None) => None,
                Err(error) => {
                    state.done = true;
                    Some((Err(map_error(&error)), state))
                }
            }
        }))
    }

    /// The number of characters of the bearer secret (a test hook).
    #[doc(hidden)]
    #[must_use]
    pub fn access_secret_len(&self) -> usize {
        self.access
            .lock()
            .map(|access| access.secret.len())
            .unwrap_or(0)
    }

    fn agent(&self) -> Arc<AgentServiceClient> {
        let secret = self
            .access
            .lock()
            .map(|access| access.secret.clone())
            .unwrap_or_else(|_| Zeroizing::new(String::new()));
        let options = ClientOptions::new().with_header(
            "Authorization".to_string(),
            format!("Bearer {}", secret.as_str()),
        );
        let transport: Arc<dyn foundation_connectrpc::Transport> = self.transport.clone();
        Arc::new(
            AgentServiceClient::new(transport, &self.url, options)
                .expect("the browser client URL is valid"),
        )
    }

    fn bootstrap_client(
        transport: &Arc<H1Transport>,
        url: &str,
    ) -> AgentRpcResult<BootstrapServiceClient> {
        let transport: Arc<dyn foundation_connectrpc::Transport> = transport.clone();
        BootstrapServiceClient::new(transport, url, ClientOptions::new())
            .map_err(|error| config("the enrollment client", error))
    }

    /// Refresh the access secret when its last fifth has begun.
    async fn ensure_access(&self) -> AgentRpcResult<()> {
        let (valid_from, valid_until) = self
            .access
            .lock()
            .map(|access| (access.valid_from, access.valid_until))
            .unwrap_or((0, 0));
        if valid_until == 0 {
            return Ok(());
        }
        let now = u64::try_from(foundation_compact::time::unix_seconds(
            self.ctx.clock().wall_now(),
        ))
        .unwrap_or(0);
        let window = valid_until.saturating_sub(valid_from);
        if now + window / 5 >= valid_until {
            self.refresh().await?;
        }
        Ok(())
    }

    /// Sign a refresh nonce with the stored key and take a new access secret.
    async fn refresh(&self) -> AgentRpcResult<()> {
        let Some(identity) = self.identity.lock().ok().and_then(|slot| slot.clone()) else {
            return Err(AgentRpcError::Auth(
                "this client has no enrolled identity to refresh".to_string(),
            )
            .into());
        };
        let nonce = random_nonce()?;
        let unix_ms = u64::try_from(foundation_compact::time::unix_millis(
            self.ctx.clock().wall_now(),
        ))
        .unwrap_or(0);
        let mut message =
            Vec::with_capacity(REFRESH_DOMAIN.len() + identity.client_id.len() + 8 + nonce.len());
        message.extend_from_slice(REFRESH_DOMAIN);
        message.extend_from_slice(identity.client_id.as_bytes());
        message.extend_from_slice(&unix_ms.to_be_bytes());
        message.extend_from_slice(&nonce);
        let signature = sign_with_identity(&identity, &message).await?;
        let enroll_url = self
            .enroll_url
            .clone()
            .unwrap_or_else(|| identity.enroll_url.clone());
        let client = {
            let mut slot = self
                .bootstrap
                .lock()
                .map_err(|_| config("the enrollment client", "the lock is poisoned"))?;
            if slot.is_none() {
                *slot = Some(Arc::new(Self::bootstrap_client(
                    &self.transport,
                    &enroll_url,
                )?));
            }
            Arc::clone(slot.as_ref().expect("the client was just set"))
        };
        let request = pb::RefreshAccessRequest {
            client_id: identity.client_id,
            unix_ms,
            nonce: nonce.to_vec(),
            signature,
            ..Default::default()
        };
        let response = SendWrapper::new(
            client.refresh_access(RpcCtx::client(self.ctx.clone()), Request::new(request)),
        )
        .await
        .map_err(|error| map_error(&error))?
        .msg;
        let access = SignedSecret::decode(&response.access_secret, &DecodeLimits::default())
            .map_err(|error| {
                ErrorTrace::new(AgentRpcError::Auth(format!("the access secret: {error}")))
            })?;
        let mut slot = self
            .access
            .lock()
            .map_err(|_| config("the access secret", "the lock is poisoned"))?;
        *slot = Access {
            secret: access.encode(),
            valid_from: access.valid_from(),
            valid_until: access.valid_until(),
        };
        Ok(())
    }
}

/// Sign `message` with the identity key: Ed25519, or ECDSA P-256.
async fn sign_with_identity(identity: &WebIdentity, message: &[u8]) -> AgentRpcResult<Vec<u8>> {
    let subtle = subtle()?;
    let algorithm = if identity.alg == "ed25519" {
        algorithm_object("Ed25519", None)
    } else {
        ecdsa_algorithm()
    };
    let promise = subtle
        .sign_with_object_and_u8_array(&algorithm, &identity.private_key, message)
        .map_err(|error| config("the refresh signature", error))?;
    let signature = SendWrapper::new(JsFuture::from(promise))
        .await
        .map_err(|error| config("the refresh signature", error))?;
    Ok(Uint8Array::new(&signature).to_vec())
}

struct ChatState {
    agent: Arc<AgentServiceClient>,
    ctx: Ctx,
    request: Option<pb::ChatRequest>,
    stream: Option<SendWrapper<ServerStream<pb::TurnEvent>>>,
    done: bool,
}

fn create_request(spec: &InstanceSpec) -> pb::CreateInstanceRequest {
    let device: buffa::MessageField<pb::Device, buffa::Inline<pb::Device>> = spec
        .model
        .device
        .map_or_else(buffa::MessageField::default, |device| {
            Some(device.to_wire()).into()
        });
    pb::CreateInstanceRequest {
        name: spec.name.clone(),
        model: spec.model.name.clone().unwrap_or_default(),
        system_prompt: spec.system_prompt.clone(),
        max_tokens: spec
            .max_tokens
            .and_then(|tokens| u32::try_from(tokens).ok()),
        temperature: spec.temperature,
        kind: buffa::EnumValue::Known(spec.model.kind.map_or_default(ModelKind::to_wire)),
        device,
        ..Default::default()
    }
}

fn instance_info(info: pb::InstanceInfo) -> InstanceInfo {
    InstanceInfo {
        name: info.name,
        model: info.model,
        kind: ModelKind::Api,
        device: Device::Remote,
        created_unix_ms: info.created_unix_ms,
        turns: info.turns,
    }
}
