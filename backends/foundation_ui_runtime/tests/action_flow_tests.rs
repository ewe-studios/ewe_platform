//! Submit a todo through the typed registry with the same context used for the reply.

use std::future::{poll_fn, Future};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::{Poll, Waker};

use foundation_context::Ctx;
use foundation_core::valtron::valtron_test;
use foundation_errstacks::ErrorTrace;
use foundation_ui_runtime::action::{
    encode_updates, invoke_action, Action, ActionError, ActionFuture, ActionRegistry, Context,
    ErasedAction, Interaction, InvokeError, Origin, ReceiverItem, Scope, Surface,
};
use foundation_ui_runtime::state::{
    install_state_signer, sign_state, verify_state, with_state_binding, HmacStateSigner,
    StateConfig, StateError,
};
use foundation_ui_wire::writer::{ClientCaps, FrameHeader, WriterConfig};
use foundation_ui_wire::{
    read_frame, ActionInputT, ActionRequestT, ErrorCode, OperationT, SetTextT, UiOperationT,
    WireLimits,
};

use super::support::{test_ctx, TTL, WALL};

struct AddTodo(Arc<AtomicUsize>);
struct CurrentUser(&'static str);

impl Action for AddTodo {
    type State = Vec<String>;
    const NAME: &'static str = "add_todo";

    fn origin() -> Option<Origin> {
        None
    }
    fn scope() -> Scope {
        Scope::All
    }
    fn call<'a>(&'a self, ctx: &'a Context, event: Interaction<Self::State>) -> ActionFuture<'a> {
        Box::pin(async move {
            self.0.fetch_add(1, Ordering::Relaxed);
            assert_eq!(event.event_name, "submit");
            assert_eq!(event.node_id, 42);
            let user = ctx.get::<CurrentUser>().unwrap();
            let root = ctx.state_root().unwrap();
            let fail = event.input_value.is_empty();
            let mut todos = event.state;
            todos.push(event.input_value);
            let mut text = SetTextT::default();
            text.node_id = root;
            text.text = Some(format!("{}: {}", user.0, todos.join(", ")));
            let mut op = OperationT::default();
            op.ui = UiOperationT::SetText(Box::new(text));
            ctx.receiver().render_op(op);
            ctx.receiver().state_for(&[root, event.node_id], &todos)?;
            let mut paused = false;
            poll_fn(|task| {
                if paused {
                    return Poll::Ready(());
                }
                paused = true;
                task.waker().wake_by_ref();
                Poll::Pending
            })
            .await;
            if fail {
                return Err(ErrorTrace::new(ActionError::new(
                    ErrorCode::ActionFailed,
                    "Enter a todo title",
                )));
            }
            Ok(())
        })
    }
}

fn registry() -> (ActionRegistry, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut actions = ActionRegistry::new();
    actions.register(AddTodo(calls.clone())).unwrap();
    (actions, calls)
}

fn bound_user(base: &Ctx, user: &'static str) -> Ctx {
    let ctx = with_state_binding(base, user.as_bytes()).unwrap();
    ctx.store(CurrentUser(user));
    ctx
}

fn request(value: &str, signed: Vec<u8>) -> ActionRequestT {
    let mut input = ActionInputT::default();
    input.event_name = Some("submit".into());
    input.value = Some(value.into());
    input.parent_node = 999;
    let mut request = ActionRequestT::default();
    request.action = Some("server:add_todo".into());
    request.node_id = 42;
    request.input = Some(Box::new(input));
    request.state = Some(signed);
    request
}

async fn submit(
    ctx: &Ctx,
    action: &dyn ErasedAction,
    value: &str,
    state: Vec<u8>,
) -> Result<Vec<ReceiverItem>, InvokeError> {
    invoke_action(ctx, action, Surface::Web, Some(40), request(value, state)).await
}

fn check_reply(
    ctx: &Ctx,
    items: Vec<ReceiverItem>,
    expected_view: &str,
    expected_items: &[&str],
) -> Vec<u8> {
    let header = FrameHeader {
        stream_id: 1,
        sequence_id: 2,
        ack: 1,
    };
    let bytes = encode_updates(
        ctx,
        items,
        &ClientCaps::web_current(),
        header,
        &WriterConfig::default(),
    )
    .unwrap();
    let messages = read_frame(&bytes, &WireLimits::default())
        .unwrap()
        .kind_as_frame_message()
        .unwrap()
        .messages()
        .unwrap();
    assert_eq!(messages.len(), 3);
    let text = messages
        .get(0)
        .operation()
        .unwrap()
        .ui_as_set_text()
        .unwrap();
    assert_eq!(text.node_id(), 40);
    assert_eq!(text.text(), Some(expected_view));
    let root = messages
        .get(1)
        .operation()
        .unwrap()
        .ui_as_attach_state()
        .unwrap();
    let input = messages
        .get(2)
        .operation()
        .unwrap()
        .ui_as_attach_state()
        .unwrap();
    assert_eq!((root.root_node(), input.root_node()), (40, 42));
    let signed = input.state().unwrap().bytes();
    assert_eq!(root.state().unwrap().bytes(), signed);
    let body = verify_state(ctx, signed).unwrap();
    assert_eq!(
        serde_json::from_slice::<Vec<String>>(&body).unwrap(),
        expected_items
    );
    signed.to_vec()
}

#[valtron_test]
async fn repeated_submits_verify_the_state_attached_by_the_previous_render() {
    let (base, clock) = test_ctx(WALL);
    install_state_signer(
        &base,
        Arc::new(HmacStateSigner::new([9; 32], StateConfig::default()).unwrap()),
    );
    let alice = bound_user(&base, "Alice");
    let bob = bound_user(&base, "Bob");
    let (registry, calls) = registry();
    let action = registry
        .get(Origin::Server, "add_todo", Surface::Web)
        .unwrap();
    let start = sign_state(&alice, b"[]").unwrap();
    let first = submit(&alice, action, "Buy milk", start.clone())
        .await
        .unwrap();
    let next = check_reply(&alice, first, "Alice: Buy milk", &["Buy milk"]);
    let second = submit(&alice, action, "Get eggs", next).await.unwrap();
    check_reply(
        &alice,
        second,
        "Alice: Buy milk, Get eggs",
        &["Buy milk", "Get eggs"],
    );

    assert!(matches!(
        submit(&bob, action, "Wrong user", start.clone()).await,
        Err(InvokeError::State(StateError::BadTag))
    ));
    let malformed = sign_state(&alice, b"false").unwrap();
    assert!(
        matches!(submit(&alice,action,"Wrong type",malformed).await, Err(InvokeError::Action(error)) if error.current_context().code == ErrorCode::StateRejected)
    );
    clock.set_elapsed(TTL + 1);
    assert!(matches!(
        submit(&alice, action, "Too late", start).await,
        Err(InvokeError::State(StateError::Expired))
    ));
    assert_eq!(calls.load(Ordering::Relaxed), 2);
}

#[valtron_test]
async fn overlapping_submits_keep_users_receivers_and_failures_local() {
    let (base, _) = test_ctx(WALL);
    install_state_signer(
        &base,
        Arc::new(HmacStateSigner::new([9; 32], StateConfig::default()).unwrap()),
    );
    let alice = bound_user(&base, "Alice");
    let bob = bound_user(&base, "Bob");
    let (registry, _) = registry();
    let action = registry
        .get(Origin::Server, "add_todo", Surface::Web)
        .unwrap();
    let alice_start = sign_state(&alice, b"[]").unwrap();
    let bob_start = sign_state(&bob, b"[]").unwrap();
    let mut alice_call = std::pin::pin!(submit(&alice, action, "A", alice_start.clone()));
    let mut failing_call = std::pin::pin!(submit(&alice, action, "", alice_start));
    {
        let mut task = std::task::Context::from_waker(Waker::noop());
        assert!(alice_call.as_mut().poll(&mut task).is_pending());
        assert!(failing_call.as_mut().poll(&mut task).is_pending());
    }
    let items = submit(&bob, action, "B", bob_start).await.unwrap();
    let bob_next = check_reply(&bob, items, "Bob: B", &["B"]);
    assert!(verify_state(&alice, &bob_next).is_err());
    assert!(
        matches!(failing_call.await,Err(InvokeError::Action(error)) if error.current_context().code == ErrorCode::ActionFailed)
    );
    let alice_next = check_reply(&alice, alice_call.await.unwrap(), "Alice: A", &["A"]);
    assert!(verify_state(&bob, &alice_next).is_err());
}
