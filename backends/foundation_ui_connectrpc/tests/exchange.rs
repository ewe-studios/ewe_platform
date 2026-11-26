//! Drive generated application and UI clients through the same platform interface.
use buffa::Message;
use foundation_connectrpc::{ClientOptions, Request, RpcCtx, TransportError};
use foundation_core::valtron::valtron_test;
use foundation_netio::shared::http::{RequestDescriptor, SimpleHeader, SimpleHeaders, Status};
use foundation_ui_connectrpc::{
    v1, ConnectActionTransport, PlatformExchange, PlatformHttpClient, PlatformHttpTransport,
};
use foundation_ui_runtime::{action::ActionTransport, network::client_hello};
use foundation_ui_wire::{writer::ClientCaps, WireLimits};
use futures::future::BoxFuture;
use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
};

#[allow(clippy::all, clippy::pedantic, missing_docs, unused_qualifications)]
#[allow(unused_imports, dead_code)]
mod notes {
    include!(concat!(env!("OUT_DIR"), "/ewe.demo.v1.mod.rs"));
    include!(concat!(env!("OUT_DIR"), "/notes_service.rs"));
}

#[derive(Default)]
struct Network {
    opened: Mutex<Vec<String>>,
    exchanges: Mutex<VecDeque<Arc<Exchange>>>,
}
impl PlatformHttpClient for Network {
    fn exchange(
        &self,
        request: RequestDescriptor,
    ) -> Result<Arc<dyn PlatformExchange>, TransportError> {
        self.opened
            .lock()
            .unwrap()
            .push(request.request_uri.path().into());
        self.exchanges
            .lock()
            .unwrap()
            .pop_front()
            .map(|e| e as Arc<dyn PlatformExchange>)
            .ok_or(TransportError::Reset)
    }
}
struct Exchange {
    status: Status,
    content_type: &'static str,
    sent: Mutex<Option<Vec<u8>>>,
    chunks: Mutex<VecDeque<Result<Vec<u8>, TransportError>>>,
    closed: AtomicUsize,
    pending_at_end: bool,
}
impl Exchange {
    fn add(
        network: &Network,
        status: Status,
        content_type: &'static str,
        chunks: Vec<Result<Vec<u8>, TransportError>>,
        pending: bool,
    ) -> Arc<Self> {
        let exchange = Arc::new(Self {
            status,
            content_type,
            sent: Mutex::default(),
            chunks: Mutex::new(chunks.into()),
            closed: AtomicUsize::default(),
            pending_at_end: pending,
        });
        network
            .exchanges
            .lock()
            .unwrap()
            .push_back(exchange.clone());
        exchange
    }
}
impl PlatformExchange for Exchange {
    fn start(
        &self,
        body: Vec<u8>,
    ) -> BoxFuture<'_, Result<(Status, SimpleHeaders), TransportError>> {
        assert!(self.sent.lock().unwrap().replace(body).is_none());
        let mut headers = SimpleHeaders::new();
        headers.insert(SimpleHeader::CONTENT_TYPE, vec![self.content_type.into()]);
        Box::pin(std::future::ready(Ok((self.status.clone(), headers))))
    }
    fn read(&self, _: usize) -> BoxFuture<'_, Result<Option<Vec<u8>>, TransportError>> {
        let part = self.chunks.lock().unwrap().pop_front();
        if part.is_none() && self.pending_at_end {
            return Box::pin(std::future::pending());
        }
        Box::pin(std::future::ready(part.transpose()))
    }
    fn close(&self) {
        assert_eq!(self.closed.fetch_add(1, Ordering::Relaxed), 0);
    }
}

fn item(text: &str) -> notes::Item {
    notes::Item {
        text: text.into(),
        ..Default::default()
    }
}
fn envelope(msg: &notes::Item) -> Vec<u8> {
    let body = msg.encode_to_vec();
    let mut result = vec![0];
    result.extend(u32::try_from(body.len()).unwrap().to_be_bytes());
    result.extend(body);
    result
}

#[valtron_test(threads = 2, timeout = 10000)]
async fn generated_stream_delivers_split_and_combined_messages_before_eof() {
    let network = Arc::new(Network::default());
    let transport = Arc::new(PlatformHttpTransport::new(network.clone()));
    let client = notes::NotesClient::new(
        transport.clone(),
        "http://example.test/",
        ClientOptions::new(),
    )
    .unwrap();
    let first = envelope(&item("one"));
    let mut rest = first[2..].to_vec();
    rest.extend(envelope(&item("two")));
    let exchange = Exchange::add(
        &network,
        Status::OK,
        "application/connect+proto",
        vec![Ok(first[..2].into()), Ok(rest)],
        true,
    );
    let ctx = RpcCtx::background();
    let mut stream = client
        .watch(ctx.clone(), Request::new(item("request")))
        .await
        .unwrap();
    assert_eq!(stream.receive().await.unwrap().unwrap().text, "one");
    assert_eq!(stream.receive().await.unwrap().unwrap().text, "two");
    assert!(client
        .upload(ctx.clone(), futures::stream::empty())
        .await
        .is_err());
    drop(stream);
    // A canceled stream releases its own exchange. The unary call uses a different exchange.
    let reply = include_bytes!("../../foundation_ui_wire/fixtures/frame_ops_counter.bin").to_vec();
    let message = v1::UiReply {
        frame: Some(reply.clone()),
        ..Default::default()
    }
    .encode_to_vec();
    let mut caps = foundation_ui_wire::ServerCapsT::default();
    caps.schema_version = foundation_ui_wire::SCHEMA_VERSION;
    caps.vocabulary_version = foundation_ui_wire::vocab::VOCABULARY_VERSION;
    let caps_frame = foundation_ui_wire::writer::write_caps(
        &caps,
        foundation_ui_wire::writer::FrameHeader {
            stream_id: 0,
            sequence_id: 1,
            ack: 0,
        },
        &foundation_ui_wire::writer::WriterConfig::default(),
    )
    .unwrap();
    let caps_reply = v1::UiReply {
        frame: Some(caps_frame),
        ..Default::default()
    }
    .encode_to_vec();
    let handshake = Exchange::add(
        &network,
        Status::OK,
        "application/proto",
        vec![Ok(caps_reply)],
        false,
    );
    let other = Exchange::add(
        &network,
        Status::OK,
        "application/proto",
        vec![Ok(message)],
        false,
    );
    let generated =
        v1::ActionsClient::new(transport, "http://example.test/", ClientOptions::new()).unwrap();
    let sender = ConnectActionTransport::new(
        generated,
        ctx.ctx.clone(),
        ClientCaps::web_current(),
        WireLimits::default(),
    );
    let standard = client_hello(&ClientCaps::web_current());
    let mut original = foundation_ui_wire::read_frame(&standard, &WireLimits::default())
        .unwrap()
        .unpack();
    original.stream_id = 91;
    let offered = foundation_ui_wire::finish_frame(&original).unwrap();
    sender.send(offered.clone()).await.unwrap();
    assert_eq!(
        v1::UiRequest::decode_from_slice(handshake.sent.lock().unwrap().as_ref().unwrap())
            .unwrap()
            .hello,
        None
    );
    let mut input = foundation_ui_wire::FrameRequestT::default();
    input.action = Some(Box::default());
    let mut frame = foundation_ui_wire::FrameT::default();
    frame.schema_version = foundation_ui_wire::SCHEMA_VERSION;
    frame.kind = foundation_ui_wire::FrameKindT::FrameRequest(Box::new(input));
    let request = foundation_ui_wire::finish_frame(&frame).unwrap();
    assert_eq!(sender.send(request.clone()).await.unwrap().unwrap(), reply);
    let got =
        v1::UiRequest::decode_from_slice(other.sent.lock().unwrap().as_ref().unwrap()).unwrap();
    assert_eq!(got.frame, request);
    assert_eq!(got.hello.unwrap(), offered);
    assert_eq!(
        *network.opened.lock().unwrap(),
        [
            notes::procedure::WATCH,
            v1::procedure::EXCHANGE,
            v1::procedure::EXCHANGE
        ]
    );
    assert_eq!(other.closed.load(Ordering::Relaxed), 1);
    for (frame, valid) in [
        (None, true),
        (Some(Vec::new()), false),
        (Some(vec![0]), false),
    ] {
        let payload = v1::UiReply {
            frame,
            ..Default::default()
        }
        .encode_to_vec();
        let chunks = if payload.is_empty() {
            vec![]
        } else {
            vec![Ok(payload)]
        };
        Exchange::add(&network, Status::OK, "application/proto", chunks, false);
        let result = sender.send(request.clone()).await;
        if valid {
            assert_eq!(result.unwrap(), None);
        } else {
            assert!(result.is_err());
        }
    }
    for _ in 0..100 {
        if exchange.closed.load(Ordering::Relaxed) == 1 {
            break;
        }
        foundation_core::valtron::timer::sleep_for(std::time::Duration::from_millis(2), &ctx.ctx)
            .await;
    }
    assert_eq!(exchange.closed.load(Ordering::Relaxed), 1);
}

#[valtron_test(threads = 2, timeout = 10000)]
async fn failures_release_each_exchange_and_preserve_the_other_session() {
    use foundation_connectrpc::Code;
    let first = Arc::new(Network::default());
    let second = Arc::new(Network::default());
    let client = |network: Arc<Network>| {
        v1::ActionsClient::new(
            Arc::new(PlatformHttpTransport::new(network)),
            "http://account.test/",
            ClientOptions::new()
                .with_read_max_bytes(8)
                .with_error_body_max_bytes(64),
        )
        .unwrap()
    };
    let a = client(first.clone());
    let b = client(second.clone());
    let request = || Request::new(v1::UiRequest::default());
    let ctx = RpcCtx::background();
    let denied = Exchange::add(
        &first,
        Status::NotFound,
        "application/json",
        vec![Ok(br#"{"code":"not_found","message":"No record"}"#.to_vec())],
        false,
    );
    let err = a.exchange(ctx.clone(), request()).await.unwrap_err();
    assert_eq!(err.current_context().code(), Code::NotFound);
    assert_eq!(denied.closed.load(Ordering::Relaxed), 1);
    for part in [Ok(vec![0; 24]), Err(TransportError::Reset)] {
        let exchange = Exchange::add(&first, Status::OK, "application/proto", vec![part], false);
        assert!(a.exchange(ctx.clone(), request()).await.is_err());
        assert_eq!(exchange.closed.load(Ordering::Relaxed), 1);
    }
    let pending = Exchange::add(&first, Status::OK, "application/proto", vec![], true);
    let deadline = ctx
        .clone()
        .with_deadline(std::time::Duration::from_millis(20));
    assert_eq!(
        a.exchange(deadline, request())
            .await
            .unwrap_err()
            .current_context()
            .code(),
        Code::DeadlineExceeded
    );
    assert_eq!(pending.closed.load(Ordering::Relaxed), 1);
    let separate = Exchange::add(&second, Status::OK, "application/proto", vec![], false);
    assert!(b
        .exchange(ctx, request())
        .await
        .unwrap()
        .msg
        .frame
        .is_none());
    assert_eq!(separate.closed.load(Ordering::Relaxed), 1);
}
