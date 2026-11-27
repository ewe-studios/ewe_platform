//! Issue one pause per session and confirm release at its deadline.
use super::{wall_ms, Block, Current, Request, TodoId, Update};
use crate::shared::{error, TodoDelivery};
use foundation_context::{env::Env, Instant};
use foundation_errstacks::ErrorTrace;
use foundation_ui_runtime::{
    action,
    action::{ActionError, ActionResult, Context, Interaction, ViewData},
};
use foundation_ui_wire::ErrorCode;
use std::{collections::BTreeMap, sync::Mutex, time::Duration};

struct Receipt {
    block: Block,
    deadline: Instant,
    keep_until_ms: u64,
}

#[derive(Default)]
struct Records {
    requests: BTreeMap<(TodoId, TodoId), Receipt>,
    current: BTreeMap<TodoId, (Block, Instant)>,
}

/// Limits for one Todo countdown service.
#[derive(Clone, Copy, Debug)]
pub struct CountdownConfig {
    duration: Duration,
    receipt_lifetime: Duration,
    max_receipts: usize,
    release_wait: Duration,
}

impl Default for CountdownConfig {
    fn default() -> Self {
        Self {
            duration: Duration::from_secs(60),
            receipt_lifetime: Duration::from_mins(15),
            max_receipts: 1024,
            release_wait: Duration::from_secs(2),
        }
    }
}

#[cfg(feature = "server")]
impl CountdownConfig {
    /// Set how long the service keeps a request receipt. The default is 15 minutes.
    #[must_use]
    pub fn with_receipt_lifetime(mut self, duration: Duration) -> Self {
        self.receipt_lifetime = duration.max(self.duration);
        self
    }

    /// Set the maximum number of recent requests. The default is 1024.
    #[must_use]
    pub fn with_max_receipts(mut self, count: usize) -> Self {
        self.max_receipts = count;
        self
    }

    /// Set how long the server holds a private release request. The default is two seconds.
    #[must_use]
    pub fn with_release_wait(mut self, duration: Duration) -> Self {
        self.release_wait = duration.min(self.duration);
        self
    }
}

struct CountdownService {
    records: Mutex<Records>,
    config: CountdownConfig,
}

#[cfg(any(feature = "server", feature = "mobile"))]
pub(crate) fn install(ctx: &foundation_context::Ctx) {
    ctx.store(CountdownService {
        records: Mutex::new(Records::default()),
        config: ctx
            .get::<CountdownConfig>()
            .map_or_else(CountdownConfig::default, |config| *config),
    });
}

fn millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn request_update(ctx: &Context, input: Request) -> Result<Update, ErrorTrace<ActionError>> {
    let Request {
        scope,
        operation_id: request,
        issued_at_ms,
    } = input;
    let service = ctx
        .get::<CountdownService>()
        .ok_or_else(|| error("Missing countdown service"))?;
    let env = ctx
        .get::<Env>()
        .ok_or_else(|| error("Missing countdown identifier source"))?;
    let wall = wall_ms(ctx);
    let now = ctx.clock().now();
    let mut records = service
        .records
        .lock()
        .map_err(|_| error("Countdown state is unavailable"))?;
    records
        .requests
        .retain(|_, receipt| receipt.keep_until_ms >= wall);
    let lifetime = millis(service.config.receipt_lifetime);
    let key = (scope.session, request);
    if let Some(receipt) = records.requests.get(&key) {
        let block = receipt.block.clone();
        let remaining_ms = millis(receipt.deadline.saturating_duration_since(now));
        return Ok(if remaining_ms == 0 {
            Update::Receipt {
                scope,
                block,
                request,
            }
        } else {
            Update::Active {
                scope,
                block,
                remaining_ms,
            }
        });
    }

    if issued_at_ms.abs_diff(wall) > lifetime {
        return Err(ErrorTrace::new(ActionError::new(
            ErrorCode::InvalidInput,
            "The pause request has expired",
        )));
    }
    if records.requests.len() >= service.config.max_receipts {
        return Err(error("Too many recent pause requests"));
    }

    records.current.retain(|_, (_, deadline)| *deadline > now);
    let (block, deadline) = if let Some(active) = records.current.get(&scope.session) {
        active.clone()
    } else {
        let id = env
            .entropy()
            .uuid_v4()
            .map_err(|cause| error(&cause.to_string()))?;
        let expires_at_ms = wall
            .checked_add(millis(service.config.duration))
            .ok_or_else(|| error("Invalid countdown expiry"))?;
        let block = Block { id, expires_at_ms };
        let deadline = now + service.config.duration;
        records
            .current
            .insert(scope.session, (block.clone(), deadline));
        (block, deadline)
    };
    records.requests.insert(
        key,
        Receipt {
            block: block.clone(),
            deadline,
            keep_until_ms: issued_at_ms.max(wall).saturating_add(lifetime),
        },
    );
    Ok(Update::Active {
        scope,
        block,
        remaining_ms: millis(deadline.saturating_duration_since(now)),
    })
}

#[action(server)]
pub(in crate::shared) async fn request_countdown(
    ctx: &Context,
    event: Interaction<(), Request>,
) -> ActionResult {
    TodoDelivery::Countdown(request_update(ctx, event.input_value)?).send(ctx)
}

#[action(server)]
pub(in crate::shared) async fn current_countdown(
    ctx: &Context,
    event: Interaction<(), Current>,
) -> ActionResult {
    let Current {
        scope,
        observed,
        await_release,
    } = event.input_value;
    let service = ctx
        .get::<CountdownService>()
        .ok_or_else(|| error("Missing countdown service"))?;
    let now = ctx.clock().now();
    let current = {
        let mut records = service
            .records
            .lock()
            .map_err(|_| error("Countdown state is unavailable"))?;
        let current = records.current.get(&scope.session).cloned();
        records.current.retain(|_, (_, deadline)| *deadline > now);
        current
    };
    let update = match current {
        Some((block, deadline)) if deadline > now => {
            let remaining = deadline.saturating_duration_since(now);
            if await_release
                && observed == Some(block.id)
                && remaining <= service.config.release_wait
            {
                let env = ctx
                    .get::<Env>()
                    .ok_or_else(|| error("Missing countdown timer"))?;
                env.timer().sleep_until(ctx.inner().clone(), deadline).await;
                Update::Released {
                    scope,
                    id: block.id,
                }
            } else {
                Update::Active {
                    scope,
                    block,
                    remaining_ms: millis(remaining),
                }
            }
        }
        Some((block, _)) => Update::Released {
            scope,
            id: block.id,
        },
        None => Update::Clear { scope, observed },
    };
    TodoDelivery::Countdown(update).send(ctx)
}
