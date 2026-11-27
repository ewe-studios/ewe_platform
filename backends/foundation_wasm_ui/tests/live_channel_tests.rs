//! Check the public stream contract with independent readers and concurrent producers.
use foundation_ui_wire::{
    ops, read_frame,
    writer::{ClientCaps, WriterConfig},
    MessageT, Platform, WireLimits,
};
use foundation_wasm_ui::server::{BacklogConfig, FrameTransport, LiveChannel, LiveError};
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
struct Recorder(Arc<Mutex<Vec<Vec<u8>>>>);

impl FrameTransport for Recorder {
    fn send_binary(&mut self, bytes: &[u8]) -> bool {
        self.0.lock().unwrap().push(bytes.to_vec());
        true
    }
    fn send_text(&mut self, _: &str) -> bool {
        panic!("expected a binary frame")
    }
}

impl Recorder {
    fn sequences(&self) -> Vec<u64> {
        self.0
            .lock()
            .unwrap()
            .iter()
            .map(|bytes| {
                let frame = read_frame(bytes, &WireLimits::default()).unwrap();
                assert_eq!(frame.stream_id(), 7);
                frame.sequence_id()
            })
            .collect()
    }
}
fn channel() -> LiveChannel {
    LiveChannel::new(7, BacklogConfig::default(), WriterConfig::default())
}
fn update() -> Vec<MessageT> {
    vec![
        ops::message(ops::set_text(42, "next")),
        ops::message(ops::remove_child(40, 42)),
    ]
}

#[test]
fn writer_selects_each_surface_and_preserves_sequence_after_failure() {
    let channel = channel();
    let web = Recorder::default();
    let native = Recorder::default();
    channel
        .register(web.clone(), &ClientCaps::web_current(), 0)
        .unwrap();
    channel
        .register(
            native.clone(),
            &ClientCaps::native_current(Platform::Unknown),
            0,
        )
        .unwrap();
    assert!(matches!(
        channel.sender().push_messages(vec![MessageT::default()]),
        Err(LiveError::Write(_))
    ));
    assert_eq!(channel.sender().push_messages(update()).unwrap(), 1);
    assert_eq!(web.sequences(), [1]);
    assert_eq!(native.sequences(), [1]);
    for (client, count) in [(web, 2), (native, 1)] {
        let history = client.0.lock().unwrap();
        let frame = read_frame(&history[0], &WireLimits::default()).unwrap();
        assert_eq!(
            frame
                .kind_as_frame_message()
                .unwrap()
                .messages()
                .unwrap()
                .len(),
            count
        );
    }
}

#[test]
fn restored_channel_requires_a_snapshot_for_missing_history() {
    let channel = LiveChannel::resume(7, 5, BacklogConfig::default(), WriterConfig::default());
    let client = Recorder::default();
    let caps = ClientCaps::web_current();
    assert_eq!(
        channel.register(client.clone(), &caps, 4),
        Err(LiveError::ResumeTooOld { oldest: 6 })
    );
    channel.register(client.clone(), &caps, 5).unwrap();
    channel.sender().push_messages(update()).unwrap();
    assert_eq!(client.sequences(), [6]);
    let replay = Recorder::default();
    channel.register(replay.clone(), &caps, 5).unwrap();
    assert_eq!(replay.sequences(), [6]);
}

#[test]
fn resume_covers_exactly_the_retained_frames() {
    let channel = LiveChannel::new(
        7,
        BacklogConfig::default().max_frames(2),
        WriterConfig::default(),
    );
    for _ in 0..5 {
        channel.sender().push_messages(update()).unwrap();
    }
    let client = Recorder::default();
    let caps = ClientCaps::web_current();
    assert_eq!(
        channel.register(client.clone(), &caps, 2),
        Err(LiveError::ResumeTooOld { oldest: 4 })
    );
    assert_eq!(
        channel.register(client.clone(), &caps, 6),
        Err(LiveError::ResumeAhead { last: 5 })
    );
    let mut old = caps.clone();
    old.vocabulary_version = 0;
    assert_eq!(
        channel.register(client.clone(), &old, 4),
        Err(LiveError::UpdateRequired)
    );
    assert_eq!(client.sequences(), [0_u64; 0]);
    let replay = channel.subscribe_web(client.clone(), 3).unwrap();
    let direct = Recorder::default();
    channel.register(direct.clone(), &caps, 3).unwrap();
    channel.sender().push_messages(update()).unwrap();
    assert_eq!(client.sequences(), [6]);
    assert_eq!(direct.sequences(), [4, 5, 6]);
    let initial: Vec<_> = replay
        .iter()
        .map(|bytes| {
            read_frame(bytes, &WireLimits::default())
                .unwrap()
                .sequence_id()
        })
        .collect();
    assert_eq!(initial, [4, 5]);
    assert_eq!(channel.backlog_len(), 2);
    channel.close();
    assert_eq!(channel.conn_count(), 0);
}

struct Closed;
impl FrameTransport for Closed {
    fn send_binary(&mut self, _: &[u8]) -> bool {
        false
    }
    fn send_text(&mut self, _: &str) -> bool {
        false
    }
}

#[test]
fn concurrent_register_and_push_share_one_order() {
    let channel = channel();
    channel
        .register(Closed, &ClientCaps::web_current(), 0)
        .unwrap();
    let tx = channel.sender();
    let thread = std::thread::spawn(move || {
        for _ in 0..40 {
            tx.push_messages(update()).unwrap();
        }
    });
    let client = Recorder::default();
    channel
        .register(client.clone(), &ClientCaps::web_current(), 0)
        .unwrap();
    for _ in 0..40 {
        channel.sender().push_messages(update()).unwrap();
    }
    thread.join().unwrap();
    assert_eq!(client.sequences(), (1..=80).collect::<Vec<_>>());
    assert_eq!(channel.conn_count(), 1);
    assert_eq!(
        channel.register(Closed, &ClientCaps::web_current(), 0),
        Err(LiveError::ConnectionClosed)
    );
    assert_eq!(channel.conn_count(), 1);
}

#[test]
fn a_mounted_signal_sends_complete_app_batches() {
    use foundation_wasm_ui::{html, server::FrameBroadcastSink, App};

    let channel = channel();
    let client = Recorder::default();
    channel
        .register(client.clone(), &ClientCaps::web_current(), 0)
        .unwrap();
    let app = App::with_protocol(FrameBroadcastSink::new(channel.sender()));
    let (ctx, receiver) = app.context();
    let (count, set_count) = ctx.signal(1i64);
    let tree = html! { ctx, receiver, <span>{count.get()}</span> };
    app.mount(tree);
    app.stabilize().unwrap();
    set_count.set(5);
    app.stabilize().unwrap();
    let history = client.0.lock().unwrap();
    let latest = read_frame(history.last().unwrap(), &WireLimits::default()).unwrap();
    let messages = latest.kind_as_frame_message().unwrap().messages().unwrap();
    assert!(messages
        .iter()
        .filter_map(|msg| msg.operation()?.ui_as_set_text())
        .any(|op| op.text() == Some("5")));
}

#[test]
fn native_subscription_filters_replay_and_live_frames() {
    let channel = channel();
    channel.sender().push_messages(update()).unwrap();
    let client = Recorder::default();
    let replay = channel.subscribe_native(client.clone(), 0).unwrap();
    assert_eq!(replay.len(), 1);
    channel.sender().push_messages(update()).unwrap();
    let live = client.0.lock().unwrap();
    let limits = WireLimits::default();
    for (bytes, sequence) in [(replay[0].as_ref(), 1), (live[0].as_slice(), 2)] {
        let frame = read_frame(bytes, &limits).unwrap();
        assert_eq!(frame.sequence_id(), sequence);
        let rows = frame.kind_as_frame_message().unwrap().messages().unwrap();
        assert_eq!(rows.len(), 1);
        assert!(rows.get(0).operation().unwrap().ui_as_set_text().is_some());
    }
}

#[test]
fn managed_listener_starts_at_latest_and_releases_without_another_update() {
    use foundation_ui_runtime::dispatch::ReplayFeed;
    let feed = channel();
    feed.sender().push_messages(update()).unwrap();
    let mut observer = Recorder::default();
    let seen = observer.clone();
    let caps = ClientCaps::web_current();
    let lease = feed
        .listen(
            &caps,
            Box::new(move |event| match event {
                foundation_ui_runtime::dispatch::FeedEvent::Frame(bytes) => {
                    observer.send_binary(bytes)
                }
                foundation_ui_runtime::dispatch::FeedEvent::Disconnected => false,
            }),
        )
        .unwrap();
    assert_eq!(feed.conn_count(), 1);
    assert_eq!(seen.sequences(), [0_u64; 0]);
    feed.sender().push_messages(update()).unwrap();
    assert_eq!(seen.sequences(), [2]);
    let replay = feed.replay(&caps, 0).unwrap();
    assert_eq!(replay.len(), 2);
    assert_eq!(seen.sequences(), [2]);
    drop(lease);
    assert_eq!(feed.conn_count(), 0);
}

#[test]
fn supported_older_clients_receive_live_and_replayed_frames_in_their_vocabulary() {
    use foundation_ui_runtime::dispatch::{FeedEvent, ReplayFeed};
    use foundation_ui_wire::{CreateNodeT, NodeKind, OperationT, UiOperationT};

    let feed = channel();
    let update = || {
        let mut operation = OperationT::default();
        let mut node = CreateNodeT::default();
        node.node_id = 42;
        node.kind = NodeKind::DRAWING_SURFACE;
        operation.ui = UiOperationT::CreateNode(Box::new(node));
        vec![ops::message(operation)]
    };
    let mut caps = ClientCaps::native_current(Platform::Ios);
    caps.vocabulary_version = 11;
    feed.sender().push_messages(update()).unwrap();
    let mut client = Recorder::default();
    let seen = client.clone();
    let _lease = feed
        .listen(
            &caps,
            Box::new(move |event| match event {
                FeedEvent::Frame(bytes) => client.send_binary(bytes),
                FeedEvent::Disconnected => false,
            }),
        )
        .unwrap();
    feed.sender().push_messages(update()).unwrap();
    let kind = |bytes: &[u8]| {
        read_frame(bytes, &WireLimits::default())
            .unwrap()
            .kind_as_frame_message()
            .unwrap()
            .messages()
            .unwrap()
            .get(0)
            .operation()
            .unwrap()
            .ui_as_create_node()
            .unwrap()
            .kind()
    };
    assert_eq!(seen.sequences(), [2]);
    assert_eq!(kind(&seen.0.lock().unwrap()[0]), NodeKind::UNKNOWN);
    for bytes in feed.replay(&caps, 0).unwrap() {
        assert_eq!(kind(&bytes), NodeKind::UNKNOWN);
    }
    let current = ClientCaps::native_current(Platform::Ios);
    for bytes in feed.replay(&current, 0).unwrap() {
        assert_eq!(kind(&bytes), NodeKind::DRAWING_SURFACE);
    }
}

#[test]
fn builder_receives_the_published_position_and_failure_keeps_it_available() {
    let channel = channel();
    let sender = channel.sender();
    assert_eq!(sender.push_messages(update()).unwrap(), 1);
    let failed: Result<u64, LiveError> = sender.push_with(|position| {
        assert_eq!(position, 2);
        Err(LiveError::ConnectionClosed)
    });
    assert!(matches!(failed, Err(LiveError::ConnectionClosed)));
    assert_eq!(channel.last_sequence(), 1);
    assert_eq!(
        sender
            .push_with::<LiveError>(|position| {
                assert_eq!(position, 2);
                Ok(update())
            })
            .unwrap(),
        2
    );
}

#[test]
fn reserved_frames_publish_after_commit_and_disappear_after_cancellation() {
    let channel = channel();
    let sender = channel.sender();
    let make = |_| Ok::<_, LiveError>(update());
    let abandoned = sender.reserve(make).unwrap();
    let reader = Recorder::default();
    channel
        .register(reader.clone(), &ClientCaps::web_current(), 0)
        .unwrap();
    assert_eq!(
        sender.push_messages(update()),
        Err(LiveError::PublicationInProgress)
    );
    assert_eq!(channel.last_sequence(), 0);
    drop(abandoned);
    assert_eq!(reader.sequences(), Vec::<u64>::new());
    let committed = sender.reserve(make).unwrap();
    assert_eq!(committed.position(), 1);
    let next_reader = Recorder::default();
    channel.subscribe_native(next_reader.clone(), 0).unwrap();
    committed.publish();
    assert_eq!(reader.sequences(), [1]);
    assert_eq!(next_reader.sequences(), [1]);
    assert_eq!(sender.push_messages(update()).unwrap(), 2);
}
