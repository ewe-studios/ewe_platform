/// Linux IPC bus controller — routes messages between endpoints based on label selectors.

use std::{mem, sync::mpsc::Sender, thread, time::{Duration, Instant}};

use crate::ipc::{
    decode, version::version, version::Version,
    ConnectMessage, ConnectMessageAck, Message,
    EndpointID, Error, Label, LabelOp, Selector, SelectorMode,
};
use super::fd::Remote;
use super::encoded_message::EncodedMessage;
use foundation_core::type_uuid::TypeUuid;

use super::IoHub;

struct Endpoint {
    id: EndpointID,
    label: Label,
    remote: Remote,
}

impl PartialEq for Endpoint {
    fn eq(&self, other: &Self) -> bool {
        self.label == other.label && self.remote == other.remote
    }
}

pub(crate) struct BusController {
    label: Label,
    token: String,
    #[allow(dead_code)]
    endpoint_id: EndpointID,
    sender: Sender<EncodedMessage>,
    endpoints: Vec<Endpoint>,
    message_buffer: Vec<(Instant, EncodedMessage)>,
    message_buffer_swap: Vec<(Instant, EncodedMessage)>,
    io_hub: IoHub,
    last_detect_reachable: Instant,
}

impl BusController {
    pub(crate) fn new(
        endpoint_id: EndpointID,
        label: Label,
        token: String,
        sender: Sender<EncodedMessage>,
        io_hub: IoHub,
    ) -> Self {
        Self {
            endpoint_id,
            label,
            token,
            sender,
            endpoints: Default::default(),
            message_buffer: Default::default(),
            message_buffer_swap: Default::default(),
            io_hub,
            last_detect_reachable: Instant::now(),
        }
    }

    pub fn run(mut self) {
        thread::Builder::new()
            .name(String::from("ipmb bus controller"))
            .spawn(move || loop {
                let msg = match self.io_hub.recv(None, None) {
                    Ok(msg) => msg,
                    Err(Error::VersionMismatch(_, _)) => {
                        // Can't send back version mismatch reply without a remote reference
                        continue;
                    }
                    _ => continue,
                };

                let now = Instant::now();

                let (remain, endpoint_connected) = self.handle_message(msg);

                if let Some(remain) = remain {
                    if !remain.selector.ttl.is_zero() {
                        self.message_buffer
                            .push((now + remain.selector.ttl, remain));
                    }
                } else if endpoint_connected && !self.message_buffer.is_empty() {
                    let mut message_buffer = mem::take(&mut self.message_buffer);

                    for (expire, msg) in message_buffer.drain(..) {
                        let (remain, _) = self.handle_message(msg);
                        if let Some(remain) = remain {
                            if expire > now {
                                self.message_buffer_swap.push((expire, remain));
                            }
                        }
                    }

                    self.message_buffer = message_buffer;
                    mem::swap(&mut self.message_buffer, &mut self.message_buffer_swap);
                }

                self.detect_reachable(now);
                self.maintain(now);
            })
            .expect("failed to spawn ipmb bus controller");
    }

    // Don't read or write self.message_buffer directly
    fn handle_message(
        &mut self,
        mut encoded_msg: EncodedMessage,
    ) -> (Option<EncodedMessage>, bool) {
        let mut routed = false;
        let mut remain = None;
        let mut endpoint_connected = false;

        match encoded_msg.selector.uuid {
            <ConnectMessage as TypeUuid>::UUID => {
                endpoint_connected = self.endpoint_connect(encoded_msg);
            }
            _ => {
                self.endpoints.retain(|Endpoint { label, remote, .. }| {
                    let mut online = true;

                    if routed && encoded_msg.selector.mode == SelectorMode::Unicast {
                        return online;
                    }

                    if encoded_msg.selector.label_op.validate(label) {
                        match encoded_msg.send(remote) {
                            Ok(_) => routed = true,
                            Err(Error::Disconnect) => online = false,
                            _ => {}
                        }
                    }

                    online
                });

                if (!routed || encoded_msg.selector.mode == SelectorMode::Multicast)
                    && encoded_msg.selector.label_op.validate(&self.label)
                {
                    match self.sender.send(encoded_msg) {
                        Ok(_) => {}
                        Err(err) => {
                            if !routed {
                                remain = Some(err.0);
                            }
                        }
                    }
                } else {
                    if !routed {
                        remain = Some(encoded_msg);
                    }
                }
            }
        }

        (remain, endpoint_connected)
    }

    fn endpoint_connect(&mut self, mut encoded_msg: EncodedMessage) -> bool {
        let remote = encoded_msg.extract_remote();
        let Some(remote) = remote else { return false };

        // Decode ConnectMessage
        let payload = match decode::<ConnectMessage>(encoded_msg.payload_data) {
            Ok(p) => p,
            Err(_) => {
                let _ = Message::new(
                    encoded_msg.selector.clone(),
                    ConnectMessageAck::ErrVersion(version()),
                )
                .into_encoded()
                .send(&remote);
                return false;
            }
        };

        // Version check
        if !payload.version.compatible(version()) {
            let _ = Message::new(
                encoded_msg.selector.clone(),
                ConnectMessageAck::ErrVersion(version()),
            )
            .into_encoded()
            .send(&remote);
            return false;
        }

        // Token check
        if payload.token != self.token {
            let _ = Message::new(encoded_msg.selector.clone(), ConnectMessageAck::ErrToken)
                .into_encoded()
                .send(&remote);
            return false;
        }

        let endpoint_id = EndpointID::new();

        // Send ack
        if let Err(err) = Message::new(
            encoded_msg.selector.clone(),
            ConnectMessageAck::Ok(endpoint_id),
        )
        .into_encoded()
        .send(&remote)
        {
            tracing::error!("connect ack: {:?}", err);
            return false;
        }

        let pair = Endpoint {
            id: endpoint_id,
            label: payload.label,
            remote,
        };

        // Don't add duplicates
        if self
            .endpoints
            .iter()
            .any(|ep| ep.label == pair.label && ep.remote == pair.remote)
        {
            return false;
        }

        let _ = self.endpoints.push(pair);
        true
    }

    fn detect_reachable(&mut self, now: Instant) {
        if now - self.last_detect_reachable > Duration::from_secs(30) {
            self.endpoints.retain(|ep| !ep.remote.is_dead());
            self.last_detect_reachable = now;
        }
    }

    fn maintain(&mut self, now: Instant) {
        self.message_buffer.retain(|(expire, _)| *expire > now);
    }
}
