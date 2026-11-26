//! The sandbox lifetime deadline and the one alarm (feature 18).
//!
//! Each test takes its own pool lease through [`super::support::block_on`].

use std::time::Duration;

use foundation_sandbox::shared::cloudflare::control::SANDBOX_ID_KEY;
use foundation_sandbox::shared::cloudflare::store::SANDBOX_KEY;
use foundation_sandbox::shared::cloudflare::testing::{
    control_get_request, response_body, response_status,
};
use foundation_sandbox::shared::cloudflare::StopReason;
use foundation_sandbox::shared::cloudflare::{ContainerApi, ContainerStart, DoControlPlane};
use foundation_sandbox::shared::control::SandboxState;
use foundation_sandbox::shared::driver::{SlaveEndpoint, StoredRecord};
use foundation_sandbox::shared::ids::{DriverId, SandboxId};
use foundation_sandbox::shared::rpc::messages::SandboxInfo;
use foundation_sandbox::shared::spec::SandboxSpec;
use proptest::prelude::*;
use proptest::test_runner::RngSeed;

use super::support::{self, block_on, spec, Harness};

/// The idle timeout of [`support::config`], in whole milliseconds.
const IDLE_MS: u64 = 600_000;

/// The test spec with a lifetime limit.
fn spec_with_ttl(ttl: Option<Duration>) -> SandboxSpec {
    let mut spec = spec();
    spec.ttl = ttl;
    spec
}

/// Restores the sandbox through one `Get` request.
fn get(planner: &DoControlPlane, id: &SandboxId) -> (u16, Vec<u8>) {
    let response = block_on(planner.handle(control_get_request(id)));
    (response_status(&response), response_body(response))
}

/// The `SandboxInfo` of one successful `Get`.
fn info(planner: &DoControlPlane, id: &SandboxId) -> SandboxInfo {
    let (status, body) = get(planner, id);
    assert_eq!(status, 200, "the sandbox is ready");
    serde_json::from_slice(&body).expect("the info is JSON")
}

/// Creates the one sandbox and sends one request, so the alarm is armed.
fn create_and_touch(h: &Harness, planner: &DoControlPlane, ttl: Option<Duration>) -> SandboxInfo {
    block_on(planner.sandboxes().create(spec_with_ttl(ttl))).expect("create");
    info(planner, &h.id())
}

#[test]
fn alarm_at_earlier_deadline() {
    // A lifetime shorter than the idle timeout is the alarm time.
    let h = Harness::new(200, true, false);
    let planner = support::planner(&h, Some(h.id()));
    let info = create_and_touch(&h, &planner, Some(Duration::from_secs(300)));
    assert_eq!(info.expires_at_unix_ms, Some(300_000));
    assert_eq!(h.alarm.value(), Some(300_000));

    // A lifetime longer than the idle timeout leaves the idle time.
    let h = Harness::new(201, true, false);
    let planner = support::planner(&h, Some(h.id()));
    let info = create_and_touch(&h, &planner, Some(Duration::from_mins(20)));
    assert_eq!(info.expires_at_unix_ms, Some(1_200_000));
    assert_eq!(h.alarm.value(), Some(IDLE_MS));

    // No lifetime leaves the idle time.
    let h = Harness::new(202, true, false);
    let planner = support::planner(&h, Some(h.id()));
    let info = create_and_touch(&h, &planner, None);
    assert_eq!(info.expires_at_unix_ms, None);
    assert_eq!(h.alarm.value(), Some(IDLE_MS));
}

#[test]
fn expires_before_idle() {
    let h = Harness::new(203, true, false);
    let planner = support::planner(&h, Some(h.id()));
    let _ = create_and_touch(&h, &planner, Some(Duration::from_secs(60)));
    assert_eq!(h.alarm.value(), Some(60_000));

    h.clock.advance(Duration::from_secs(61));
    h.clock.advance_wall(Duration::from_secs(61));
    block_on(planner.on_alarm());

    assert!(!h.container.is_running());
    assert!(h.container.destroy_count() > 0);
    assert_eq!(h.alarm.value(), None);
    assert_eq!(h.storage.value(SANDBOX_KEY), None);
    assert_eq!(planner.last_stop_reason(), Some(StopReason::Expired));
    let (status, _body) = get(&planner, &h.id());
    assert_ne!(status, 200, "the record is gone");
}

#[test]
fn idle_before_expires() {
    let h = Harness::new(204, true, false);
    let planner = support::planner(&h, Some(h.id()));
    let _ = create_and_touch(&h, &planner, Some(Duration::from_secs(3_600)));
    assert_eq!(h.alarm.value(), Some(IDLE_MS));

    h.clock.advance(Duration::from_secs(601));
    h.clock.advance_wall(Duration::from_secs(601));
    block_on(planner.on_alarm());

    assert!(!h.container.is_running());
    assert_eq!(h.alarm.value(), None);
    assert_eq!(h.storage.value(SANDBOX_KEY), None);
    assert_eq!(planner.last_stop_reason(), Some(StopReason::Idle));
}

#[test]
fn restore_expired_removes() {
    let h = Harness::new(205, true, false);
    h.container
        .start(ContainerStart {
            entrypoint: h.host.config.slave_entrypoint(),
            enable_internet: false,
            env: Vec::new(),
        })
        .expect("start");
    let record = StoredRecord {
        id: h.id(),
        driver: DriverId::CloudflareContainer,
        spec: spec_with_ttl(Some(Duration::from_secs(60))),
        state: SandboxState::Ready,
        created_unix_ms: 0,
        expires_at_unix_ms: Some(60_000),
        endpoint: Some(SlaveEndpoint::CloudflarePort { port: 1024 }),
    };
    h.storage.insert(
        SANDBOX_KEY,
        serde_json::to_vec(&record).expect("the record is JSON"),
    );
    h.storage
        .insert(SANDBOX_ID_KEY, h.id().to_string().into_bytes());
    h.clock.advance_wall(Duration::from_secs(61));

    let planner = support::planner(&h, None);
    let (status, _body) = get(&planner, &h.id());

    assert_ne!(status, 200);
    assert!(!h.container.is_running());
    assert!(h.container.destroy_count() > 0);
    assert_eq!(h.storage.value(SANDBOX_KEY), None);
    assert!(h.index.is_empty());
}

#[test]
fn touch_keeps_expiry() {
    let h = Harness::new(206, true, false);
    let planner = support::planner(&h, Some(h.id()));
    let before = create_and_touch(&h, &planner, Some(Duration::from_secs(1_000)));
    assert_eq!(before.expires_at_unix_ms, Some(1_000_000));
    assert_eq!(h.alarm.value(), Some(IDLE_MS));

    // A touch after the first idle deadline moves the idle deadline only.
    h.clock.advance_wall(Duration::from_secs(500));
    let after = info(&planner, &h.id());
    assert_eq!(after.expires_at_unix_ms, Some(1_000_000));
    assert_eq!(after.created_unix_ms, before.created_unix_ms);
    assert_eq!(
        h.alarm.value(),
        Some(1_000_000),
        "the lifetime deadline is now the earlier one"
    );

    // A later touch still does not move the lifetime deadline.
    h.clock.advance_wall(Duration::from_secs(100));
    let after = info(&planner, &h.id());
    assert_eq!(after.expires_at_unix_ms, Some(1_000_000));
    assert_eq!(h.alarm.value(), Some(1_000_000));
}

#[test]
fn old_client_has_no_ttl() {
    let mut body = serde_json::to_value(spec()).expect("the spec is JSON");
    let object = body.as_object_mut().expect("the spec is a JSON object");
    object.remove("ttl");
    let decoded: SandboxSpec = serde_json::from_value(body).expect("a body with no ttl decodes");
    assert_eq!(decoded.ttl, None);
}

/// The model of one lifetime run.
struct Model {
    /// Whether the sandbox runs.
    alive: bool,
    /// The idle deadline in Unix milliseconds.
    idle: u64,
    /// The lifetime deadline in Unix milliseconds.
    expires: Option<u64>,
    /// How many stops the alarm made.
    stops: usize,
    /// The reason of the last stop.
    reason: Option<StopReason>,
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 16,
        rng_seed: RngSeed::Fixed(0x5eed_0012),
        ..ProptestConfig::default()
    })]

    /// Random lifetimes, clock changes, requests and alarms against the one
    /// alarm. The stop count and the last stop reason equal the model.
    #[test]
    fn lifetime_model(
        ttl_secs in prop::option::of(30_u64..3_600),
        raw in prop::collection::vec((0_u8..10, 0_u64..900), 0..8),
    ) {
        let h = Harness::new(0xf18, true, false);
        let planner = support::planner(&h, Some(h.id()));
        let ttl = ttl_secs.map(Duration::from_secs);
        let _ = create_and_touch(&h, &planner, ttl);
        let mut model = Model {
            alive: true,
            idle: IDLE_MS,
            expires: ttl_secs.map(|seconds| seconds * 1_000),
            stops: 0,
            reason: None,
        };
        let mut now = 0_u64;
        for (kind, delta) in raw {
            if kind < 5 {
                now = now.saturating_add(delta * 1_000);
                h.clock.advance_wall(Duration::from_secs(delta));
                h.clock.advance(Duration::from_secs(delta));
            }
            if kind % 10 <= 4 {
                let (status, body) = get(&planner, &h.id());
                if model.alive {
                    prop_assert_eq!(status, 200);
                    let info: SandboxInfo =
                        serde_json::from_slice(&body).expect("the info is JSON");
                    prop_assert_eq!(info.expires_at_unix_ms, model.expires);
                    model.idle = model.idle.max(now.saturating_add(IDLE_MS));
                    let arm = min_deadline(Some(model.idle), model.expires);
                    prop_assert_eq!(h.alarm.value(), arm);
                } else {
                    prop_assert_ne!(status, 200);
                }
            } else {
                block_on(planner.on_alarm());
                let expired = model.expires.is_some_and(|expires| now >= expires);
                let idle = now >= model.idle;
                if model.alive && (expired || idle) {
                    model.alive = false;
                    model.stops += 1;
                    model.reason = Some(if expired {
                        StopReason::Expired
                    } else {
                        StopReason::Idle
                    });
                }
                if model.alive {
                    let arm = min_deadline(Some(model.idle), model.expires);
                    prop_assert_eq!(h.alarm.value(), arm);
                } else {
                    prop_assert_eq!(h.alarm.value(), None);
                }
            }
            prop_assert_eq!(h.container.destroy_count(), model.stops);
            prop_assert_eq!(planner.last_stop_reason(), model.reason);
            let listed = block_on(planner.sandboxes().list()).expect("list");
            prop_assert_eq!(listed.len(), usize::from(model.alive));
        }
    }
}

/// The earlier of two optional deadlines, as the control plane computes it.
fn min_deadline(idle: Option<u64>, expires: Option<u64>) -> Option<u64> {
    match (idle, expires) {
        (Some(idle), Some(expires)) => Some(idle.min(expires)),
        (Some(deadline), None) | (None, Some(deadline)) => Some(deadline),
        (None, None) => None,
    }
}
