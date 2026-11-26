//! The Worker's instance registry: the D1 index that forwards to the instance
//! Durable Object (spec 83 feature 05, sections 5.5 and 5.9).
//!
//! WHY: a Worker holds no instance state (invariant I1). The index lives in D1,
//! and the session lives in its Durable Object (DO). The Worker reserves a
//! name in the index, then sends the call to the DO with the checked principal.
//!
//! WHAT: [`DurableObjectRegistry`], which implements [`InstanceRegistry`] over
//! the D1 table `agent_rpc_instances` and [`InstanceStub`].
//!
//! HOW: `create` inserts the row before it calls the DO, so two callers cannot
//! reserve one name; a failed create removes the row. `list` reads one ordered
//! page. `destroy` calls the DO (which archives, then deletes) and only then
//! removes the row, so a failed archive keeps the index entry too.

use std::sync::Arc;

use crate::core::config::RegistryConfig;
use crate::core::model::{AgentRpcFuture, Device, ModelKind};
use crate::core::registry::{
    Caller, InstanceInfo, InstanceRegistry, InstanceSpec, Page, TurnEventStream,
};
use crate::error::{AgentRpcError, AgentRpcResult};
use crate::proto::v1 as pb;
use base64::Engine;
use buffa::Message;
use foundation_context::Ctx;
use foundation_db::wasm::bindgen::cf::D1Database;
use foundation_db::{D1WasmStorage, DataValue, QueryStore, SqlRow};
use foundation_errstacks::ErrorTrace;

use crate::host::worker::config::ServeLimits;
use crate::host::worker::do_client::InstanceStub;

/// The D1 table of the instance index.
pub const INDEX_TABLE: &str = "agent_rpc_instances";

/// The Worker's registry over the D1 index and the instance Durable Objects.
pub struct DurableObjectRegistry {
    env: worker::Env,
    index: Arc<D1WasmStorage>,
    config: RegistryConfig,
    limits: ServeLimits,
    ctx: Ctx,
}

impl std::fmt::Debug for DurableObjectRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DurableObjectRegistry")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl DurableObjectRegistry {
    /// Build the registry over the D1 `AGENT_RPC_DB` binding and create the
    /// index table.
    ///
    /// # Errors
    /// [`AgentRpcError::Storage`] when the table cannot be created.
    pub async fn new(
        env: worker::Env,
        database: D1Database,
        config: RegistryConfig,
        limits: ServeLimits,
        ctx: Ctx,
    ) -> AgentRpcResult<Self> {
        let index = Arc::new(D1WasmStorage::new(Arc::new(database), "agent_rpc"));
        index
            .execute_batch(&format!(
                "CREATE TABLE IF NOT EXISTS {INDEX_TABLE} (\
                 scope TEXT NOT NULL, \
                 name TEXT NOT NULL, \
                 model TEXT NOT NULL, \
                 created_ms INTEGER NOT NULL, \
                 turns INTEGER NOT NULL DEFAULT 0, \
                 PRIMARY KEY (scope, name))"
            ))
            .await
            .map_err(|error| AgentRpcError::Storage(error.to_string()))?;
        Ok(Self {
            env,
            index,
            config,
            limits,
            ctx,
        })
    }

    /// The generated client for `name`, with the principal forwarded.
    fn stub(&self, caller: &Caller, name: &str) -> AgentRpcResult<InstanceStub> {
        let _ = &self.limits;
        InstanceStub::new(&self.env, name, caller.principal.subject.as_str())
    }

    /// Validate an instance name with the registry rules.
    fn validate_name(&self, name: &str) -> AgentRpcResult<()> {
        let invalid = |why: &str| -> AgentRpcResult<()> {
            Err(AgentRpcError::InvalidName(format!("{name:?}: {why}")).into())
        };
        if name.is_empty() {
            return invalid("empty");
        }
        if name.len() > self.config.max_name_len {
            return invalid("too long");
        }
        if !name.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"._-".contains(&byte)
        }) {
            return invalid("allowed characters are a-z 0-9 . _ -");
        }
        if !name
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_alphanumeric())
        {
            return invalid("must start with a letter or digit");
        }
        Ok(())
    }

    async fn rows(&self, sql: &str, params: &[DataValue]) -> AgentRpcResult<Vec<SqlRow>> {
        self.index
            .query(sql, params)
            .await
            .map_err(|error| ErrorTrace::new(AgentRpcError::Storage(error.to_string())))?
            .collect_all()
            .await
            .map_err(|error| ErrorTrace::new(AgentRpcError::Storage(error.to_string())))
    }

    async fn execute(&self, sql: &str, params: &[DataValue]) -> AgentRpcResult<u64> {
        self.index
            .execute(sql, params)
            .await
            .map_err(|error| ErrorTrace::new(AgentRpcError::Storage(error.to_string())))
    }

    fn page_size(&self, page_size: u32) -> usize {
        let size = if page_size == 0 {
            self.config.default_page_size
        } else {
            page_size.min(self.config.max_page_size)
        }
        .max(1);
        usize::try_from(size).unwrap_or(usize::MAX)
    }

    fn info_of(row: &SqlRow) -> AgentRpcResult<InstanceInfo> {
        let convert = |what: &str| AgentRpcError::Storage(format!("the index row has no {what}"));
        let name = row
            .get_by_name::<String>("name")
            .map_err(|_| convert("name"))?;
        let model = row
            .get_by_name::<String>("model")
            .map_err(|_| convert("model"))?;
        let created_unix_ms = u64::try_from(
            row.get_by_name::<i64>("created_ms")
                .map_err(|_| convert("created_ms"))?,
        )
        .unwrap_or(0);
        let turns = u64::try_from(
            row.get_by_name::<i64>("turns")
                .map_err(|_| convert("turns"))?,
        )
        .unwrap_or(0);
        Ok(InstanceInfo {
            name,
            model,
            kind: ModelKind::Api,
            device: Device::Remote,
            created_unix_ms,
            turns,
        })
    }
}

impl InstanceRegistry for DurableObjectRegistry {
    fn create<'a>(
        &'a self,
        caller: &'a Caller,
        spec: InstanceSpec,
    ) -> AgentRpcFuture<'a, InstanceInfo> {
        Box::pin(async move {
            self.validate_name(&spec.name)?;
            let scope = caller.scope.as_key().to_string();
            let count = self
                .rows(
                    &format!("SELECT COUNT(*) AS n FROM {INDEX_TABLE} WHERE scope = ?"),
                    &[DataValue::Text(scope.clone())],
                )
                .await?
                .first()
                .and_then(|row| row.get_by_name::<i64>("n").ok())
                .unwrap_or(0);
            if u64::try_from(count).unwrap_or(u64::MAX)
                >= u64::try_from(self.config.max_instances).unwrap_or(u64::MAX)
            {
                return Err(AgentRpcError::InstanceLimit(self.config.max_instances).into());
            }

            let now = i64::try_from(foundation_compact::time::unix_millis(
                self.ctx.clock().wall_now(),
            ))
            .unwrap_or(0);
            let model = spec.model.name.clone().unwrap_or_default();
            let reserved = self
                .execute(
                    &format!(
                        "INSERT INTO {INDEX_TABLE} (scope, name, model, created_ms, turns) \
                         VALUES (?, ?, ?, ?, 0)"
                    ),
                    &[
                        DataValue::Text(scope.clone()),
                        DataValue::Text(spec.name.clone()),
                        DataValue::Text(model),
                        DataValue::Integer(now),
                    ],
                )
                .await;
            if reserved.is_err() {
                return Err(AgentRpcError::InstanceExists(spec.name).into());
            }

            let stub = self.stub(caller, &spec.name)?;
            let body = match stub
                .unary(
                    crate::proto::v1::agent_service::procedure::CREATE_INSTANCE,
                    create_request(&spec).encode_to_vec(),
                )
                .await
            {
                Ok(body) => body,
                Err(error) => {
                    // The DO did not create the instance: release the name.
                    let _ = self
                        .execute(
                            &format!("DELETE FROM {INDEX_TABLE} WHERE scope = ? AND name = ?"),
                            &[DataValue::Text(scope), DataValue::Text(spec.name.clone())],
                        )
                        .await;
                    return Err(error);
                }
            };
            let info = pb::InstanceInfo::decode_from_slice(&body).map_err(|error| {
                ErrorTrace::new(AgentRpcError::Transport(format!(
                    "the instance info did not decode: {error}"
                )))
            })?;
            let _ = self
                .execute(
                    &format!(
                        "UPDATE {INDEX_TABLE} SET model = ?, created_ms = ?, turns = ? \
                         WHERE scope = ? AND name = ?"
                    ),
                    &[
                        DataValue::Text(info.model.clone()),
                        DataValue::Integer(i64::try_from(info.created_unix_ms).unwrap_or(now)),
                        DataValue::Integer(i64::try_from(info.turns).unwrap_or(0)),
                        DataValue::Text(scope),
                        DataValue::Text(spec.name.clone()),
                    ],
                )
                .await;
            Ok(InstanceInfo {
                name: info.name,
                model: info.model,
                kind: ModelKind::Api,
                device: Device::Remote,
                created_unix_ms: info.created_unix_ms,
                turns: info.turns,
            })
        })
    }

    fn get<'a>(&'a self, caller: &'a Caller, name: &'a str) -> AgentRpcFuture<'a, InstanceInfo> {
        Box::pin(async move {
            let rows = self
                .rows(
                    &format!(
                        "SELECT name, model, created_ms, turns FROM {INDEX_TABLE} \
                         WHERE scope = ? AND name = ?"
                    ),
                    &[
                        DataValue::Text(caller.scope.as_key().to_string()),
                        DataValue::Text(name.to_string()),
                    ],
                )
                .await?;
            rows.first()
                .map(Self::info_of)
                .transpose()?
                .ok_or_else(|| AgentRpcError::InstanceNotFound(name.to_string()).into())
        })
    }

    fn list<'a>(
        &'a self,
        caller: &'a Caller,
        page_size: u32,
        page_token: Option<&'a str>,
    ) -> AgentRpcFuture<'a, Page<InstanceInfo>> {
        Box::pin(async move {
            let size = self.page_size(page_size);
            let after = page_token
                .filter(|token| !token.is_empty())
                .map(decode_token);
            let after = after.transpose()?.unwrap_or_default();
            let rows = self
                .rows(
                    &format!(
                        "SELECT name, model, created_ms, turns FROM {INDEX_TABLE} \
                         WHERE scope = ? AND name > ? ORDER BY name LIMIT ?"
                    ),
                    &[
                        DataValue::Text(caller.scope.as_key().to_string()),
                        DataValue::Text(after),
                        DataValue::Integer(i64::try_from(size + 1).unwrap_or(i64::MAX)),
                    ],
                )
                .await?;
            let has_more = rows.len() > size;
            let items: Vec<InstanceInfo> = rows
                .iter()
                .take(size)
                .map(Self::info_of)
                .collect::<AgentRpcResult<_>>()?;
            let next_token = has_more
                .then(|| items.last().map(|info| encode_token(&info.name)))
                .flatten();
            Ok(Page { items, next_token })
        })
    }

    fn destroy<'a>(&'a self, caller: &'a Caller, name: &'a str) -> AgentRpcFuture<'a, ()> {
        Box::pin(async move {
            let stub = self.stub(caller, name)?;
            stub.unary(
                crate::proto::v1::agent_service::procedure::DESTROY_INSTANCE,
                pb::DestroyInstanceRequest {
                    name: name.to_string(),
                    ..Default::default()
                }
                .encode_to_vec(),
            )
            .await?;
            self.execute(
                &format!("DELETE FROM {INDEX_TABLE} WHERE scope = ? AND name = ?"),
                &[
                    DataValue::Text(caller.scope.as_key().to_string()),
                    DataValue::Text(name.to_string()),
                ],
            )
            .await?;
            Ok(())
        })
    }

    fn start_turn<'a>(
        &'a self,
        caller: &'a Caller,
        name: &'a str,
        prompt: String,
    ) -> AgentRpcFuture<'a, TurnEventStream> {
        Box::pin(async move {
            // A turn on a name that the index does not hold is a not-found.
            let _ = self.get(caller, name).await?;
            let stub = self.stub(caller, name)?;
            let chat = pb::ChatRequest {
                instance: name.to_string(),
                prompt,
                ..Default::default()
            };
            let body = connect_envelope(&chat.encode_to_vec());
            let stream = stub
                .stream(
                    crate::proto::v1::agent_service::procedure::CHAT_STREAM,
                    body,
                )
                .await?;
            Ok(stream)
        })
    }

    fn count<'a>(&'a self, caller: &'a Caller) -> AgentRpcFuture<'a, u64> {
        Box::pin(async move {
            let count = self
                .rows(
                    &format!("SELECT COUNT(*) AS n FROM {INDEX_TABLE} WHERE scope = ?"),
                    &[DataValue::Text(caller.scope.as_key().to_string())],
                )
                .await?
                .first()
                .and_then(|row| row.get_by_name::<i64>("n").ok())
                .unwrap_or(0);
            Ok(u64::try_from(count).unwrap_or(0))
        })
    }
}

/// The wire request of one `CreateInstance`.
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

/// One Connect envelope: flag zero, a big-endian length and the message.
fn connect_envelope(message: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(5 + message.len());
    out.push(0);
    out.extend_from_slice(&u32::try_from(message.len()).unwrap_or(0).to_be_bytes());
    out.extend_from_slice(message);
    out
}

fn encode_token(name: &str) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(name.as_bytes())
}

fn decode_token(token: &str) -> AgentRpcResult<String> {
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(token)
        .map_err(|_| {
            ErrorTrace::new(AgentRpcError::InvalidArgument(
                "the page token is not base64url".to_string(),
            ))
        })?;
    String::from_utf8(bytes).map_err(|_| {
        AgentRpcError::InvalidArgument("the page token is not UTF-8".to_string()).into()
    })
}
