//! The in-memory network: one unary call with no socket.
//!
//! Feature 04 task T18: a client and a server in one process.

#![cfg(unix)]

use std::sync::Arc;

use buffa::encoding::{decode_varint, encode_varint, skip_field, Tag, WireType};
use buffa::{DecodeContext, DecodeError, DefaultInstance, EncodeSink, Message, SizeCache};
use bytes::{Buf, Bytes};
use foundation_connectrpc::shared::codec::CodecFor;
use foundation_connectrpc::shared::transport::base::round_trip;
use foundation_connectrpc::testing::{request_descriptor, MemNetwork, MemTransport};
use foundation_connectrpc::{
    HandlerOptions, JsonCodec, ProcedureCodecs, Request, Response, Router,
};
use foundation_core::valtron::valtron_test;
use foundation_netio::shared::http::{SimpleHeader, SimpleHeaders, Status};

const ECHO_PATH: &str = "/test.EchoService/Echo";
const ECHO_URL: &str = "http://mem/test.EchoService/Echo";

#[derive(Clone, Default, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
struct TestMsg {
    id: i32,
    name: String,
}

impl DefaultInstance for TestMsg {
    fn default_instance() -> &'static Self {
        static INST: std::sync::OnceLock<TestMsg> = std::sync::OnceLock::new();
        INST.get_or_init(TestMsg::default)
    }
}

impl Message for TestMsg {
    fn compute_size(&self, _cache: &mut SizeCache) -> u32 {
        let mut size = 0usize;
        size += 1 + buffa::types::int32_encoded_len(self.id);
        size += 1 + buffa::types::string_encoded_len(&self.name);
        size as u32
    }

    fn write_to(&self, _cache: &mut SizeCache, buf: &mut impl EncodeSink) {
        Tag::new(1, WireType::Varint).encode(buf);
        encode_varint(self.id as u64, buf);
        Tag::new(2, WireType::LengthDelimited).encode(buf);
        encode_varint(self.name.len() as u64, buf);
        buf.put_slice(self.name.as_bytes());
    }

    fn merge_field(
        &mut self,
        tag: Tag,
        buf: &mut impl Buf,
        _ctx: DecodeContext<'_>,
    ) -> Result<(), DecodeError> {
        match tag.field_number() {
            1 => {
                self.id = decode_varint(buf)? as i32;
                Ok(())
            }
            2 => {
                let len = decode_varint(buf)? as usize;
                let mut bytes = vec![0u8; len];
                buf.copy_to_slice(&mut bytes);
                self.name = String::from_utf8(bytes).map_err(|_| DecodeError::InvalidUtf8)?;
                Ok(())
            }
            _ => skip_field(tag, buf),
        }
    }

    fn clear(&mut self) {
        *self = Self::default();
    }
}

fn echo_router() -> Router {
    let mut router = Router::new();
    router.unary(
        ECHO_PATH,
        ProcedureCodecs::<TestMsg, TestMsg>::defaults(),
        |_ctx, request: Request<TestMsg>| async move { Ok(Response::new(request.msg)) },
        HandlerOptions::new(),
    );
    router
}

fn json_headers() -> SimpleHeaders {
    let mut headers = SimpleHeaders::new();
    headers.insert(
        SimpleHeader::CONTENT_TYPE,
        vec!["application/json".to_string()],
    );
    headers
}

#[valtron_test]
async fn mem_unary_round_trip_returns_the_server_reply() {
    let network = Arc::new(MemNetwork::new(
        echo_router(),
        foundation_context::Ctx::host(),
    ));
    let transport = MemTransport::new(network);
    let request = request_descriptor(ECHO_URL, json_headers()).expect("descriptor");
    let input = TestMsg {
        id: 7,
        name: "in memory".into(),
    };
    let body = Bytes::from(serde_json::to_vec(&input).expect("json"));

    let (status, _headers, reply) = round_trip(&transport, request, body).await.expect("call");
    assert_eq!(status, Status::OK);
    let output: TestMsg = JsonCodec
        .unmarshal(reply)
        .expect("the response is the echoed message");
    assert_eq!(output, input);
}
