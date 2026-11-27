use foundation_context::{
    Clock, Ctx, Duration, Instant, SystemTime,
    env::{CounterSource, Env, SeededEntropy, Timer, TimerFuture},
};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

struct TestClock {
    origin: Instant,
    seconds: AtomicU64,
}

impl Clock for TestClock {
    fn elapsed(&self) -> Duration {
        Duration::from_secs(self.seconds.load(Ordering::Relaxed))
    }

    fn now(&self) -> Instant {
        self.origin + self.elapsed()
    }

    fn wall_now(&self) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(100) + self.elapsed()
    }
}

struct AdvancingTimer(Arc<TestClock>);

impl Timer for AdvancingTimer {
    fn sleep_until(&self, _: Ctx, deadline: Instant) -> TimerFuture {
        self.0
            .seconds
            .store((deadline - self.0.origin).as_secs(), Ordering::Relaxed);
        Box::pin(std::future::ready(()))
    }
}
use foundation_ui_runtime::action::{Origin, invoke_action, prepare_messages};
use foundation_ui_wire::{ActionInputT, ActionRequestT, MountSurface};
use serde_json::{Value, json};

fn call(ctx: &Ctx, name: &str, payload: Value) -> Value {
    let registry = ui_todo::server_registry().unwrap();
    let entry = registry
        .get(Origin::Server, name, MountSurface::Web)
        .unwrap();
    let mut input = ActionInputT::default();
    input.json = Some(serde_json::to_vec(&payload).unwrap());
    let mut request = ActionRequestT::default();
    request.input = Some(Box::new(input));
    let updates =
        futures_lite::future::block_on(invoke_action(ctx, entry, MountSurface::Web, None, request))
            .unwrap();
    let messages = prepare_messages(ctx, updates).unwrap();
    let bytes = messages[0]
        .data
        .as_ref()
        .unwrap()
        .msgpack
        .as_ref()
        .unwrap()
        .bytes
        .as_ref()
        .unwrap();
    let (topic, data): (String, Value) = rmp_serde::from_slice(bytes).unwrap();
    assert_eq!(topic, "ewe.todo.v1");
    data
}

/// WHY: A lost reply must not restart a client's block or pause another client.
/// WHAT: Repeating one request returns the same block. A second client has no active block.
#[test]
fn a_retried_block_request_reuses_the_original_block() {
    let clock = Arc::new(TestClock {
        origin: Instant::now(),
        seconds: AtomicU64::new(0),
    });
    let inputs = Ctx::with_seed(clock.clone(), 42);
    inputs.store(Env::new(
        Arc::new(AdvancingTimer(clock.clone())),
        Arc::new(SeededEntropy::new(42)),
        Arc::new(CounterSource::default()),
    ));
    let channel =
        foundation_wasm_ui::server::LiveChannel::new(1, Default::default(), Default::default());
    let app =
        ui_todo::server::app_with_context(ui_todo::TodoStore::default(), channel, inputs).unwrap();
    let ctx = app.context();
    let session = "00000000-0000-4000-8000-000000000001";
    let other = "00000000-0000-4000-8000-000000000002";
    let request = "00000000-0000-4000-8000-000000000003";
    let issued_at_ms = foundation_compact::time::unix_millis(ctx.clock().wall_now());
    let payload = json!({"session": session, "request": request, "issued_at_ms": issued_at_ms});
    let original = call(ctx, "request_countdown", payload.clone());
    let retried = call(ctx, "request_countdown", payload.clone());
    let original = &original["Countdown"]["Active"]["block"];
    let retried = &retried["Countdown"]["Active"]["block"];
    assert!(original["id"].is_string());
    assert!(original["expires_at_ms"].is_number());
    assert_eq!(original["id"], retried["id"]);
    assert_eq!(original["expires_at_ms"], retried["expires_at_ms"]);
    assert_eq!(
        call(ctx, "current_countdown", json!({"session": other})),
        json!({"Countdown":{"Clear":{"scope":{"session":other},"observed":null}}})
    );
    assert_eq!(original["expires_at_ms"], 160_000);
    let id = &original["id"];
    clock.seconds.store(59, Ordering::Relaxed);
    let observed = json!({"session": session, "observed": id, "await_release": true});
    assert_eq!(
        call(ctx, "current_countdown", observed.clone()),
        json!({"Countdown":{"Released":{"scope":{"session":session},"id":id}}})
    );
    assert_eq!(clock.elapsed(), Duration::from_secs(60));
    assert_eq!(
        call(ctx, "current_countdown", observed.clone()),
        json!({"Countdown":{"Released":{"scope":{"session":session},"id":id}}})
    );
    assert_eq!(
        call(ctx, "current_countdown", observed),
        json!({"Countdown":{"Clear":{"scope":{"session":session},"observed":id}}})
    );
    assert_eq!(
        &call(ctx, "request_countdown", payload)["Countdown"]["Receipt"]["block"],
        original,
    );
}
