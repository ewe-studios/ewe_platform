use std::sync::Arc;

use foundation_compact::rng::Rng;
use foundation_core::valtron::valtron_test;
use foundation_ui_runtime::action::{ActionRegistry, Origin, Surface};
use foundation_ui_runtime::dispatch::{Connection, DispatcherBuilder};
use foundation_ui_wire::writer::ClientCaps;
use foundation_ui_wire::{read_frame, WireLimits, SCHEMA_VERSION};

use super::dispatch_support::request;
use super::support::{test_ctx, WALL};

#[valtron_test]
async fn every_input_gives_one_valid_frame() {
    let (ctx, _) = test_ctx(WALL);
    let dispatcher =
        DispatcherBuilder::new(Origin::Server, Arc::new(ActionRegistry::new())).build();
    let conn = Connection::stateless(ClientCaps::web_current(), Surface::Web, 7);
    let check = async |bytes: &[u8]| {
        let reply = dispatcher.dispatch(&ctx, &conn, bytes).await;
        let frame = read_frame(&reply, &WireLimits::default()).unwrap();
        assert_eq!(frame.schema_version(), SCHEMA_VERSION);
        let messages = frame.kind_as_frame_message().unwrap().messages().unwrap();
        assert_eq!(messages.len(), 1);
        assert!(messages.get(0).control().is_some());
    };
    let valid = request("server:ping", 42, None, 3);
    for len in 0..=valid.len() {
        check(&valid[..len]).await;
    }
    for index in 0..valid.len().min(128) {
        for bit in 0..8 {
            let mut altered = valid.clone();
            altered[index] ^= 1 << bit;
            check(&altered).await;
        }
    }
    for _ in 0..200 {
        let bytes = {
            let rng = ctx.rng();
            let mut rng = rng.lock().unwrap();
            let mut bytes = vec![0; (rng.next_u32() % 256) as usize];
            rng.fill_bytes(&mut bytes);
            bytes
        };
        check(&bytes).await;
    }
}
