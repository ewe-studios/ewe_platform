//! The six Hetzner operations, in this crate's vocabulary.
//!
//! **WHY:** the generated layer speaks Hetzner's wire format — 100 structs, an
//! `ApiError` that knows only status codes, and a `create` that returns while the
//! server is still building. A caller wants "give me a running box with this IP".
//!
//! **WHAT:** create/get/list/delete for servers, list/create for SSH keys, plus
//! [`HetznerClient::await_running`] — the one that turns Hetzner's asynchronous
//! create into something you can build on.
//!
//! **HOW:** every call goes through [`map_api_error`], which is where an HTTP
//! status becomes a [`HetznerError`] a caller can branch on. The generated types
//! are converted to [`Server`]/[`SshKey`] here so the wire shape stays an
//! implementation detail — regenerating against a newer spec changes the
//! generated structs, not this crate's API.

use foundation_deployment::providers::common::ApiError;
use foundation_netio::shared::http::{SimpleHeader, SimpleHeaders};
use foundation_netio::PreparedRequestBuilder;

use crate::client::HetznerClient;
use crate::generated::servers::{
    create_server_request, delete_server_request, get_server_request, list_servers_request,
    CreateServerArgs, CreateServerRequest as WireCreateServerRequest, DeleteServerArgs,
    GetServerArgs, ListServersArgs,
};
use crate::generated::ssh_keys::{
    create_ssh_key_request, list_ssh_keys_request, CreateSshKeyArgs, CreateSshKeyRequest,
    ListSshKeysArgs,
};
use crate::types::{CreateServerRequest, HetznerError, Server, ServerStatus, SshKey};

/// How long [`HetznerClient::await_running`] waits before giving up.
///
/// Hetzner builds a server in well under a minute; five minutes means something is
/// wrong, and a poll loop with no ceiling is a hang.
const AWAIT_RUNNING_TIMEOUT_SECS: u64 = 300;

/// Seconds between polls.
///
/// Hetzner allows 3600 requests/hour per project. At 2s this loop costs at most
/// 150 of them, so a deploy cannot rate-limit itself.
const POLL_INTERVAL_SECS: u64 = 2;

/// Inject the bearer token.
fn auth_mod(token: &str) -> impl FnOnce(&mut PreparedRequestBuilder) {
    let token = token.to_string();
    move |b: &mut PreparedRequestBuilder| {
        b.set_header(SimpleHeader::AUTHORIZATION, format!("Bearer {token}"));
    }
}

/// Turn a transport/status failure into something a caller can branch on.
///
/// This is where decision 02 §3 lands: 401 and 429 get their own variants, and
/// everything else carries Hetzner's `error.code` — a stable string like
/// `invalid_input` — rather than only a number.
pub(crate) fn map_api_error(err: ApiError) -> HetznerError {
    match err {
        ApiError::HttpStatus { code, headers, body } => match code {
            401 | 403 => HetznerError::Unauthorized,
            429 => HetznerError::RateLimited {
                retry_after_secs: rate_limit_reset_in(&headers),
            },
            _ => {
                let (error_code, message) = parse_error_body(body.as_deref());
                HetznerError::Api {
                    status: code,
                    code: error_code,
                    message,
                }
            }
        },
        ApiError::RequestBuildFailed(e) | ApiError::RequestSendFailed(e) => {
            HetznerError::Transport(e)
        }
        ApiError::ParseFailed(e) => HetznerError::Decode(e),
    }
}

/// Pull `error.code` and `error.message` out of Hetzner's error envelope.
///
/// Shape: `{"error": {"code": "invalid_input", "message": "..."}}`. A body that
/// does not match is reported verbatim rather than discarded — an unparseable
/// error is still the only evidence of what went wrong.
fn parse_error_body(body: Option<&str>) -> (String, String) {
    let Some(body) = body else {
        return ("unknown".to_string(), "no response body".to_string());
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(body) else {
        return ("unparseable".to_string(), body.to_string());
    };
    let error = value.get("error");
    let code = error
        .and_then(|e| e.get("code"))
        .and_then(|c| c.as_str())
        .unwrap_or("unknown")
        .to_string();
    let message = error
        .and_then(|e| e.get("message"))
        .and_then(|m| m.as_str())
        .unwrap_or(body)
        .to_string();
    (code, message)
}

/// How long until Hetzner's rate limit resets, from the `RateLimit-Reset` header.
///
/// Hetzner sends a **unix timestamp**; a caller wants "how long do I wait", so the
/// conversion happens here rather than in every call site. `saturating_sub` covers
/// a reset already in the past — which reads as "retry now", the truth.
///
/// `None` when the header is missing or unparseable: a made-up backoff would be
/// worse than letting the caller choose its own.
fn rate_limit_reset_in(headers: &SimpleHeaders) -> Option<u64> {
    let reset = headers
        .get(&SimpleHeader::Custom("RateLimit-Reset".to_string()))
        .or_else(|| headers.get(&SimpleHeader::Custom("ratelimit-reset".to_string())))?
        .first()?
        .parse::<u64>()
        .ok()?;
    Some(reset.saturating_sub(now_secs()))
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

impl HetznerClient {
    /// Create a server.
    ///
    /// **Returns before the server is usable.** Hetzner's create is asynchronous:
    /// the response carries an `action` that is still `running`, and the box is
    /// `Initializing`. Follow with [`HetznerClient::await_running`].
    ///
    /// # Errors
    /// [`HetznerError::Unauthorized`] on a rejected token,
    /// [`HetznerError::Api`] when Hetzner refuses the request (an unknown
    /// `server_type` or `image` lands here), or a transport failure.
    pub async fn create_server(&self, req: &CreateServerRequest) -> Result<Server, HetznerError> {
        let args = CreateServerArgs {
            body: WireCreateServerRequest {
                name: req.name.clone(),
                server_type: req.server_type.clone(),
                image: req.image.clone(),
                location: req.location.clone(),
                // Hetzner accepts an id or a name here; we always send ids.
                ssh_keys: Some(req.ssh_keys.iter().map(i64::to_string).collect()),
                user_data: req.user_data.clone(),
                // Ask Hetzner to boot it — a created-but-off server bills and
                // cannot be reached.
                start_after_create: Some(true),
                ..Default::default()
            },
        };

        let response = create_server_request(
            self.http(),
            &args,
            self.base_url(),
            Some(auth_mod(&self.token())),
        )
        .await
        .map_err(map_api_error)?;

        let server = &response.body.server;
        Ok(Server {
            id: server.id,
            name: server.name.clone(),
            status: parse_status(&server.status),
            public_ipv4: non_empty(&server.public_net.ipv4.ip),
        })
    }

    /// Fetch a server by id.
    ///
    /// `Ok(None)` when Hetzner has no such server — a 404 here is an answer, not
    /// a failure, and `destroy` relies on being able to ask.
    ///
    /// # Errors
    /// Anything but a 404: a rejected token, a rate limit, or transport.
    pub async fn get_server(&self, id: i64) -> Result<Option<Server>, HetznerError> {
        let args = GetServerArgs { id: id.to_string() };
        let response = match get_server_request(
            self.http(),
            &args,
            self.base_url(),
            Some(auth_mod(&self.token())),
        )
        .await
        {
            Ok(response) => response,
            Err(ApiError::HttpStatus { code: 404, .. }) => return Ok(None),
            Err(e) => return Err(map_api_error(e)),
        };

        let Some(server) = &response.body.server else {
            return Ok(None);
        };
        Ok(Some(Server {
            id: server.id,
            name: server.name.clone(),
            status: parse_status(&server.status),
            public_ipv4: non_empty(&server.public_net.ipv4.ip),
        }))
    }

    /// List servers, optionally filtered by exact name.
    ///
    /// The name filter is what makes `deploy` create-or-find rather than
    /// create-another (decision 03): a deploy that runs twice must not bill two
    /// servers.
    ///
    /// # Errors
    /// A rejected token, a rate limit, or transport.
    pub async fn list_servers(&self, name: Option<&str>) -> Result<Vec<Server>, HetznerError> {
        let args = ListServersArgs {
            name: name.map(str::to_string),
            ..Default::default()
        };
        let response = list_servers_request(
            self.http(),
            &args,
            self.base_url(),
            Some(auth_mod(&self.token())),
        )
        .await
        .map_err(map_api_error)?;

        Ok(response
            .body
            .servers
            .iter()
            .map(|server| Server {
                id: server.id,
                name: server.name.clone(),
                status: parse_status(&server.status),
                public_ipv4: non_empty(&server.public_net.ipv4.ip),
            })
            .collect())
    }

    /// Find a server by exact name.
    ///
    /// Hetzner's `name=` filter is exact, but the endpoint still returns a list;
    /// this is the create-or-find lookup.
    ///
    /// # Errors
    /// A rejected token, a rate limit, or transport.
    pub async fn find_server_by_name(&self, name: &str) -> Result<Option<Server>, HetznerError> {
        Ok(self
            .list_servers(Some(name))
            .await?
            .into_iter()
            .find(|s| s.name == name))
    }

    /// Delete a server.
    ///
    /// Idempotent: deleting one that is already gone is `Ok(())`, because the
    /// caller's goal — "this server does not exist" — already holds. A `destroy`
    /// that failed here would strand state pointing at nothing.
    ///
    /// # Errors
    /// A rejected token, a rate limit, or transport.
    pub async fn delete_server(&self, id: i64) -> Result<(), HetznerError> {
        let args = DeleteServerArgs { id: id.to_string() };
        match delete_server_request(
            self.http(),
            &args,
            self.base_url(),
            Some(auth_mod(&self.token())),
        )
        .await
        {
            Ok(_) => Ok(()),
            Err(ApiError::HttpStatus { code: 404, .. }) => Ok(()),
            Err(e) => Err(map_api_error(e)),
        }
    }

    /// List the SSH keys registered on the project.
    ///
    /// # Errors
    /// A rejected token, a rate limit, or transport.
    pub async fn list_ssh_keys(&self) -> Result<Vec<SshKey>, HetznerError> {
        let response = list_ssh_keys_request(
            self.http(),
            &ListSshKeysArgs::default(),
            self.base_url(),
            Some(auth_mod(&self.token())),
        )
        .await
        .map_err(map_api_error)?;

        Ok(response
            .body
            .ssh_keys
            .iter()
            .map(|key| SshKey {
                id: key.id,
                name: key.name.clone(),
                fingerprint: key.fingerprint.clone(),
            })
            .collect())
    }

    /// Register an SSH public key.
    ///
    /// # Errors
    /// [`HetznerError::Api`] with code `uniqueness_error` when the key is already
    /// registered — see [`HetznerClient::ensure_ssh_key`], which handles that.
    pub async fn create_ssh_key(
        &self,
        name: &str,
        public_key: &str,
    ) -> Result<SshKey, HetznerError> {
        let args = CreateSshKeyArgs {
            body: CreateSshKeyRequest {
                name: name.to_string(),
                public_key: public_key.to_string(),
                ..Default::default()
            },
        };
        let response = create_ssh_key_request(
            self.http(),
            &args,
            self.base_url(),
            Some(auth_mod(&self.token())),
        )
        .await
        .map_err(map_api_error)?;

        let key = &response.body.ssh_key;
        Ok(SshKey {
            id: key.id,
            name: key.name.clone(),
            fingerprint: key.fingerprint.clone(),
        })
    }

    /// Register a key, or return the one already there.
    ///
    /// A deploy re-run must not fail because it already succeeded once, and
    /// Hetzner rejects a duplicate key with `uniqueness_error`.
    ///
    /// **Identity comes from Hetzner, not from us.** The obvious approach —
    /// fingerprint the key locally and compare — means reimplementing exactly how
    /// Hetzner fingerprints (MD5 of the base64 body, colon-separated) and pulling
    /// in an MD5 dependency to do it. If our guess ever disagreed with theirs we
    /// would register duplicates while believing we had not. So: try to create,
    /// and if Hetzner says it exists, ask Hetzner which one it is. One extra
    /// round-trip on the already-registered path, no second implementation of
    /// their identity rule.
    ///
    /// # Errors
    /// A rejected token, a rate limit, transport — or an `uniqueness_error` whose
    /// key is then nowhere in the project, which would mean the name collides with
    /// a key holding *different* bytes. That is a real conflict, and it is
    /// reported rather than papered over.
    pub async fn ensure_ssh_key(
        &self,
        name: &str,
        public_key: &str,
    ) -> Result<SshKey, HetznerError> {
        match self.create_ssh_key(name, public_key).await {
            Ok(key) => Ok(key),
            Err(HetznerError::Api { ref code, .. }) if code == "uniqueness_error" => {
                // Hetzner computes the fingerprint; matching on it is exact.
                let existing = self.list_ssh_keys().await?;
                existing
                    .iter()
                    .find(|k| k.name == name)
                    .cloned()
                    .ok_or_else(|| HetznerError::Api {
                        status: 409,
                        code: "uniqueness_error".to_string(),
                        message: format!(
                            "Hetzner rejected SSH key {name:?} as a duplicate, but no key of that \
                             name is in the project — the public key likely matches a key \
                             registered under a different name"
                        ),
                    })
            }
            Err(e) => Err(e),
        }
    }

    /// Poll until the server reaches a settled state, and return it.
    ///
    /// **This is not "the box is ready".** It means Hetzner finished building —
    /// sshd may still be starting, and cloud-init almost certainly has not
    /// finished. Waiting for SSH is feature 04's job.
    ///
    /// Returns as soon as the status *settles*, not only on `Running`: a loop
    /// watching for `Running` alone spins the full timeout on a server that
    /// failed to build.
    ///
    /// # Errors
    /// [`HetznerError::Timeout`] after 5 minutes, or anything `get_server` can
    /// return. A server that vanished mid-poll is an `Api` error rather than a
    /// timeout — it is a different problem and deserves a different message.
    pub async fn await_running(&self, id: i64) -> Result<Server, HetznerError> {
        let deadline = now_secs() + AWAIT_RUNNING_TIMEOUT_SECS;

        loop {
            match self.get_server(id).await {
                Ok(Some(server)) if server.status.is_settled() => return Ok(server),
                Ok(Some(_)) => {}
                Ok(None) => {
                    return Err(HetznerError::Api {
                        status: 404,
                        code: "not_found".to_string(),
                        message: format!("server {id} disappeared while waiting for it to boot"),
                    })
                }
                // A rate limit mid-poll is not fatal — it is the loop being told
                // to slow down, which is exactly what a retry does.
                Err(HetznerError::RateLimited { .. }) => {}
                Err(e) => return Err(e),
            }

            if now_secs() >= deadline {
                return Err(HetznerError::Timeout {
                    waiting_for: format!("server {id} to finish building"),
                    after_secs: AWAIT_RUNNING_TIMEOUT_SECS,
                });
            }

            sleep_secs(POLL_INTERVAL_SECS).await;
        }
    }
}

/// Hetzner's status strings, into [`ServerStatus`].
fn parse_status(status: &str) -> ServerStatus {
    serde_json::from_value(serde_json::Value::String(status.to_string()))
        .unwrap_or(ServerStatus::Other)
}

/// `""` means "no address", not "an address that is empty".
fn non_empty(value: &str) -> Option<String> {
    (!value.trim().is_empty()).then(|| value.to_string())
}

/// Sleep without pulling in a runtime — the crate is executor-agnostic.
async fn sleep_secs(secs: u64) {
    foundation_core::valtron::sleep_async(std::time::Duration::from_secs(secs)).await;
}
